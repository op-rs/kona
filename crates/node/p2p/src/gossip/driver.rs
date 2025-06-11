//! Consensus-layer gossipsub driver for Optimism.

use derive_more::Debug;
use discv5::Enr;
use futures::stream::StreamExt;
use kona_peers::{EnrValidation, PeerMonitoring, enr_to_multiaddr};
use libp2p::{
    Multiaddr, PeerId, Swarm, TransportError,
    gossipsub::{IdentTopic, MessageId},
    swarm::SwarmEvent,
};
use libp2p_stream::IncomingStreams;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

use crate::{
    Behaviour, BlockHandler, ConnectionGate, Event, GossipDriverBuilder, Handler, PublishError,
};

/// A driver for a [`Swarm`] instance.
///
/// Connects the swarm to the given [`Multiaddr`]
/// and handles events using the [`BlockHandler`].
#[derive(Debug)]
pub struct GossipDriver<G: ConnectionGate> {
    /// The [`Swarm`] instance.
    #[debug(skip)]
    pub swarm: Swarm<Behaviour>,
    /// A [`Multiaddr`] to listen on.
    pub addr: Multiaddr,
    /// The [`BlockHandler`].
    pub handler: BlockHandler,
    /// A [`libp2p_stream::Control`] instance. Can be used to control the sync request/response
    #[debug(skip)]
    pub sync_handler: libp2p_stream::Control,
    /// The inbound streams for the sync request/response protocol.
    /// Set to `None` if the sync request/response protocol is not enabled.
    #[debug(skip)]
    pub sync_protocol: Option<IncomingStreams>,
    /// A mapping from [`PeerId`] to [`Multiaddr`].
    pub peerstore: HashMap<PeerId, Multiaddr>,
    /// A mapping from [`PeerId`] to [`libp2p::identify::Info`].
    /// TODO(@theochap, `<https://github.com/op-rs/kona/issues/2015>`): we should probably find a way to merge `peer_infos` and `peerstore` into a
    /// single map.
    pub peer_infos: HashMap<PeerId, libp2p::identify::Info>,
    /// If set, the gossip layer will monitor peer scores and ban peers that are below a given
    /// threshold.
    pub peer_monitoring: Option<PeerMonitoring>,
    /// Tracks connection start time for peers
    pub peer_connection_start: HashMap<PeerId, Instant>,
    /// The connection gate.
    pub connection_gate: G,
    /// Tracks ping times for peers.
    pub ping: Arc<Mutex<HashMap<PeerId, Duration>>>,
}

impl<G> GossipDriver<G>
where
    G: ConnectionGate,
{
    /// Returns the [`GossipDriverBuilder`] that can be used to construct the [`GossipDriver`].
    pub const fn builder() -> GossipDriverBuilder {
        GossipDriverBuilder::new()
    }

    /// Creates a new [`GossipDriver`] instance.
    pub fn new(
        swarm: Swarm<Behaviour>,
        addr: Multiaddr,
        handler: BlockHandler,
        sync_handler: libp2p_stream::Control,
        sync_protocol: IncomingStreams,
        gate: G,
    ) -> Self {
        Self {
            swarm,
            addr,
            handler,
            peerstore: Default::default(),
            peer_infos: Default::default(),
            peer_monitoring: None,
            peer_connection_start: Default::default(),
            sync_handler,
            // TODO(@theochap): make this field truly optional (through CLI args).
            sync_protocol: Some(sync_protocol),
            connection_gate: gate,
            ping: Arc::new(Mutex::new(Default::default())),
        }
    }

    /// Publishes an unsafe block to gossip.
    ///
    /// ## Arguments
    ///
    /// * `topic_selector` - A function that selects the topic for the block. This is expected to be
    ///   a closure that takes the [`BlockHandler`] and returns the [`IdentTopic`] for the block.
    /// * `payload` - The payload to be published.
    ///
    /// ## Returns
    ///
    /// Returns the [`MessageId`] of the published message or a [`PublishError`]
    /// if the message could not be published.
    pub fn publish(
        &mut self,
        selector: impl FnOnce(&BlockHandler) -> IdentTopic,
        payload: Option<OpNetworkPayloadEnvelope>,
    ) -> Result<Option<MessageId>, PublishError> {
        let Some(payload) = payload else {
            return Ok(None);
        };
        let topic = selector(&self.handler);
        let topic_hash = topic.hash();
        let data = self.handler.encode(topic, payload)?;
        let id = self.swarm.behaviour_mut().gossipsub.publish(topic_hash, data)?;
        kona_macros::inc!(gauge, crate::Metrics::UNSAFE_BLOCK_PUBLISHED);
        Ok(Some(id))
    }

    /// Tells the swarm to listen on the given [`Multiaddr`].
    /// Waits for the swarm to start listen before returning and connecting to peers.
    pub async fn listen(&mut self) -> Result<(), TransportError<std::io::Error>> {
        match self.swarm.listen_on(self.addr.clone()) {
            Ok(id) => loop {
                if let SwarmEvent::NewListenAddr { address, listener_id } =
                    self.swarm.select_next_some().await
                {
                    if id == listener_id {
                        info!(target: "gossip", "Swarm now listening on: {address}");
                        break;
                    }
                }
            },
            Err(err) => {
                error!(target: "gossip", "Fail to listen on {}: {err}", self.addr);
                return Err(err);
            }
        }
        Ok(())
    }

    /// Returns the local peer id.
    pub fn local_peer_id(&self) -> &libp2p::PeerId {
        self.swarm.local_peer_id()
    }

    /// Returns a mutable reference to the Swarm's behaviour.
    pub fn behaviour_mut(&mut self) -> &mut Behaviour {
        self.swarm.behaviour_mut()
    }

    /// Attempts to select the next event from the Swarm.
    pub async fn next(&mut self) -> Option<SwarmEvent<Event>> {
        self.swarm.next().await
    }

    /// Returns the number of connected peers.
    pub fn connected_peers(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// Dials the given [`Enr`].
    pub fn dial(&mut self, enr: Enr) {
        let validation = EnrValidation::validate(&enr, self.handler.rollup_config.l2_chain_id);
        if validation.is_invalid() {
            trace!(target: "gossip", "Invalid OP Stack ENR for chain id {}: {}", self.handler.rollup_config.l2_chain_id, validation);
            return;
        }
        let Some(multiaddr) = enr_to_multiaddr(&enr) else {
            debug!(target: "gossip", "Failed to extract tcp socket from enr: {:?}", enr);
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "invalid_enr");
            return;
        };
        self.dial_multiaddr(multiaddr);
    }

    /// Dials the given [`Multiaddr`].
    pub fn dial_multiaddr(&mut self, addr: Multiaddr) {
        // Check if we're allowed to dial the address.
        if !self.connection_gate.can_dial(&addr) {
            warn!(target: "gossip", "unable to dial peer");
            return;
        }

        // Extract the peer ID from the address.
        let Some(peer_id) = crate::ConnectionGater::peer_id_from_addr(&addr) else {
            warn!(target: "gossip", peer=?addr, "Failed to extract PeerId from Multiaddr");
            return;
        };

        if self.swarm.connected_peers().any(|p| p == &peer_id) {
            debug!(target: "gossip", peer=?addr, "Already connected to peer, not dialing");
            kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "already_connected", "peer" => peer_id.to_string());
            return;
        }

        // Let the gate know we are dialing the address.
        self.connection_gate.dialing(&addr);

        // Dial
        match self.swarm.dial(addr.clone()) {
            Ok(_) => {
                trace!(target: "gossip", peer=?addr, "Dialed peer");
                self.connection_gate.dialed(&addr);
                kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER, "peer" => peer_id.to_string());
            }
            Err(e) => {
                error!(target: "gossip", "Failed to connect to peer: {:?}", e);
                self.connection_gate.remove_dial(&peer_id);
                kona_macros::inc!(gauge, crate::Metrics::DIAL_PEER_ERROR, "type" => "connection_error", "error" => e.to_string(), "peer" => peer_id.to_string());
            }
        }
    }

    fn handle_gossip_event(&mut self, event: Event) -> Option<OpNetworkPayloadEnvelope> {
        match event {
            Event::Gossipsub(e) => return self.handle_gossipsub_event(e),
            Event::Ping(libp2p::ping::Event { peer, result, .. }) => {
                trace!(target: "gossip", ?peer, ?result, "Ping received");
                if let Some(start_time) = self.peer_connection_start.get(&peer) {
                    let ping_duration = start_time.elapsed();
                    kona_macros::record!(
                        histogram,
                        crate::Metrics::GOSSIP_PEER_CONNECTION_DURATION_SECONDS,
                        ping_duration.as_secs_f64()
                    );
                }
                let pings = Arc::clone(&self.ping);
                tokio::spawn(async move {
                    if let Ok(time) = result {
                        pings.lock().await.insert(peer, time);
                    }
                });
            }
            Event::Identify(e) => self.handle_identify_event(e),
            // Don't do anything with stream events as this should be unreachable code.
            Event::Stream => {
                error!(target: "gossip", "Stream events should not be emitted!");
            }
        };

        None
    }

    fn handle_identify_event(&mut self, event: libp2p::identify::Event) {
        match event {
            libp2p::identify::Event::Received { connection_id, peer_id, info } => {
                debug!(target: "gossip", ?connection_id, ?peer_id, ?info, "Received identify info from peer");
                self.peer_infos.insert(peer_id, info);
            }
            libp2p::identify::Event::Sent { connection_id, peer_id } => {
                debug!(target: "gossip", ?connection_id, ?peer_id, "Sent identify info to peer");
            }
            libp2p::identify::Event::Pushed { connection_id, peer_id, info } => {
                debug!(target: "gossip", ?connection_id, ?peer_id, ?info, "Pushed identify info to peer");
            }
            libp2p::identify::Event::Error { connection_id, peer_id, error } => {
                error!(target: "gossip", ?connection_id, ?peer_id, ?error, "Error raised while attempting to identify remote");
            }
        }
    }

    /// Handles a [`libp2p::gossipsub::Event`].
    fn handle_gossipsub_event(
        &mut self,
        event: libp2p::gossipsub::Event,
    ) -> Option<OpNetworkPayloadEnvelope> {
        match event {
            libp2p::gossipsub::Event::Message {
                propagation_source: src,
                message_id: id,
                message,
            } => {
                trace!(target: "gossip", "Received message with topic: {}", message.topic);
                kona_macros::inc!(gauge, crate::Metrics::GOSSIP_EVENT, "type" => "message", "topic" => message.topic.to_string());
                if self.handler.topics().contains(&message.topic) {
                    let (status, payload) = self.handler.handle(message);
                    _ = self
                        .swarm
                        .behaviour_mut()
                        .gossipsub
                        .report_message_validation_result(&id, &src, status);
                    return payload;
                }
            }
            libp2p::gossipsub::Event::Subscribed { peer_id, topic } => {
                trace!(target: "gossip", "Peer: {:?} subscribed to topic: {:?}", peer_id, topic);
                kona_macros::inc!(gauge, crate::Metrics::GOSSIP_EVENT, "type" => "subscribed", "topic" => topic.to_string());
            }
            libp2p::gossipsub::Event::Unsubscribed { peer_id, topic } => {
                trace!(target: "gossip", "Peer: {:?} unsubscribed from topic: {:?}", peer_id, topic);
                kona_macros::inc!(gauge, crate::Metrics::GOSSIP_EVENT, "type" => "unsubscribed", "topic" => topic.to_string());
            }
            libp2p::gossipsub::Event::SlowPeer { peer_id, .. } => {
                trace!(target: "gossip", "Slow peer: {:?}", peer_id);
                kona_macros::inc!(gauge, crate::Metrics::GOSSIP_EVENT, "type" => "slow_peer", "peer" => peer_id.to_string());
            }
            libp2p::gossipsub::Event::GossipsubNotSupported { peer_id } => {
                trace!(target: "gossip", "Peer: {:?} does not support gossipsub", peer_id);
                kona_macros::inc!(gauge, crate::Metrics::GOSSIP_EVENT, "type" => "not_supported", "peer" => peer_id.to_string());
            }
        }
        None
    }

    /// Handles the [`SwarmEvent<Event>`].
    pub fn handle_event(&mut self, event: SwarmEvent<Event>) -> Option<OpNetworkPayloadEnvelope> {
        match event {
            SwarmEvent::Behaviour(behavior_event) => {
                return self.handle_gossip_event(behavior_event)
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                let peer_count = self.swarm.connected_peers().count();
                info!(target: "gossip", "Connection established: {:?} | Peer Count: {}", peer_id, peer_count);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIPSUB_CONNECTION,
                    "type" => "connected",
                    "peer" => peer_id.to_string(),
                );
                kona_macros::set!(gauge, crate::Metrics::GOSSIP_PEER_COUNT, peer_count as f64);

                self.peerstore.insert(peer_id, endpoint.get_remote_address().clone());
                self.peer_connection_start.insert(peer_id, Instant::now());
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                debug!(target: "gossip", "Outgoing connection error: {:?}", error);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIPSUB_CONNECTION,
                    "type" => "outgoing_error",
                    "peer" => peer_id.map(|p| p.to_string()).unwrap_or_default()
                );
            }
            SwarmEvent::IncomingConnectionError { error, connection_id, .. } => {
                debug!(target: "gossip", "Incoming connection error: {:?}", error);
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIPSUB_CONNECTION,
                    "type" => "incoming_error",
                    "connection_id" => connection_id.to_string()
                );
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                let peer_count = self.swarm.connected_peers().count();
                warn!(target: "gossip", ?peer_id, ?cause, peer_count, "Connection closed");
                kona_macros::inc!(
                    gauge,
                    crate::Metrics::GOSSIPSUB_CONNECTION,
                    "type" => "closed",
                    "peer" => peer_id.to_string()
                );
                kona_macros::set!(gauge, crate::Metrics::GOSSIP_PEER_COUNT, peer_count as f64);

                if let Some(start_time) = self.peer_connection_start.remove(&peer_id) {
                    let peer_duration = start_time.elapsed();
                    kona_macros::record!(
                        histogram,
                        crate::Metrics::GOSSIP_PEER_CONNECTION_DURATION_SECONDS,
                        peer_duration.as_secs_f64()
                    );
                }

                let pings = Arc::clone(&self.ping);
                tokio::spawn(async move {
                    pings.lock().await.remove(&peer_id);
                });

                // If the connection was initiated by us, remove the peer from the current dials
                // set so that we can dial it again.
                self.connection_gate.remove_dial(&peer_id);
            }
            SwarmEvent::NewListenAddr { listener_id, address } => {
                debug!(target: "gossip", reporter_id = ?listener_id, new_address = ?address, "New listen address");
            }
            SwarmEvent::Dialing { peer_id, connection_id } => {
                debug!(target: "gossip", ?peer_id, ?connection_id, "Dialing peer");
            }
            SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {
                debug!(target: "gossip", ?peer_id, ?address, "New external address of peer");
            }
            _ => {
                debug!(target: "gossip", ?event, "Ignoring non-behaviour in event handler");
            }
        };

        None
    }
}
