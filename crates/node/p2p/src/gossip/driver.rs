//! Consensus-layer gossipsub driver for Optimism.

use futures::stream::StreamExt;
use libp2p::{
    Multiaddr, Swarm, TransportError,
    swarm::{DialError, SwarmEvent, dial_opts::DialOpts},
};

use crate::{Behaviour, BlockHandler, Event, Handler};

/// A driver for a [`Swarm`] instance.
///
/// Connects the swarm to the given [`Multiaddr`]
/// and handles events using the [`BlockHandler`].
pub struct GossipDriver {
    /// The [`Swarm`] instance.
    pub swarm: Swarm<Behaviour>,
    /// A [`Multiaddr`] to listen on.
    pub addr: Multiaddr,
    /// The [`BlockHandler`].
    pub handler: BlockHandler,
}

impl GossipDriver {
    /// Creates a new [`GossipDriver`] instance.
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

    /// Dials the given [`Option<DialOpts>`].
    pub async fn dial_opt(&mut self, opts: Option<impl Into<DialOpts>>) {
        let Some(opts) = opts else {
            return;
        };
        match self.dial(opts).await {
            Ok(_) => info!("Dialed peer"),
            Err(e) => error!("Failed to dial peer: {:?}", e),
        }
    }

    /// Dials the given [`DialOpts`].
    pub async fn dial(&mut self, opts: impl Into<DialOpts>) -> Result<(), DialError> {
        self.swarm.dial(opts)?;
        Ok(())
    }

    /// Handles the [`SwarmEvent<Event>`].
    pub fn handle_event(&mut self, event: SwarmEvent<Event>) {
        debug!(target: "p2p::gossip", "Handling event: {:?}", event);
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
