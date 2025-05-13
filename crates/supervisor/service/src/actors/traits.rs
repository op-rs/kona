//! [SupervisorActor] trait.

use async_trait::async_trait;

/// The [`SupervisorActor`] trait is an actor-like service for the supervisor.
#[async_trait]
pub trait SupervisorActor {
    /// The event type received by the actor.
    type InboundEvent;
    /// The error type for the actor.
    type Error: std::fmt::Debug;

    /// Starts the actor.
    async fn start(self) -> Result<(), Self::Error>;

    /// Processes an incoming message.
    async fn process(&mut self, msg: Self::InboundEvent) -> Result<(), Self::Error>;
}
