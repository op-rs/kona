//! Driver for network services.

use alloy_primitives::Address;
use libp2p::TransportError;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::collections::HashSet;
use tokio::{
    select,
    sync::{broadcast::Receiver as BroadcastReceiver, watch::Sender},
    time::Duration,
};

use crate::{Broadcast, Discv5Driver, GossipDriver, HandlerRequest, NetworkBuilder, P2pRpcRequest};

/// Network
///
/// Contains the logic to run Optimism's consensus-layer networking stack.
/// There are two core services that are run by the driver:
/// - Block gossip through Gossipsub.
/// - Peer discovery with `discv5`.
#[derive(Debug)]
pub struct Network {
    /// The broadcast handler to broadcast unsafe payloads.
    pub(crate) broadcast: Broadcast,
    /// Channel to send unsafe signer updates.
    pub(crate) unsafe_block_signer_sender: Option<Sender<Address>>,
    /// Handler for RPC Requests.
    ///
    /// This is allowed to be optional since it may not be desirable
    /// run a networking stack with RPC access.
    pub(crate) rpc: Option<tokio::sync::mpsc::Receiver<P2pRpcRequest>>,
    /// A channel to publish an unsafe block.
    // TODO(@theochap, <`https://github.com/op-rs/kona/issues/1849`>): we should fix that channel handler.
    // pub(crate) publish_rx: Option<tokio::sync::mpsc::Receiver<OpNetworkPayloadEnvelope>>,
    /// The swarm instance.
    pub gossip: GossipDriver,
    /// The discovery service driver.
    pub discovery: Discv5Driver,
}

impl Network {
    /// The frequency at which to inspect peer scores to ban poorly performing peers.
    const PEER_SCORE_INSPECT_FREQUENCY: Duration = Duration::from_secs(1);

    /// Returns the [`NetworkBuilder`] that can be used to construct the [`Network`].
    pub const fn builder() -> NetworkBuilder {
        NetworkBuilder::new()
    }

    /// Take the unsafe block receiver.
    pub fn unsafe_block_recv(&mut self) -> BroadcastReceiver<OpNetworkPayloadEnvelope> {
        self.broadcast.subscribe()
    }

    /// Take the unsafe block signer sender.
    pub const fn take_unsafe_block_signer_sender(&mut self) -> Option<Sender<Address>> {
        self.unsafe_block_signer_sender.take()
    }

    /// Starts the Discv5 peer discovery & libp2p services
    /// and continually listens for new peers and messages to handle
    pub async fn start(mut self) -> Result<(), TransportError<std::io::Error>> {
        let mut rpc = self.rpc.unwrap_or_else(|| tokio::sync::mpsc::channel(1024).1);
        // TODO(@theochap): we should fix that channel handler.
        // We are currently using a mpsc channel without senders which causes it to drop.
        // let publish = self.publish_rx.unwrap_or_else(|| tokio::sync::mpsc::channel(1024).1);
        let (handler, mut enr_receiver) = self.discovery.start();
        let mut broadcast = self.broadcast;

        // We are checking the peer scores every [`Self::PEER_SCORE_INSPECT_FREQUENCY`] seconds.
        let mut peer_score_inspector = tokio::time::interval(Self::PEER_SCORE_INSPECT_FREQUENCY);

        // Start the libp2p Swarm
        self.gossip.listen().await?;

        // Spawn the network handler
        tokio::spawn(async move {
            loop {
                select! {
                    // TODO(@theochap, <`https://github.com/op-rs/kona/issues/1849`>): we should fix that channel handler.
                    // We are currently using a mpsc channel without senders which causes it to drop.
                    // block = publish.recv() => {
                    //     let Some(block) = block else {
                    //         continue;
                    //     };
                    //     let timestamp = block.payload.timestamp();
                    //     let selector = |handler: &crate::BlockHandler| {
                    //         handler.topic(timestamp)
                    //     };
                    //     match self.gossip.publish(selector, Some(block)) {
                    //         Ok(id) => info!("Published unsafe payload | {:?}", id),
                    //         Err(e) => warn!("Failed to publish unsafe payload: {:?}", e),
                    //     }
                    // }
                    event = self.gossip.next() => {
                        let Some(event) = event else {
                            error!(target: "node::p2p", "The gossip swarm stream has ended");
                            return;
                        };

                        if let Some(payload) = self.gossip.handle_event(event) {
                            broadcast.push(payload);
                            broadcast.broadcast();
                        }
                    },
                    enr = enr_receiver.recv() => {
                        let Some(enr) = enr else {
                            error!(target: "node::p2p", "The enr receiver channel has closed");
                            return;
                        };
                        self.gossip.dial(enr);
                        kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER);
                    },

                    _ = peer_score_inspector.tick(), if self.gossip.peer_monitoring.as_ref().is_some() => {
                        // Inspect peer scores and ban peers that are below the threshold.
                        let Some(ban_peers) = self.gossip.peer_monitoring.as_ref() else {
                            continue;
                        };

                        // We iterate over all connected peers and check their scores.
                        // We collect a list of peers to remove
                        let peers_to_remove = self.gossip.swarm.connected_peers().filter_map(
                            |peer_id| {
                                // If the score is not available, we use a default value of 0.
                                let score = self.gossip.swarm.behaviour().gossipsub.peer_score(peer_id).unwrap_or_default();

                                // Record the peer score in the metrics.
                                kona_macros::record!(histogram, crate::Metrics::PEER_SCORES, score);

                                if score < ban_peers.ban_threshold {
                                   return Some(*peer_id);
                                }

                                None
                            }
                        ).collect::<Vec<_>>();

                        // We remove the addresses from the gossip layer.
                        let addrs_to_ban = peers_to_remove.into_iter().filter_map(|peer_to_remove| {
                            // In that case, we ban the peer. This means...
                            // 1. We remove the peer from the network gossip.
                            // 2. We ban the peer from the discv5 service.
                            if self.gossip.swarm.disconnect_peer_id(peer_to_remove).is_err() {
                                warn!(peer = ?peer_to_remove, "Trying to disconnect a non-existing peer from the gossip driver.");
                            }

                            // Record the duration of the peer connection.
                            if let Some(start_time) = self.gossip.peer_connection_start.remove(&peer_to_remove) {
                                let peer_duration = start_time.elapsed();
                                kona_macros::record!(
                                    histogram,
                                    crate::Metrics::GOSSIP_PEER_CONNECTION_DURATION_SECONDS,
                                    peer_duration.as_secs_f64()
                                );
                            }

                            if let Some(addr) = self.gossip.peerstore.remove(&peer_to_remove){
                                self.gossip.dialed_peers.remove(&addr);
                                let score = self.gossip.swarm.behaviour().gossipsub.peer_score(&peer_to_remove).unwrap_or_default();
                                kona_macros::inc!(gauge, crate::Metrics::BANNED_PEERS, "peer_id" => peer_to_remove.to_string(), "score" => score.to_string());
                                return Some(addr);
                            }

                            None
                        }).collect::<HashSet<_>>();

                        // We send a request to the discovery handler to ban the set of addresses.
                        if let Err(send_err) = handler.sender.send(HandlerRequest::BanAddrs { addrs_to_ban: addrs_to_ban.into(), ban_duration: ban_peers.ban_duration }).await{
                            warn!(err = ?send_err, "Impossible to send a request to the discovery handler. The channel connection is dropped.");
                        }
                    },
                    req = rpc.recv() => {
                        let Some(req) = req else {
                            error!(target: "node::p2p", "The rpc receiver channel has closed");
                            return;
                        };
                        req.handle(&mut self.gossip, &handler);
                    },
                }
            }
        });

        Ok(())
    }
}
