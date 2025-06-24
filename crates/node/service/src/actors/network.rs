//! Network Actor

use crate::{NodeActor, actors::ActorContext};
use alloy_primitives::Address;
use async_trait::async_trait;
use derive_more::Debug;
use kona_p2p::Network;
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use thiserror::Error;
use tokio::{select, sync::mpsc};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

/// The network actor handles two core networking components of the rollup node:
/// - *discovery*: Peer discovery over UDP using discv5.
/// - *gossip*: Block gossip over TCP using libp2p.
///
/// The network actor itself is a light wrapper around the [`Network`].
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
    driver: Network,
}

impl NetworkActor {
    /// Constructs a new [`NetworkActor`] given the [`Network`]
    pub const fn new(
        driver: Network,
        blocks: mpsc::Sender<OpExecutionPayloadEnvelope>,
        signer: mpsc::Receiver<Address>,
        cancellation: CancellationToken,
    ) -> (Self, NetworkContext) {
        let actor = Self { driver };
        let context = NetworkContext { blocks, signer, cancellation };
        (actor, context)
    }
}

/// The communication context used by the network actor.
#[derive(Debug)]
pub struct NetworkContext {
    blocks: mpsc::Sender<OpExecutionPayloadEnvelope>,
    signer: mpsc::Receiver<Address>,
    cancellation: CancellationToken,
}

impl ActorContext for NetworkContext {
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}

#[async_trait]
impl NodeActor for NetworkActor {
    type Error = NetworkActorError;
    type Context = NetworkContext;

    async fn start(
        mut self,
        NetworkContext { blocks, mut signer, cancellation }: Self::Context,
    ) -> Result<(), Self::Error> {
        // Take the unsafe block receiver
        let mut unsafe_block_receiver = self.driver.unsafe_block_recv();

        // Take the unsafe block signer sender.
        let unsafe_block_signer = self.driver.unsafe_block_signer_sender();

        // Start the network driver.
        self.driver.start().await?;

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
