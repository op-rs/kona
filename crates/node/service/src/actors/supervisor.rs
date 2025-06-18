//! Contains an actor for the supervisor rpc api.

use crate::NodeActor;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use kona_interop::{ControlEvent, ManagedEvent};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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

/// The supervisor actor.
///
/// This actor routes `ManagedEvent`s from the kona node
/// a trait-abstracted component that communicates with the supervisor
/// either in-memory or through an external-facing rpc.
///
/// See: <https://specs.optimism.io/interop/managed-mode.html>
#[derive(Debug)]
pub struct SupervisorActor<E: SupervisorExt> {
    /// A channel to receive `ManagedEvent`s from the kona node.
    node_events: mpsc::Receiver<ManagedEvent>,
    /// A channel to communicate with the engine.
    engine_control: mpsc::Sender<ControlEvent>,
    /// The module to communicate with the supervisor.
    supervisor_ext: E,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl<E> SupervisorActor<E>
where
    E: SupervisorExt,
{
    /// Creates a new instance of the supervisor actor.
    pub const fn new(
        node_events: mpsc::Receiver<ManagedEvent>,
        engine_control: mpsc::Sender<ControlEvent>,
        supervisor_ext: E,
        cancellation: CancellationToken,
    ) -> Self {
        Self { supervisor_ext, node_events, engine_control, cancellation }
    }
}

#[async_trait]
impl<E> NodeActor for SupervisorActor<E>
where
    E: SupervisorExt + Send + Sync,
{
    type InboundEvent = ();
    type Error = SupervisorActorError;

    async fn start(mut self) -> Result<(), Self::Error> {
        let mut control_events = Box::pin(self.supervisor_ext.subscribe_control_events());
        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    warn!(target: "supervisor", "Supervisor actor cancelled");
                    return Ok(());
                },
                Some(event) = self.node_events.recv() => {
                    if let Err(err) = self.supervisor_ext.send_event(event).await {
                        error!(target: "supervisor", ?err, "Failed to send event to supervisor");
                    }
                },
                // Wait for a control event from the supervisor.
                Some(control_event) = control_events.next() => {
                    // TODO: Handle the control event (e.g., restart, stop, etc.).
                    debug!(target: "supervisor", "Received control event: {:?}", control_event);
                    self.engine_control
                        .send(control_event)
                        .await
                        .map_err(|_| SupervisorActorError::ControlEventSendFailed)?;
                },
            }
        }
    }

    async fn process(&mut self, _msg: Self::InboundEvent) -> Result<(), Self::Error> {
        // The supervisor actor does not process any incoming messages.
        Ok(())
    }
}

/// The error type for the [`SupervisorActor`].
#[derive(Error, Debug, Clone)]
pub enum SupervisorActorError {
    /// Failed to send a control event to the engine.
    #[error("Failed to send control event to engine")]
    ControlEventSendFailed,
}
