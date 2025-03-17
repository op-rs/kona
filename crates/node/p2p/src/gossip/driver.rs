//! Consensus-layer gossipsub driver for Optimism.

use discv5::Enr;
use futures::stream::StreamExt;
use libp2p::{Multiaddr, Swarm, TransportError, swarm::SwarmEvent};

use crate::{Behaviour, BlockHandler, Event, Handler, OpStackEnr, enr_to_multiaddr};

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

    /// Returns if the swarm is already connected to the specified [`Multiaddr`].
    pub fn is_connected(&mut self, addr: &Multiaddr) -> bool {
        self.swarm.external_addresses().any(|a| *a == *addr)
    }

    /// Returns the number of connected peers.
    pub fn connected_peers(&self) -> usize {
        self.swarm.connected_peers().count()
    }

    /// Dials the given [`Enr`].
    pub fn dial(&mut self, enr: Enr) {
        let key = OpStackEnr::OP_CL_KEY.as_bytes();
        if enr.get_raw_rlp(key).is_none() {
            debug!(target: "p2p::gossip::driver", "Ignoring peer without opstack CL key...");
            return;
        }
        let Some(multiaddr) = enr_to_multiaddr(&enr) else {
            debug!(target: "p2p::gossip::driver", "Failed to extract tcp socket from enr: {:?}", enr);
            return;
        };
        self.dial_multiaddr(multiaddr);
    }

    /// Dials the given [`Multiaddr`].
    pub fn dial_multiaddr(&mut self, addr: Multiaddr) {
        if self.is_connected(&addr) {
            warn!(target: "p2p::gossip::driver", "Already connected to peer: {:?}", addr.clone());
            return;
        }

        match self.swarm.dial(addr.clone()) {
            Ok(_) => trace!(target: "p2p::gossip::driver", "Dialed peer: {:?}", addr),
            Err(e) => {
                debug!(target: "p2p::gossip::driver", "Failed to connect to peer: {:?}", e);
            }
        }
    }

    /// Handles a [`libp2p::gossipsub::Event`].
    fn handle_gossipsub_event(&mut self, event: libp2p::gossipsub::Event) {
        match event {
            libp2p::gossipsub::Event::Message {
                propagation_source: src,
                message_id: id,
                message,
            } => {
                trace!(target: "p2p::gossip::driver", "Received message with topic: {}", message.topic);
                if self.handler.topics().contains(&message.topic) {
                    debug!(target: "p2p::gossip::driver", "Handling message with topic: {}", message.topic);
                    let status = self.handler.handle(message);
                    debug!(target: "p2p::gossip::driver", "Reporting message validation result: {:?}", status);
                    _ = self
                        .swarm
                        .behaviour_mut()
                        .gossipsub
                        .report_message_validation_result(&id, &src, status);
                }
            }
            _ => {
                warn!(target: "p2p::gossip::driver", "Ignoring non-message gossipsub event: {:?}", event)
            }
        }
    }

    /// Handles the [`SwarmEvent<Event>`].
    pub fn handle_event(&mut self, event: SwarmEvent<Event>) {
        let SwarmEvent::Behaviour(event) = event else {
            warn!(target: "p2p::gossip::driver", "Ignoring non-behaviour in event handler: {:?}", event);
            return;
        };

        match event {
            Event::Ping(libp2p::ping::Event { peer, result, .. }) => {
                trace!(target: "p2p::gossip::driver", "Ping from peer: {:?} | Result: {:?}", peer, result);
            }
            Event::Gossipsub(e) => self.handle_gossipsub_event(e),
        }
    }
}
