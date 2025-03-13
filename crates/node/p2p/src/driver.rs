//! Driver for network services.

use std::sync::mpsc::Receiver;

use alloy_primitives::Address;
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use tokio::{select, sync::watch};

use crate::{AnyNode, Discv5Driver, GossipDriver, NetworkDriverBuilder};

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
        let mut enr_recv = self.discovery.start();
        self.gossip.listen()?;
        tokio::spawn(async move {
            loop {
                select! {
                    enr = enr_recv.recv() => {
                        let Some(ref enr) = enr else {
                            warn!(target: "p2p::driver", "Receiver `None` peer enr");
                            continue;
                        };
                        let any_node = AnyNode::from(enr.clone());
                        let dial_opts = match any_node.as_dial_opts() {
                            Ok(opts) => opts,
                            Err(e) => {
                                warn!(target: "p2p::driver", "Failed to construct dial opts from enr: {:?}, {:?}", enr, e);
                                continue;
                            }
                        };

                        match self.gossip.dial(dial_opts).await {
                            Ok(_) => {
                                info!(target: "p2p::driver", "Connected to peer: {:?} | Connected peers: {:?}", enr, self.gossip.connected_peers());
                            }
                            Err(e) => {
                                warn!(target: "p2p::driver", "Failed to connect to peer: {:?} | Connected peers: {:?} | Error: {:?}", enr, self.gossip.connected_peers(), e);
                            }
                        }
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
