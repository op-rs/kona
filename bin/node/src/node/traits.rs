//! Traits for the core rollup node service.

use async_trait::async_trait;

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
