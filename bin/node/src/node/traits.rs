//! Traits for the core rollup node service.

use async_trait::async_trait;

/// The [NodeProducer] is an outbound-only service for the node. Generally, implementations of this
/// trait listen to external sources in order to generate events for [NodeActor]s to process.
#[async_trait]
pub trait NodeProducer {
    /// The event type emitted by the producer.
    type Event;
    /// The error type emitted by the producer.
    type Error;

    /// Starts the producer.
    async fn start(self) -> Result<(), Self::Error>;
}

/// The [NodeActor] is an actor-like service for the node.
///
/// Actors may:
/// - Handle incoming messages.
///     - Perform background tasks.
/// - Emit new events for other actors to process.
#[async_trait]
pub trait NodeActor {
    /// The event type emitted by the actor.
    type Event;
    /// The error type emitted by the actor.
    type Error;

    /// Starts the actor.
    async fn start(self) -> Result<(), Self::Error>;

    /// Processes an incoming message.
    async fn process(&mut self, msg: Self::Event) -> Result<(), Self::Error>;
}
