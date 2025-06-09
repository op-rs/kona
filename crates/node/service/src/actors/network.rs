//! Network Actor

use crate::NodeActor;
use alloy_primitives::Address;
use async_trait::async_trait;
use derive_more::Debug;
use kona_p2p::Network;
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use thiserror::Error;
use tokio::{select, sync::mpsc};
use tokio_util::sync::CancellationToken;

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
    /// The sender for [OpNetworkPayloadEnvelope]s received via p2p gossip.
    blocks: mpsc::Sender<OpNetworkPayloadEnvelope>,
    /// The receiver for unsafe block signer updates.
    signer: mpsc::Receiver<Address>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl NetworkActor {
    /// Constructs a new [`NetworkActor`] given the [`Network`]
    pub const fn new(
        driver: Network,
        blocks: mpsc::Sender<OpNetworkPayloadEnvelope>,
        signer: mpsc::Receiver<Address>,
        cancellation: CancellationToken,
    ) -> Self {
        Self { driver, blocks, signer, cancellation }
    }
}

#[async_trait]
impl NodeActor for NetworkActor {
    type InboundEvent = ();
    type Error = NetworkActorError;

    async fn start(mut self) -> Result<(), Self::Error> {
        // Take the unsafe block receiver
        let mut unsafe_block_receiver = self.driver.unsafe_block_recv();

        // Take the unsafe block signer sender.
        let Some(unsafe_block_signer) = self.driver.take_unsafe_block_signer_sender() else {
            return Err(NetworkActorError::MissingUnsafeBlockSigner);
        };

        // Start the network driver.
        self.driver.start().await?;

        loop {
            select! {
                _ = self.cancellation.cancelled() => {
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

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        Ok(())
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
