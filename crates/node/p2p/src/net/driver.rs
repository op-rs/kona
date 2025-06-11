//! Driver for network services.

use alloy_primitives::{Address, hex};
use futures::{AsyncReadExt, AsyncWriteExt, StreamExt};
use libp2p::TransportError;
use libp2p_stream::IncomingStreams;
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
    pub(crate) publish_tx: tokio::sync::mpsc::Sender<OpNetworkPayloadEnvelope>,
    /// A channel to receive unsafe blocks and send them through the gossip layer.
    pub(crate) publish_rx: tokio::sync::mpsc::Receiver<OpNetworkPayloadEnvelope>,
    /// The swarm instance.
    pub gossip: GossipDriver<crate::ConnectionGater>,
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

    /// Creates a new unsafe block mpsc sender.
    pub fn new_unsafe_block_sender(
        &mut self,
    ) -> tokio::sync::mpsc::Sender<OpNetworkPayloadEnvelope> {
        self.publish_tx.clone()
    }

    /// Take the unsafe block receiver.
    pub fn unsafe_block_recv(&mut self) -> BroadcastReceiver<OpNetworkPayloadEnvelope> {
        self.broadcast.subscribe()
    }

    /// Take the unsafe block signer sender.
    pub const fn take_unsafe_block_signer_sender(&mut self) -> Option<Sender<Address>> {
        self.unsafe_block_signer_sender.take()
    }

    /// Handles the sync request/response protocol.
    ///
    /// This is a mock handler that supports the `payload_by_number` protocol.
    /// It always returns: not found (1), version (0). `<https://specs.optimism.io/protocol/rollup-node-p2p.html#payload_by_number>`
    ///
    /// ## Note
    ///
    /// This is used to ensure op-nodes are not penalizing kona-nodes for not supporting it.
    /// This feature is being deprecated by the op-node team. Once it is fully removed from the
    /// op-node's implementation we will remove this handler.
    async fn sync_protocol_handler(mut sync_protocol: IncomingStreams) {
        loop {
            let Some((peer_id, mut inbound_stream)) = sync_protocol.next().await else {
                warn!(target: "node::p2p::sync", "The sync protocol stream has ended");
                return;
            };

            info!(target: "node::p2p::sync", "Received a sync request from {peer_id}, spawning a new task to handle it");

            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let Ok(bytes_received) = inbound_stream.read_to_end(&mut buffer).await else {
                    error!(target: "node::p2p::sync", "Failed to read the sync request from {peer_id}");
                    return;
                };

                debug!(target: "node::p2p::sync", bytes_received = bytes_received, peer_id = ?peer_id, payload = ?buffer, "Received inbound sync request");

                // We return: not found (1), version (0). `<https://specs.optimism.io/protocol/rollup-node-p2p.html#payload_by_number>`
                // Response format: <response> = <res><version><payload>
                // No payload is returned.
                const OUTPUT: [u8; 2] = hex!("0100");

                // We only write that we're not supporting the sync request.
                if let Err(e) = inbound_stream.write_all(&OUTPUT).await {
                    error!(target: "node::p2p::sync", err = ?e, "Failed to write the sync response to {peer_id}");
                    return;
                };

                debug!(target: "node::p2p::sync", bytes_sent = OUTPUT.len(), peer_id = ?peer_id, "Sent outbound sync response");
            });
        }
    }

    /// Starts the Discv5 peer discovery & libp2p services
    /// and continually listens for new peers and messages to handle
    pub async fn start(mut self) -> Result<(), TransportError<std::io::Error>> {
        let mut rpc = self.rpc.unwrap_or_else(|| tokio::sync::mpsc::channel(1024).1);
        let (handler, mut enr_receiver) = self.discovery.start();
        let mut broadcast = self.broadcast;

        // We are checking the peer scores every [`Self::PEER_SCORE_INSPECT_FREQUENCY`] seconds.
        let mut peer_score_inspector = tokio::time::interval(Self::PEER_SCORE_INSPECT_FREQUENCY);

        // Start the libp2p Swarm
        self.gossip.listen().await?;

        // Start the sync request/response protocol handler.
        if let Some(sync_protocol) = self.gossip.sync_protocol.take() {
            tokio::spawn(Self::sync_protocol_handler(sync_protocol));
        }

        // Spawn the network handler
        tokio::spawn(async move {
            loop {
                select! {
                    block = self.publish_rx.recv() => {
                        let Some(block) = block else {
                            continue;
                        };
                        let timestamp = block.payload.timestamp();
                        let selector = |handler: &crate::BlockHandler| {
                            handler.topic(timestamp)
                        };
                        match self.gossip.publish(selector, Some(block)) {
                            Ok(id) => info!("Published unsafe payload | {:?}", id),
                            Err(e) => warn!("Failed to publish unsafe payload: {:?}", e),
                        }
                    }
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
                                use crate::ConnectionGate;
                                self.gossip.connection_gate.remove_dial(&peer_to_remove);
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
