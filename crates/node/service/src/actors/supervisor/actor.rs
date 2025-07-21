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
    /// A channel to receive `ManagedEvent`s from the kona node.
    node_events: mpsc::Receiver<ManagedEvent>,
}

/// The inbound data for the supervisor actor.
#[derive(Debug)]
pub struct SupervisorInboundData {
    /// A channel to send managed events to the supervisor.
    /// TODO(@theochap, `<https://github.com/op-rs/kona/issues/2320>`): plug it in the rest of the rollup service.
    #[allow(dead_code)]
    node_events: mpsc::Sender<ManagedEvent>,
}

/// The communication context used by the supervisor actor.
#[derive(Debug)]
pub struct SupervisorActorContext {
    /// A channel to communicate with the engine.
    /// For now, we don't support sending control events to the engine.
    #[allow(dead_code)]
    engine_control: mpsc::Sender<ControlEvent>,
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
    pub fn new(supervisor_ext: E) -> (SupervisorInboundData, Self) {
        let (node_events_tx, node_events_rx) = mpsc::channel(1024);
        let actor = Self { supervisor_ext, node_events: node_events_rx };
        let outbound_data = SupervisorInboundData { node_events: node_events_tx };
        (outbound_data, actor)
    }
}

#[async_trait]
impl<E> NodeActor for SupervisorActor<E>
where
    E: SupervisorExt + Send + Sync + 'static,
{
    type Error = SupervisorActorError;
    type InboundData = SupervisorInboundData;
    type OutboundData = SupervisorActorContext;
    type Builder = E;

    fn build(state: Self::Builder) -> (Self::InboundData, Self) {
        Self::new(state)
    }

    async fn start(
        mut self,
        SupervisorActorContext { engine_control, cancellation }: Self::OutboundData,
    ) -> Result<(), Self::Error> {
        let mut control_events = Box::pin(self.supervisor_ext.subscribe_control_events());

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
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
                    engine_control
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
