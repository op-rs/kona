//! [NodeActor] services for the node.
//!
//! [NodeActor]: super::NodeActor

use async_trait::async_trait;

mod derivation;
pub use derivation::{DerivationActor, DerivationError, InboundDerivationMessage};

mod l1_watcher_rpc;
pub use l1_watcher_rpc::{L1WatcherRpc, L1WatcherRpcError};

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
    type Error;

    /// Starts the actor.
    async fn start(self) -> Result<(), Self::Error>;

    /// Processes an incoming message.
    async fn process(&mut self, msg: Self::InboundEvent) -> Result<(), Self::Error>;
}
