//! Network Actor

use crate::{NodeActor, actors::CancellableContext};
use alloy_primitives::Address;
use async_trait::async_trait;
use derive_more::Debug;
use kona_p2p::{NetworkBuilder, NetworkBuilderError};
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
    /// The channel for sending unsafe blocks from the network actor.
    blocks: mpsc::Sender<OpExecutionPayloadEnvelope>,
}

/// The outbound data for the network actor.
#[derive(Debug)]
pub struct NetworkOutboundData {
    /// The unsafe block received from the network.
    pub unsafe_block: mpsc::Receiver<OpExecutionPayloadEnvelope>,
}

impl NetworkActor {
    /// Constructs a new [`NetworkActor`] given the [`NetworkBuilder`]
    pub fn new(driver: NetworkBuilder) -> (NetworkOutboundData, Self) {
        let (unsafe_block_tx, unsafe_block_rx) = mpsc::channel(1024);
        let actor = Self { config: driver, blocks: unsafe_block_tx };
        let outbound_data = NetworkOutboundData { unsafe_block: unsafe_block_rx };
        (outbound_data, actor)
    }
}

/// The communication context used by the network actor.
#[derive(Debug)]
pub struct NetworkContext {
    /// A channel to receive the unsafe block signer address.
    pub signer: mpsc::Receiver<Address>,
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
    type InboundData = NetworkContext;
    type OutboundData = NetworkOutboundData;
    type Builder = NetworkBuilder;

    fn build(state: Self::Builder) -> (Self::OutboundData, Self) {
        Self::new(state)
    }

    async fn start(
        mut self,
        NetworkContext { mut signer, cancellation }: Self::InboundData,
    ) -> Result<(), Self::Error> {
        let mut driver = self.config.build()?;

        // Take the unsafe block receiver
        let mut unsafe_block_receiver = driver.unsafe_block_recv();

        // Take the unsafe block signer sender.
        let unsafe_block_signer = driver.unsafe_block_signer_sender();

        // Start the network driver.
        driver.start().await?;

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
                            match self.blocks.send(block).await {
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
                signer = signer.recv() => {
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
