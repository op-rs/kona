use alloy_primitives::Address;
use alloy_signer::SignerSync;
use async_trait::async_trait;
use kona_p2p::P2pRpcRequest;
use libp2p::TransportError;
use op_alloy_rpc_types_engine::{OpExecutionPayloadEnvelope, OpNetworkPayloadEnvelope};
use thiserror::Error;
use tokio::{self, select, sync::mpsc};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

use crate::{
    CancellableContext, NodeActor,
    actors::network::{
        builder::NetworkBuilder, driver::NetworkDriverError, error::NetworkBuilderError,
    },
};

/// The network actor handles two core networking components of the rollup node:
/// - *discovery*: Peer discovery over UDP using discv5.
/// - *gossip*: Block gossip over TCP using libp2p.
///
/// The network actor itself is a light wrapper around the [`NetworkBuilder`].
///
/// ## Example
///
/// ```rust,ignore
/// use kona_p2p::NetworkDriver;
/// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
///
/// let chain_id = 10;
/// let signer = Address::random();
/// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9099);
///
/// // Construct the `Network` using the builder.
/// // let mut driver = Network::builder()
/// //    .with_unsafe_block_signer(signer)
/// //    .with_chain_id(chain_id)
/// //    .with_gossip_addr(socket)
/// //    .build()
/// //    .unwrap();
///
/// // Construct the `NetworkActor` with the [`Network`].
/// // let actor = NetworkActor::new(driver);
/// ```
#[derive(Debug)]
pub struct NetworkActor {
    /// Network driver
    pub(super) builder: NetworkBuilder,
    /// A channel to receive the unsafe block signer address.
    pub(super) signer: mpsc::Receiver<Address>,
    /// Handler for RPC Requests.
    pub(super) rpc: mpsc::Receiver<P2pRpcRequest>,
    /// A channel to receive unsafe blocks and send them through the gossip layer.
    pub(super) publish_rx: mpsc::Receiver<OpExecutionPayloadEnvelope>,
}

/// The inbound data for the network actor.
#[derive(Debug)]
pub struct NetworkInboundData {
    /// A channel to send the unsafe block signer address to the network actor.
    pub signer: mpsc::Sender<Address>,
    /// Handler for RPC Requests sent to the network actor.
    pub rpc: mpsc::Sender<P2pRpcRequest>,
    /// A channel to send unsafe blocks to the network actor.
    pub unsafe_blocks: mpsc::Sender<OpExecutionPayloadEnvelope>,
}

impl NetworkActor {
    /// Constructs a new [`NetworkActor`] given the [`NetworkBuilder`]
    pub fn new(driver: NetworkBuilder) -> (NetworkInboundData, Self) {
        let (signer_tx, signer_rx) = mpsc::channel(16);
        let (rpc_tx, rpc_rx) = mpsc::channel(1024);
        let (publish_tx, publish_rx) = tokio::sync::mpsc::channel(256);
        let actor = Self { builder: driver, signer: signer_rx, rpc: rpc_rx, publish_rx };
        let outbound_data =
            NetworkInboundData { signer: signer_tx, rpc: rpc_tx, unsafe_blocks: publish_tx };
        (outbound_data, actor)
    }
}

/// The communication context used by the network actor.
#[derive(Debug)]
pub struct NetworkContext {
    /// The channel used by the sequencer actor for sending unsafe blocks to the network.
    pub blocks: mpsc::Sender<OpExecutionPayloadEnvelope>,
    /// Cancels the network actor.
    pub cancellation: CancellationToken,
}

impl CancellableContext for NetworkContext {
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}

/// An error from the network actor.
#[derive(Debug, Error)]
pub enum NetworkActorError {
    /// Network builder error.
    #[error(transparent)]
    NetworkBuilder(#[from] NetworkBuilderError),
    /// Network driver error.
    #[error(transparent)]
    NetworkDriver(#[from] NetworkDriverError),
    /// Driver startup failed.
    #[error(transparent)]
    DriverStartup(#[from] TransportError<std::io::Error>),
    /// The network driver was missing its unsafe block receiver.
    #[error("Missing unsafe block receiver in network driver")]
    MissingUnsafeBlockReceiver,
    /// The network driver was missing its unsafe block signer sender.
    #[error("Missing unsafe block signer in network driver")]
    MissingUnsafeBlockSigner,
    /// Channel closed unexpectedly.
    #[error("Channel closed unexpectedly")]
    ChannelClosed,
}

#[async_trait]
impl NodeActor for NetworkActor {
    type Error = NetworkActorError;
    type InboundData = NetworkInboundData;
    type OutboundData = NetworkContext;
    type Builder = NetworkBuilder;

    fn build(state: Self::Builder) -> (Self::InboundData, Self) {
        Self::new(state)
    }

    async fn start(
        mut self,
        NetworkContext { blocks, cancellation }: Self::OutboundData,
    ) -> Result<(), Self::Error> {
        let local_signer = self.builder.local_signer.clone();

        let mut handler = self.builder.build()?.start().await?;

        // New unsafe block channel.
        let (unsafe_block_tx, mut unsafe_block_rx) = tokio::sync::mpsc::unbounded_channel();

        loop {
            select! {
                _ = cancellation.cancelled() => {
                    info!(
                        target: "network",
                        "Received shutdown signal. Exiting network task."
                    );
                    return Ok(());
                }
                block = unsafe_block_rx.recv() => {
                    let Some(block) = block else {
                        error!(target: "node::p2p", "The unsafe block receiver channel has closed");
                        return Err(NetworkActorError::ChannelClosed);
                    };

                    if blocks.send(block).await.is_err() {
                        warn!(target: "network", "Failed to forward unsafe block");
                        return Err(NetworkActorError::ChannelClosed);
                    }
                }
                signer = self.signer.recv() => {
                    let Some(signer) = signer else {
                        warn!(
                            target: "network",
                            "Found no unsafe block signer on receive"
                        );
                        return Err(NetworkActorError::ChannelClosed);
                    };
                    if handler.unsafe_block_signer_sender.send(signer).is_err() {
                        warn!(
                            target: "network",
                            "Failed to send unsafe block signer to network handler",
                        );
                    }
                }
                Some(block) = self.publish_rx.recv(), if !self.publish_rx.is_closed() => {
                    let timestamp = block.payload.timestamp();
                    let selector = |handler: &kona_p2p::BlockHandler| {
                        handler.topic(timestamp)
                    };
                    let Some(signer) = local_signer.as_ref() else {
                        warn!(target: "net", "No local signer available to sign the payload");
                        continue;
                    };
                    use ssz::Encode;
                    let ssz_bytes = block.as_ssz_bytes();
                    let Ok(signature) = signer.sign_message_sync(&ssz_bytes) else {
                        warn!(target: "net", "Failed to sign the payload bytes");
                        continue;
                    };
                    let payload_hash = block.payload_hash();
                    let payload = OpNetworkPayloadEnvelope {
                        payload: block.payload,
                        signature,
                        payload_hash,
                        parent_beacon_block_root: block.parent_beacon_block_root,
                    };
                    match handler.gossip.publish(selector, Some(payload)) {
                        Ok(id) => info!("Published unsafe payload | {:?}", id),
                        Err(e) => warn!("Failed to publish unsafe payload: {:?}", e),
                    }
                }
                event = handler.gossip.next() => {
                    let Some(event) = event else {
                        error!(target: "node::p2p", "The gossip swarm stream has ended");
                        return Err(NetworkActorError::ChannelClosed);
                    };

                    if let Some(payload) = handler.gossip.handle_event(event) {
                        if unsafe_block_tx.send(payload.into()).is_err() {
                            warn!(target: "node::p2p", "Failed to send unsafe block to network handler");
                        }
                    }
                },
                enr = handler.enr_receiver.recv() => {
                    let Some(enr) = enr else {
                        error!(target: "node::p2p", "The enr receiver channel has closed");
                        return Err(NetworkActorError::ChannelClosed);
                    };
                    handler.gossip.dial(enr);
                },
                _ = handler.peer_score_inspector.tick(), if handler.gossip.peer_monitoring.as_ref().is_some() => {
                    handler.handle_peer_monitoring().await;
                },
                req = self.rpc.recv(), if !self.rpc.is_closed() => {
                    let Some(req) = req else {
                        error!(target: "node::p2p", "The rpc receiver channel has closed");
                        return Err(NetworkActorError::ChannelClosed);
                    };
                    let payload = match req {
                        P2pRpcRequest::PostUnsafePayload { payload } => payload,
                        req => {
                            req.handle(&mut handler.gossip, &handler.discovery);
                            continue;
                        }
                    };
                    debug!(target: "node::p2p", "Broadcasting unsafe payload from admin api");
                    if unsafe_block_tx.send(payload).is_err() {
                        warn!(target: "node::p2p", "Failed to send unsafe block to network handler");
                    }
                },
            }
        }
    }
}
