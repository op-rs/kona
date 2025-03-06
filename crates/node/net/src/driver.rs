//! Driver for network services.

use std::sync::mpsc::Receiver;

use alloy_primitives::Address;
use alloy_rpc_types_engine::ExecutionPayload;
use libp2p::TransportError;
use tokio::{select, sync::watch};

use crate::{
    builder::NetworkDriverBuilder, discovery::driver::DiscoveryDriver, gossip::driver::GossipDriver,
};

/// NetworkDriver
///
/// Contains the logic to run Optimism's consensus-layer networking stack.
/// There are two core services that are run by the driver:
/// - Block gossip through Gossipsub.
/// - Peer discovery with `discv5`.
pub struct NetworkDriver {
    /// Channel to receive unsafe blocks.
    pub(crate) unsafe_block_recv: Option<Receiver<ExecutionPayload>>,
    /// Channel to send unsafe signer updates.
    pub(crate) unsafe_block_signer_sender: Option<watch::Sender<Address>>,
    /// The swarm instance.
    pub gossip: GossipDriver,
    /// The discovery service driver.
    pub discovery: DiscoveryDriver,
}

impl NetworkDriver {
    /// Returns a new [NetworkDriverBuilder].
    pub fn builder() -> NetworkDriverBuilder {
        NetworkDriverBuilder::new()
    }

    /// Take the unsafe block receiver.
    pub fn take_unsafe_block_recv(&mut self) -> Option<Receiver<ExecutionPayload>> {
        self.unsafe_block_recv.take()
    }

    /// Take the unsafe block signer sender.
    pub fn take_unsafe_block_signer_sender(&mut self) -> Option<watch::Sender<Address>> {
        self.unsafe_block_signer_sender.take()
    }

    /// Starts the Discv5 peer discovery & libp2p services
    /// and continually listens for new peers and messages to handle
    pub fn start(mut self) -> Result<(), TransportError<std::io::Error>> {
        let mut peer_recv = self.discovery.start();
        self.gossip.listen()?;
        tokio::spawn(async move {
            loop {
                select! {
                    peer = peer_recv.recv() => {
                        self.gossip.dial_opt(peer.clone()).await;
                        tracing::info!("Received peer: {:?} | Connected peers: {:?}", peer, self.gossip.connected_peers());
                    },
                    event = self.gossip.select_next_some() => {
                        tracing::debug!("Received event: {:?}", event);
                        self.gossip.handle_event(event);
                    },
                }
            }
        });

        Ok(())
    }
}
