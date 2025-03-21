//! Driver for network services.

use alloy_primitives::Address;
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use tokio::{
    select,
    sync::{mpsc::Receiver, watch::Sender},
};

use crate::{Discv5Driver, GossipDriver, NetworkBuilder};

/// Network
///
/// Contains the logic to run Optimism's consensus-layer networking stack.
/// There are two core services that are run by the driver:
/// - Block gossip through Gossipsub.
/// - Peer discovery with `discv5`.
#[derive(Debug)]
pub struct Network {
    /// Channel to receive unsafe blocks.
    pub(crate) unsafe_block_recv: Option<Receiver<OpNetworkPayloadEnvelope>>,
    /// Channel to send unsafe signer updates.
    pub(crate) unsafe_block_signer_sender: Option<Sender<Address>>,
    /// The swarm instance.
    pub gossip: GossipDriver,
    /// The discovery service driver.
    pub discovery: Discv5Driver,
}

impl Network {
    /// Returns the [`NetworkBuilder`] that can be used to construct the [`Network`].
    pub const fn builder() -> NetworkBuilder {
        NetworkBuilder::new()
    }

    /// Take the unsafe block receiver.
    pub fn take_unsafe_block_recv(&mut self) -> Option<Receiver<OpNetworkPayloadEnvelope>> {
        self.unsafe_block_recv.take()
    }

    /// Take the unsafe block signer sender.
    pub fn take_unsafe_block_signer_sender(&mut self) -> Option<Sender<Address>> {
        self.unsafe_block_signer_sender.take()
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
                    enr = handler.enr_receiver.recv() => {
                        let Some(ref enr) = enr else {
                            trace!(target: "p2p::driver", "Receiver `None` peer enr");
                            continue;
                        };
                        self.gossip.dial(enr.clone());
                    },
                    event = self.gossip.select_next_some() => {
                        trace!(target: "p2p::driver", "Received event: {:?}", event);
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
                            self.gossip.dial(enr);
                        }
                    }
                }
            }
        });

        Ok(())
    }
}
