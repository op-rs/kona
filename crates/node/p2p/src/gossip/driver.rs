//! Consensus-layer gossipsub driver for Optimism.

use derive_more::Debug;
use discv5::Enr;
use futures::stream::StreamExt;
use libp2p::{
    Multiaddr, PeerId, Swarm, TransportError,
    gossipsub::{IdentTopic, MessageId},
    swarm::SwarmEvent,
};
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::collections::HashMap;

use crate::{
    Behaviour, BlockHandler, Event, GossipDriverBuilder, Handler, OpStackEnr, PublishError,
    enr_to_multiaddr,
};

/// A driver for a [`Swarm`] instance.
///
/// Connects the swarm to the given [`Multiaddr`]
/// and handles events using the [`BlockHandler`].
#[derive(Debug)]
pub struct GossipDriver {
    /// The [`Swarm`] instance.
    #[debug(skip)]
    pub swarm: Swarm<Behaviour>,
    /// A [`Multiaddr`] to listen on.
    pub addr: Multiaddr,
    /// The [`BlockHandler`].
    pub handler: BlockHandler,
    /// A list of peers we have successfully dialed.
    pub dialed_peers: HashMap<Multiaddr, bool>,
    /// A mapping from [`PeerId`] to [`Multiaddr`].
    pub peerstore: HashMap<PeerId, Multiaddr>,
}

impl GossipDriver {
    /// Returns the [`GossipDriverBuilder`] that can be used to construct the [`GossipDriver`].
    pub const fn builder() -> GossipDriverBuilder {
        GossipDriverBuilder::new()
    }

    /// Creates a new [`GossipDriver`] instance.
    pub fn new(swarm: Swarm<Behaviour>, addr: Multiaddr, handler: BlockHandler) -> Self {
        Self {
            swarm,
            addr,
            handler,
            dialed_peers: Default::default(),
            peerstore: Default::default(),
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
            debug!("Ignoring empty payload");
            return Ok(None);
        };
        let topic = selector(&self.handler);
        let topic_hash = topic.hash();
        let data = self.handler.encode(topic, payload)?;
        let id = self.swarm.behaviour_mut().gossipsub.publish(topic_hash, data)?;
        Ok(Some(id))
    }

    /// Listens on the address.
    pub fn listen(&mut self) -> Result<(), TransportError<std::io::Error>> {
        self.swarm.listen_on(self.addr.clone())?;
        info!(target: "p2p::gossip::driver", "Swarm listening on: {:?}", self.addr);
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
    pub async fn select_next_some(&mut self) -> SwarmEvent<Event> {
        self.swarm.select_next_some().await
    }

    /// Returns if the swarm has already successfully dialed the given [`Multiaddr`].
    pub fn has_dialed(&mut self, addr: &Multiaddr) -> bool {
        self.dialed_peers.get(addr).copied().unwrap_or(false)
    }

    /// Returns the number of connected peers.
    pub fn connected_peers(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// Dials the given [`Enr`].
    pub fn dial(&mut self, enr: Enr) {
        let key = OpStackEnr::OP_CL_KEY.as_bytes();
        if enr.get_raw_rlp(key).is_none() {
            debug!("Ignoring peer without opstack CL key...");
            return;
        }
        let Some(multiaddr) = enr_to_multiaddr(&enr) else {
            debug!("Failed to extract tcp socket from enr: {:?}", enr);
            return;
        };
        self.dial_multiaddr(multiaddr);
    }

    /// Dials the given [`Multiaddr`].
    pub fn dial_multiaddr(&mut self, addr: Multiaddr) {
        if self.has_dialed(&addr) {
            event!(tracing::Level::TRACE, peer=%addr, "Already connected to peer");
            return;
        }

        match self.swarm.dial(addr.clone()) {
            Ok(_) => {
                event!(tracing::Level::TRACE, peer=%addr, "Dialed peer");
                self.dialed_peers.insert(addr, true);
            }
            Err(e) => {
                debug!("Failed to connect to peer: {:?}", e);
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
                debug!("Received message with topic: {}", message.topic);
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
                debug!(target: "p2p::gossip::driver", "Peer: {:?} subscribed to topic: {:?}", peer_id, topic);
                // TODO: Metrice a peer subscribing to a topic?
            }
            libp2p::gossipsub::Event::SlowPeer { peer_id, .. } => {
                debug!(target: "p2p::gossip::driver", "Slow peer: {:?}", peer_id);
                // TODO: Metrice slow peer count
            }
            _ => {
                debug!("Ignoring non-message gossipsub event: {:?}", event)
            }
        }
        None
    }

    /// Redials the given [`PeerId`] using the peerstore.
    pub fn redial(&mut self, peer_id: PeerId) {
        if let Some(addr) = self.peerstore.get(&peer_id) {
            self.dialed_peers.remove(addr);
            trace!("Redialing peer with id: {:?}", peer_id);
            self.dial_multiaddr(addr.clone());
        }
    }

    /// Handles the [`SwarmEvent<Event>`].
    pub fn handle_event(&mut self, event: SwarmEvent<Event>) -> Option<OpNetworkPayloadEnvelope> {
        if let SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } = event {
            self.peerstore.insert(peer_id, endpoint.get_remote_address().clone());
            return None;
        }
        if let SwarmEvent::OutgoingConnectionError { peer_id, error, .. } = event {
            trace!("Outgoing connection error: {:?}", error);
            if let Some(id) = peer_id {
                self.redial(id);
            }
            return None;
        }
        if let SwarmEvent::ConnectionClosed { peer_id, cause, .. } = event {
            trace!("Connection closed, redialing peer: {:?} | {:?}", peer_id, cause);
            self.redial(peer_id);
            return None;
        }
        let SwarmEvent::Behaviour(event) = event else {
            trace!("Ignoring non-behaviour in event handler: {:?}", event);
            return None;
        };

        match event {
            Event::Ping(libp2p::ping::Event { peer, result, .. }) => {
                trace!("Ping from peer: {:?} | Result: {:?}", peer, result);
                None
            }
            Event::Gossipsub(e) => self.handle_gossipsub_event(e),
        }
    }
}
