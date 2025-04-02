//! Driver for network services.

use alloy_primitives::Address;
use kona_genesis::RollupConfig;
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use tokio::{
    select,
    sync::{mpsc::Receiver, watch::Sender},
};

use crate::{Discv5Driver, GossipDriver, NetRpcRequest, NetworkBuilder};

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
    /// Handler for RPC Requests.
    ///
    /// This is allowed to be optional since it may not be desirable
    /// run a networking stack with RPC access.
    pub(crate) rpc: Option<tokio::sync::mpsc::Receiver<NetRpcRequest>>,
    /// A channel to publish an unsafe block.
    pub(crate) publish_rx: Option<Receiver<OpNetworkPayloadEnvelope>>,
    /// Optional [`RollupConfig`] used for selecting the topic to publish to.
    pub(crate) cfg: Option<RollupConfig>,
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
        let mut rpc = self.rpc.unwrap_or_else(|| tokio::sync::mpsc::channel(1).1);
        let mut publish = self.publish_rx.unwrap_or_else(|| tokio::sync::mpsc::channel(1).1);
        let mut handler = self.discovery.start();
        self.gossip.listen()?;
        tokio::spawn(async move {
            loop {
                select! {
                    block = publish.recv() => {
                        let Some(block) = block else {
                            trace!(target: "p2p::driver", "Receiver `None` unsafe block");
                            continue;
                        };
                        let timestamp = block.payload.timestamp();
                        let selector = |handler: &crate::BlockHandler| {
                            handler.topic(timestamp, self.cfg.as_ref())
                        };
                        match self.gossip.publish(selector, Some(block)) {
                            Ok(id) => info!("Published unsafe payload | {:?}", id),
                            Err(e) => warn!("Failed to publish unsafe payload: {:?}", e),
                        }
                    }
                    event = self.gossip.select_next_some() => {
                        trace!(target: "p2p::driver", "Received event: {:?}", event);
                        self.gossip.handle_event(event);
                    },
                    enr = handler.enr_receiver.recv() => {
                        let Some(ref enr) = enr else {
                            trace!(target: "p2p::driver", "Receiver `None` peer enr");
                            continue;
                        };
                        self.gossip.dial(enr.clone());
                    },
                    req = rpc.recv() => {
                        let Some(req) = req else {
                            trace!(target: "p2p::driver", "Receiver `None` rpc request");
                            continue;
                        };
                        match req {
                            crate::NetRpcRequest::PeerInfo(sender) => {
                                let peer_id = self.gossip.local_peer_id().to_string();
                                let chain_id = handler.chain_id;
                                let enr = handler.local_enr().await.unwrap();
                                let node_id = handler.local_enr().await.unwrap().id().unwrap_or_default();
                                let addresses = self.gossip.swarm.external_addresses().map(|a| a.to_string()).collect::<Vec<String>>();
                                let peer_info = kona_rpc::PeerInfo {
                                    peer_id,
                                    node_id,
                                    user_agent: "kona".to_string(),
                                    protocol_version: "1".to_string(),
                                    enr: enr.to_string(),
                                    addresses,
                                    protocols: None, // TODO: peer supported protocols
                                    connectedness: kona_rpc::Connectedness::Connected,
                                    direction: kona_rpc::Direction::Inbound,
                                    protected: false,
                                    chain_id,
                                    latency: 0,
                                    gossip_blocks: false,
                                    peer_scores: kona_rpc::PeerScores::default(),
                                };
                                if let Err(e) = sender.send(peer_info) {
                                    warn!("Failed to send peer info through response channel: {:?}", e);
                                }
                            }
                            crate::NetRpcRequest::PeerCount(sender) => {
                                let disc_pc = handler.peers();
                                let gossip_pc = self.gossip.connected_peers();
                                if let Err(e) = sender.send((disc_pc, gossip_pc)) {
                                    warn!("Failed to send peer count through response channel: {:?}", e);
                                }
                            }
                        }
                    },
                }
            }
        });

        Ok(())
    }
}
