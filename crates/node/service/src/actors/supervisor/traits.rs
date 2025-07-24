//! Contains the extension for the supervisor actor.

use async_trait::async_trait;
use futures::{Stream, stream};
use kona_interop::{ControlEvent, ManagedEvent};

/// The external supervisor interface.
#[async_trait]
pub trait SupervisorExt {
    /// The error type returned by the supervisor interface.
    type Error: std::error::Error + Send + Sync;

    /// Send a `ManagedEvent` to the supervisor.
    async fn send_event(&self, event: ManagedEvent) -> Result<(), Self::Error>;

    /// Subscribe to a stream of `ControlEvent`s from the supervisor.
    fn subscribe_control_events(&self) -> impl Stream<Item = ControlEvent> + Send;
}

/// A no-op implementation of [`SupervisorExt`] that does nothing.
/// This is used when supervisor functionality is disabled.
#[derive(Debug, Clone)]
pub struct NoOpSupervisorExt;

#[derive(Debug, thiserror::Error)]
#[error("No-op supervisor error")]
pub struct NoOpSupervisorError;

#[async_trait]
impl SupervisorExt for NoOpSupervisorExt {
    type Error = NoOpSupervisorError;

    async fn send_event(&self, _event: ManagedEvent) -> Result<(), Self::Error> {
        // No-op: do nothing
        Ok(())
    }

    fn subscribe_control_events(&self) -> impl Stream<Item = ControlEvent> + Send {
        // Return an empty stream that never yields any items
        stream::empty()
    }
}
