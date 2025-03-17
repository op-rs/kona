//! Driver for network services.

use std::sync::mpsc::Receiver;

use alloy_primitives::Address;
use discv5::Enr;
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

    /// Dials a given [`Enr`] using the gossip libp2p swarm.
    pub async fn dial_enr(gossip: &mut GossipDriver, enr: &Enr) {
        let any_node = AnyNode::from(enr.clone());
        let dial_opts = match any_node.as_dial_opts() {
            Ok(opts) => opts,
            Err(e) => {
                warn!(target: "p2p::driver", "Failed to construct dial opts from enr: {:?}, {:?}", enr, e);
                return;
            }
        };

        match gossip.dial(dial_opts).await {
            Ok(_) => {
                info!(target: "p2p::driver", "Connected to peer: {:?} | Connected peers: {:?}", enr, gossip.connected_peers());
            }
            Err(e) => {
                info!(target: "p2p::driver", "Failed to connect to peer: {:?}", e);
            }
        }
    }

    /// Starts the Discv5 peer discovery & libp2p services
    /// and continually listens for new peers and messages to handle
    pub fn start(mut self) -> Result<(), TransportError<std::io::Error>> {
        let mut handler = self.discovery.start();
        self.gossip.listen()?;
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        tokio::spawn(async move {
            loop {
                select! {
                    // event = handler.event() => {
                    //     use discv5::Event;
                    //     let Some(ref event) = event else {
                    //         trace!(target: "p2p::driver", "Empty event received");
                    //         continue;
                    //     };
                    //     info!(target: "p2p::driver", "Received event: {:?}", event);
                    //     let Event::SessionEstablished(enr, _) = event else {
                    //         trace!(target: "p2p::driver", "Received non-established session event: {:?}", event);
                    //         continue;
                    //     };
                    //     Self::dial_enr(&mut self.gossip, &enr).await;
                    // },
                    enr = handler.enr_receiver.recv() => {
                        let Some(ref enr) = enr else {
                            trace!(target: "p2p::driver", "Receiver `None` peer enr");
                            continue;
                        };
                        Self::dial_enr(&mut self.gossip, enr).await;
                    },
                    event = self.gossip.select_next_some() => {
                        debug!(target: "p2p::driver", "Received event: {:?}", event);
                        self.gossip.handle_event(event);
                    },
                    _ = interval.tick() => {
                        let swarm_peers = self.gossip.connected_peers();
                        info!(target: "p2p::driver", "Swarm peer count: {}", swarm_peers);
                        let metrics = handler.metrics().await;
                        debug!(target: "p2p::driver", "Discovery metrics: {:?}", metrics);
                        let peers = handler.peers().await;
                        info!(target: "p2p::driver", "Discovery peer count: {:?}", peers);
                        let enrs = handler.table_enrs().await;
                        info!(target: "p2p::driver", "{} Enrs in Discovery Table", enrs.len());
                        for enr in enrs {
                            Self::dial_enr(&mut self.gossip, &enr).await;
                        }
                    }
                }
            }
        });

        Ok(())
    }
}
