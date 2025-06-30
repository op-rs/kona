//! Network Actor

use crate::{NodeActor, actors::CancellableContext};
use alloy_primitives::Address;
use async_trait::async_trait;
use derive_more::Debug;
use kona_p2p::{NetworkBuilder, NetworkBuilderError, P2pRpcRequest};
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use thiserror::Error;
use tokio::{select, sync::mpsc};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

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
    config: NetworkBuilder,
    /// A channel to receive the unsafe block signer address.
    signer: mpsc::Receiver<Address>,
    /// Handler for RPC Requests.
    rpc: tokio::sync::mpsc::Receiver<P2pRpcRequest>,
}

/// The inbound data for the network actor.
#[derive(Debug)]
pub struct NetworkInboundData {
    /// A channel to send the unsafe block signer address to the network actor.
    pub signer: mpsc::Sender<Address>,
    /// Handler for RPC Requests sent to the network actor.
    pub rpc: mpsc::Sender<P2pRpcRequest>,
}

impl NetworkActor {
    /// Constructs a new [`NetworkActor`] given the [`NetworkBuilder`]
    pub fn new(driver: NetworkBuilder) -> (NetworkInboundData, Self) {
        let (signer_tx, signer_rx) = mpsc::channel(16);
        let (rpc_tx, rpc_rx) = mpsc::channel(1024);
        let actor = Self { config: driver, signer: signer_rx, rpc: rpc_rx };
        let outbound_data = NetworkInboundData { signer: signer_tx, rpc: rpc_tx };
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
        let mut driver = self.config.build()?;

        // Take the unsafe block receiver
        let mut unsafe_block_receiver = driver.unsafe_block_recv();

        // Take the unsafe block signer sender.
        let unsafe_block_signer = driver.unsafe_block_signer_sender();

        // Start the network driver.
        driver.start(Some(self.rpc)).await?;

        loop {
            select! {
                _ = cancellation.cancelled() => {
                    info!(
                        target: "network",
                        "Received shutdown signal. Exiting network task."
                    );
                    return Ok(());
                }
                block = unsafe_block_receiver.recv() => {
                    match block {
                        Ok(block) => {
                            match blocks.send(block).await {
                                Ok(_) => debug!(target: "network", "Forwarded unsafe block"),
                                Err(_) => warn!(target: "network", "Failed to forward unsafe block"),
                            }
                        }
                        Err(e) => {
                            warn!(target: "network", "Failed to receive unsafe block: {:?}", e);
                            return Err(NetworkActorError::ChannelClosed);
                        }
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
                    if unsafe_block_signer.send(signer).is_err() {
                        warn!(
                            target: "network",
                            "Failed to send unsafe block signer to network driver",
                        );
                    }
                }
            }
        }
    }
}

/// An error from the network actor.
#[derive(Debug, Error)]
pub enum NetworkActorError {
    /// Network builder error.
    #[error(transparent)]
    NetworkBuilder(#[from] NetworkBuilderError),
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
