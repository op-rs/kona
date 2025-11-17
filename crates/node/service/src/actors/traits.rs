//! [NodeActor] trait.

use async_trait::async_trait;
use tokio_util::sync::WaitForCancellationFuture;

/// The communication context used by the actor.
pub trait CancellableContext: Send {
    /// Returns a future that resolves when the actor is cancelled.
    fn cancelled(&self) -> WaitForCancellationFuture<'_>;
}

/// The [NodeActor] is an actor-like service for the node.
///
/// Actors may:
/// - Handle incoming messages.
///     - Perform background tasks.
/// - Emit new events for other actors to process.
///
/// The types and functions that are part of a `NodeActor` imply a specific lifecycle:
/// 1. Build: Create the Actor, passing in configuration that is known before other actors are
///    created. Actors cannot have any dependencies on each other at `build` time.
/// 2. Init: Create the struct necessary to call `start`. This is after Build because the `start`
///    logic for one actor may depend on data structures that were created by other actors when
///    their `build` function was called. These dependencies are bundled into `StartData` during
///    `init`.
/// 3. Start: The entrypoint into the long-running actor logic.
#[async_trait]
pub trait NodeActor: Send + 'static {
    /// The error type for the actor.
    type Error: std::fmt::Debug;
    /// The type necessary to pass to the start function.
    /// This is the result of
    type StartData: Sized;

    /// Starts the actor.
    async fn start(self, start_context: Self::StartData) -> Result<(), Self::Error>;
}
