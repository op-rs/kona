//! Driver for network services.

use alloy_primitives::Address;
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::collections::VecDeque;
use tokio::{
    select,
    sync::{
        broadcast::{Receiver as BroadcastReceiver, Sender as BroadcastSender},
        watch::Sender,
    },
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
    /// Channel to send unsafe blocks.
    pub(crate) payload_tx: BroadcastSender<OpNetworkPayloadEnvelope>,
    /// Channel to send unsafe signer updates.
    pub(crate) unsafe_block_signer_sender: Option<Sender<Address>>,
    /// Handler for RPC Requests.
    ///
    /// This is allowed to be optional since it may not be desirable
    /// run a networking stack with RPC access.
    pub(crate) rpc: Option<tokio::sync::mpsc::Receiver<NetRpcRequest>>,
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
    pub fn unsafe_block_recv(&mut self) -> BroadcastReceiver<OpNetworkPayloadEnvelope> {
        self.payload_tx.subscribe()
    }

    /// Take the unsafe block signer sender.
    pub fn take_unsafe_block_signer_sender(&mut self) -> Option<Sender<Address>> {
        self.unsafe_block_signer_sender.take()
    }

    /// Starts the Discv5 peer discovery & libp2p services
    /// and continually listens for new peers and messages to handle
    pub fn start(mut self) -> Result<(), TransportError<std::io::Error>> {
        let mut rpc = self.rpc.unwrap_or_else(|| tokio::sync::mpsc::channel(1).1);
        let mut handler = self.discovery.start();
        self.gossip.listen()?;
        tokio::spawn(async move {
            // A vec deque that holds unsafe blocks to be sent over the channel.
            let mut unsafe_blocks = VecDeque::<OpNetworkPayloadEnvelope>::new();
            loop {
                // Attempt to send the unsafe blocks over the channel **in order**.
                loop {
                    if unsafe_blocks.is_empty() {
                        break;
                    }
                    if let Some(block) = unsafe_blocks.front() {
                        if let Err(e) = self.payload_tx.send(block.clone()) {
                            trace!("Failed to send unsafe block through channel: {:?}", e);
                            break;
                        }
                        unsafe_blocks.pop_front();
                    }
                }
                select! {
                    event = self.gossip.select_next_some() => {
                        trace!(target: "p2p::driver", "Received event: {:?}", event);
                        if let Some(payload) = self.gossip.handle_event(event) {
                            unsafe_blocks.push_back(payload);
                        }
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
