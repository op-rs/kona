//! Event Handling Module.

use libp2p::{gossipsub, identify, ping};

/// The type of message received
#[derive(Debug)]
pub enum Event {
    /// Represents a [ping::Event]
    #[allow(dead_code)]
    Ping(ping::Event),
    /// Represents a [gossipsub::Event]
    Gossipsub(Box<gossipsub::Event>),
    /// Represents a [identify::Event]
    Identify(Box<identify::Event>),
    /// Stream event
    Stream,
}

impl From<ping::Event> for Event {
    /// Converts [ping::Event] to [Event]
    fn from(value: ping::Event) -> Self {
        Self::Ping(value)
    }
}

impl From<gossipsub::Event> for Event {
    /// Converts [gossipsub::Event] to [Event]
    fn from(value: gossipsub::Event) -> Self {
        Self::Gossipsub(Box::new(value))
    }
}

impl From<identify::Event> for Event {
    /// Converts [identify::Event] to [Event]
    fn from(value: identify::Event) -> Self {
        Self::Identify(Box::new(value))
    }
}

impl From<()> for Event {
    /// Converts () to [Event]
    fn from(_value: ()) -> Self {
        Self::Stream
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_conversion() {
        let gossipsub_event = libp2p::gossipsub::Event::Message {
            propagation_source: libp2p::PeerId::random(),
            message_id: libp2p::gossipsub::MessageId(vec![]),
            message: libp2p::gossipsub::Message {
                source: None,
                data: vec![],
                sequence_number: None,
                topic: libp2p::gossipsub::TopicHash::from_raw("test"),
            },
        };
        let event = Event::from(gossipsub_event);
        match event {
            Event::Gossipsub(e) => {
                if !matches!(*e, libp2p::gossipsub::Event::Message { .. }) {
                    panic!("Event conversion failed");
                }
            }
            _ => panic!("Event conversion failed"),
        }
    }

    #[test]
    fn test_event_conversion_ping() {
        let ping_event = ping::Event {
            peer: libp2p::PeerId::random(),
            connection: libp2p::swarm::ConnectionId::new_unchecked(0),
            result: Ok(core::time::Duration::from_secs(1)),
        };
        let event = Event::from(ping_event);
        match event {
            Event::Ping(_) => {}
            _ => panic!("Event conversion failed"),
        }
    }
}
