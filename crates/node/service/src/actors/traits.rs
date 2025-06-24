//! [NodeActor] trait.

use async_trait::async_trait;
use tokio_util::sync::WaitForCancellationFuture;

/// The communication context used by the actor.
pub trait ActorContext: Send {
    /// Returns a future that resolves when the actor is cancelled.
    fn cancelled(&self) -> WaitForCancellationFuture<'_>;
}

/// The [NodeActor] is an actor-like service for the node.
///
/// Actors may:
/// - Handle incoming messages.
///     - Perform background tasks.
/// - Emit new events for other actors to process.
#[async_trait]
pub trait NodeActor {
    /// The error type for the actor.
    type Error: std::fmt::Debug;
    /// The communication context used by the actor.
    type Context: ActorContext;

    /// Starts the actor.
    async fn start(self, context: Self::Context) -> Result<(), Self::Error>;
}
