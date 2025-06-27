//! Contains an actor for the supervisor rpc api.

use crate::{NodeActor, SupervisorExt, actors::CancellableContext};
use async_trait::async_trait;
use futures::StreamExt;
use kona_interop::{ControlEvent, ManagedEvent};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

/// The supervisor actor.
///
/// This actor routes `ManagedEvent`s from the kona node
/// a trait-abstracted component that communicates with the supervisor
/// either in-memory or through an external-facing rpc.
///
/// See: <https://specs.optimism.io/interop/managed-mode.html>
#[derive(Debug)]
pub struct SupervisorActor<E: SupervisorExt> {
    /// The module to communicate with the supervisor.
    supervisor_ext: E,
    /// A channel to communicate with the engine.
    engine_control: mpsc::Sender<ControlEvent>,
}

/// The outbound data for the supervisor actor.
#[derive(Debug)]
pub struct SupervisorOutboundData {
    /// A channel to communicate with the engine.
    /// For now, we don't support sending control events to the engine.
    #[allow(dead_code)]
    engine_control: mpsc::Receiver<ControlEvent>,
}

/// The communication context used by the supervisor actor.
#[derive(Debug)]
pub struct SupervisorActorContext {
    /// A channel to receive `ManagedEvent`s from the kona node.
    node_events: mpsc::Receiver<ManagedEvent>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl CancellableContext for SupervisorActorContext {
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}

impl<E> SupervisorActor<E>
where
    E: SupervisorExt,
{
    /// Creates a new instance of the supervisor actor.
    pub fn new(supervisor_ext: E) -> (SupervisorOutboundData, Self) {
        let (engine_control_tx, engine_control_rx) = mpsc::channel(1024);
        let actor = Self { supervisor_ext, engine_control: engine_control_tx };
        let outbound_data = SupervisorOutboundData { engine_control: engine_control_rx };
        (outbound_data, actor)
    }
}

#[async_trait]
impl<E> NodeActor for SupervisorActor<E>
where
    E: SupervisorExt + Send + Sync + 'static,
{
    type Error = SupervisorActorError;
    type InboundData = SupervisorActorContext;
    type OutboundData = SupervisorOutboundData;
    type Builder = E;

    fn build(state: Self::Builder) -> (Self::OutboundData, Self) {
        Self::new(state)
    }

    async fn start(
        mut self,
        SupervisorActorContext { mut node_events, cancellation }: Self::InboundData,
    ) -> Result<(), Self::Error> {
        let mut control_events = Box::pin(self.supervisor_ext.subscribe_control_events());
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    warn!(target: "supervisor", "Supervisor actor cancelled");
                    return Ok(());
                },
                Some(event) = node_events.recv() => {
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
}

/// The error type for the [`SupervisorActor`].
#[derive(Error, Debug, Clone)]
pub enum SupervisorActorError {
    /// Failed to send a control event to the engine.
    #[error("Failed to send control event to engine")]
    ControlEventSendFailed,
}
