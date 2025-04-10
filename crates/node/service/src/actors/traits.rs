//! [NodeActor] trait.

use async_trait::async_trait;

/// The [NodeActor] is an actor-like service for the node.
///
/// Actors may:
/// - Handle incoming messages.
///     - Perform background tasks.
/// - Emit new events for other actors to process.
#[async_trait]
pub trait NodeActor {
    /// The event type received by the actor.
    type InboundEvent;
    /// The error type for the actor.
    type Error: std::fmt::Debug;

    /// Starts the actor.
    async fn start(self) -> Result<(), Self::Error>;

    /// Processes an incoming message.
    async fn process(&mut self, msg: Self::InboundEvent) -> Result<(), Self::Error>;
}
