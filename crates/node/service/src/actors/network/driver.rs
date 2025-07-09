use alloy_primitives::Address;
use kona_p2p::{ConnectionGater, Discv5Driver, GossipDriver, PEER_SCORE_INSPECT_FREQUENCY};
use libp2p::TransportError;
use tokio::sync::watch;

use crate::actors::network::handler::NetworkHandler;

/// A network driver. This is the driver that is used to start the network.
#[derive(Debug)]
pub struct NetworkDriver {
    /// The gossip driver.
    pub gossip: GossipDriver<ConnectionGater>,
    /// The discovery driver.
    pub discovery: Discv5Driver,
    /// The unsafe block signer sender.
    pub unsafe_block_signer_sender: watch::Sender<Address>,
}

/// An error from the [`NetworkDriver`].
#[derive(Debug, thiserror::Error)]
pub enum NetworkDriverError {
    /// An error occurred starting the libp2p Swarm.
    #[error("error starting libp2p Swarm")]
    GossipStartError(#[from] TransportError<std::io::Error>),
}

impl NetworkDriver {
    /// Starts the network.
    pub async fn start(mut self) -> Result<NetworkHandler, NetworkDriverError> {
        // Start the discovery service.
        let (handler, enr_receiver) = self.discovery.start();

        // Start the libp2p Swarm
        self.gossip.start().await?;

        // We are checking the peer scores every [`PEER_SCORE_INSPECT_FREQUENCY`] seconds.
        let peer_score_inspector = tokio::time::interval(*PEER_SCORE_INSPECT_FREQUENCY);

        Ok(NetworkHandler {
            gossip: self.gossip,
            discovery: handler,
            enr_receiver,
            unsafe_block_signer_sender: self.unsafe_block_signer_sender,
            peer_score_inspector,
        })
    }
}
