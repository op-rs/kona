//! Network Behaviour Module.

use derive_more::Debug;
use libp2p::{
    gossipsub::{Config, IdentTopic, MessageAuthenticity},
    swarm::NetworkBehaviour,
};

use crate::{Event, Handler};

/// An error that can occur when creating a [`Behaviour`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BehaviourError {
    /// The gossipsub behaviour creation failed.
    #[error("gossipsub behaviour creation failed")]
    GossipsubCreationFailed,
    /// Subscription failed.
    #[error("subscription failed")]
    SubscriptionFailed,
    /// Failed to set the peer score on the gossipsub.
    #[error("{0}")]
    PeerScoreFailed(String),
}

/// Specifies the [`NetworkBehaviour`] of the node
#[derive(NetworkBehaviour, Debug)]
#[behaviour(out_event = "Event")]
pub struct Behaviour {
    /// Responds to inbound pings and send outbound pings.
    #[debug(skip)]
    pub ping: libp2p::ping::Behaviour,
    /// Enables gossipsub as the routing layer.
    pub gossipsub: libp2p::gossipsub::Behaviour,
}

impl Behaviour {
    /// Configures the swarm behaviors, subscribes to the gossip topics, and returns a new
    /// [`Behaviour`].
    pub fn new(cfg: Config, handlers: &[Box<dyn Handler>]) -> Result<Self, BehaviourError> {
        let ping = libp2p::ping::Behaviour::default();

        let mut gossipsub = libp2p::gossipsub::Behaviour::new(MessageAuthenticity::Anonymous, cfg)
            .map_err(|_| BehaviourError::GossipsubCreationFailed)?;

        let subscriptions = handlers
            .iter()
            .flat_map(|handler| {
                handler
                    .topics()
                    .iter()
                    .map(|topic| {
                        let topic = IdentTopic::new(topic.to_string());
                        let res = gossipsub
                            .subscribe(&topic)
                            .map_err(|_| BehaviourError::SubscriptionFailed);
                        if res.is_ok() {
                            info!("libp2p switch subscribed to topic: {}", topic.to_string());
                        }
                        res
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Result<Vec<bool>, BehaviourError>>()?;

        for sub in subscriptions {
            if !sub {
                return Err(BehaviourError::SubscriptionFailed);
            }
        }

        Ok(Self { ping, gossipsub })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gossip::{config, handler::BlockHandler};
    use alloy_primitives::Address;
    use libp2p::gossipsub::{IdentTopic, TopicHash};

    fn zero_topics() -> Vec<TopicHash> {
        vec![
            IdentTopic::new("/optimism/0/0/blocks").hash(),
            IdentTopic::new("/optimism/0/1/blocks").hash(),
            IdentTopic::new("/optimism/0/2/blocks").hash(),
            IdentTopic::new("/optimism/0/3/blocks").hash(),
        ]
    }

    #[test]
    fn test_behaviour_no_handlers() {
        let cfg = config::default_config();
        let handlers = vec![];
        let _ = Behaviour::new(cfg, &handlers).unwrap();
    }

    #[test]
    fn test_behaviour_with_handlers() {
        let cfg = config::default_config();
        let (_, recv) = tokio::sync::watch::channel(Address::default());
        let block_handler = BlockHandler::new(0, recv);
        let handlers: Vec<Box<dyn Handler>> = vec![Box::new(block_handler)];
        let behaviour = Behaviour::new(cfg, &handlers).unwrap();
        let mut topics = behaviour.gossipsub.topics().cloned().collect::<Vec<TopicHash>>();
        topics.sort();
        assert_eq!(topics, zero_topics());
    }
}
