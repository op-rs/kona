//! Contains the extension for the supervisor actor.

use async_trait::async_trait;
use futures::Stream;
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
