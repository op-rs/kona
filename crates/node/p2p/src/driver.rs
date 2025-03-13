//! Driver for network services.

use std::sync::mpsc::Receiver;

use alloy_primitives::Address;
use discv5::multiaddr::{Multiaddr, Protocol};
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use tokio::{select, sync::watch};

use crate::{Discv5Driver, GossipDriver, NetworkDriverBuilder};

/// NetworkDriver
///
/// Contains the logic to run Optimism's consensus-layer networking stack.
/// There are two core services that are run by the driver:
/// - Block gossip through Gossipsub.
/// - Peer discovery with `discv5`.
pub struct NetworkDriver {
    /// Channel to receive unsafe blocks.
    pub(crate) unsafe_block_recv: Option<Receiver<OpNetworkPayloadEnvelope>>,
    /// Channel to send unsafe signer updates.
    pub(crate) unsafe_block_signer_sender: Option<watch::Sender<Address>>,
    /// The swarm instance.
    pub gossip: GossipDriver,
    /// The discovery service driver.
    pub discovery: Discv5Driver,
}

impl NetworkDriver {
    /// Returns a new [NetworkDriverBuilder].
    pub fn builder() -> NetworkDriverBuilder {
        NetworkDriverBuilder::new()
    }

    /// Take the unsafe block receiver.
    pub fn take_unsafe_block_recv(&mut self) -> Option<Receiver<OpNetworkPayloadEnvelope>> {
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
                        let dial = if let Some(ref peer) = peer {
                            let mut multi_address = Multiaddr::empty();
                            if let Some(ip) = peer.ip4() {
                                multi_address.push(Protocol::Ip4(ip));
                            } else if let Some(ip) = peer.ip6() {
                                multi_address.push(Protocol::Ip6(ip));
                            }
                            if let Some(udp) = peer.udp4() {
                                multi_address.push(Protocol::Udp(udp));
                            } else if let Some(udp) = peer.udp6() {
                                multi_address.push(Protocol::Udp(udp));
                            }
                            Some(multi_address)
                        } else {
                            None
                        };
                        self.gossip.dial_opt(dial).await;
                        info!(target: "p2p::driver", "Received peer: {:?} | Connected peers: {:?}", peer, self.gossip.connected_peers());
                    },
                    event = self.gossip.select_next_some() => {
                        debug!(target: "p2p::driver", "Received event: {:?}", event);
                        self.gossip.handle_event(event);
                    },
                }
            }
        });

        Ok(())
    }
}
