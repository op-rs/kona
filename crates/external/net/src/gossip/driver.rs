//! Consensus-layer gossipsub driver for Optimism.

use crate::gossip::{
    behaviour::Behaviour,
    event::Event,
    handler::{BlockHandler, Handler},
};
use futures::stream::StreamExt;
use libp2p::{
    swarm::{DialError, SwarmEvent},
    Multiaddr, Swarm, TransportError,
};
use tracing::{debug, error, info};

/// A [libp2p::Swarm] instance with an associated address to listen on.
pub struct GossipDriver {
    /// The [libp2p::Swarm] instance.
    pub swarm: Swarm<Behaviour>,
    /// The address to listen on.
    pub addr: Multiaddr,
    /// Block handler.
    pub handler: BlockHandler,
}

impl GossipDriver {
    /// Creates a new [GossipDriver] instance.
    pub fn new(swarm: Swarm<Behaviour>, addr: Multiaddr, handler: BlockHandler) -> Self {
        Self { swarm, addr, handler }
    }

    /// Listens on the address.
    pub fn listen(&mut self) -> Result<(), TransportError<std::io::Error>> {
        self.swarm.listen_on(self.addr.clone())?;
        info!("Swarm listening on: {:?}", self.addr);
        Ok(())
    }

    /// Returns a mutable reference to the Swarm's behaviour.
    pub fn behaviour_mut(&mut self) -> &mut Behaviour {
        self.swarm.behaviour_mut()
    }

    /// Attempts to select the next event from the Swarm.
    pub async fn select_next_some(&mut self) -> SwarmEvent<Event> {
        self.swarm.select_next_some().await
    }

    /// Returns the number of connected peers.
    pub fn connected_peers(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// Dials the given [`Option<Multiaddr>`].
    pub async fn dial_opt(&mut self, peer: Option<impl Into<Multiaddr>>) {
        let Some(addr) = peer else {
            return;
        };
        match self.dial(addr).await {
            Ok(_) => info!("Dialed peer"),
            Err(e) => error!("Failed to dial peer: {:?}", e),
        }
    }

    /// Dials the given [Multiaddr].
    pub async fn dial(&mut self, peer: impl Into<Multiaddr>) -> Result<(), DialError> {
        let addr: Multiaddr = peer.into();
        self.swarm.dial(addr)?;
        Ok(())
    }

    /// Handles the [`SwarmEvent<Event>`].
    pub fn handle_event(&mut self, event: SwarmEvent<Event>) {
        if let SwarmEvent::Behaviour(Event::Gossipsub(libp2p::gossipsub::Event::Message {
            propagation_source: src,
            message_id: id,
            message,
        })) = event
        {
            debug!("Received message with topic: {}", message.topic);
            if self.handler.topics().contains(&message.topic) {
                debug!("Handling message with topic: {}", message.topic);
                let status = self.handler.handle(message);
                debug!("Reporting message validation result: {:?}", status);
                _ = self
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .report_message_validation_result(&id, &src, status);
            }
        }
    }
}
