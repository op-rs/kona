//! Contains an implementation of the [`SupervisorExt`] trait for
//! [`kona_rpc::SupervisorRpcServer`].

use crate::SupervisorExt;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use kona_interop::{ControlEvent, ManagedEvent};

/// The external supervisor rpc server.
#[derive(Debug)]
pub struct SupervisorRpcServerExt {
    /// Contains the handle to the supervisor rpc server.
    #[allow(dead_code)]
    handle: jsonrpsee::server::ServerHandle,
    /// A broadcast sender to forward managed events from the node to the rpc server.
    managed_events_tx: tokio::sync::broadcast::Sender<ManagedEvent>,
    /// A broadcast channel to receive control events from the rpc.
    engine_control: tokio::sync::broadcast::Receiver<ControlEvent>,
}

impl SupervisorRpcServerExt {
    /// Constructs a new [`SupervisorRpcServerExt`].
    pub const fn new(
        handle: jsonrpsee::server::ServerHandle,
        managed_events_tx: tokio::sync::broadcast::Sender<ManagedEvent>,
        engine_control: tokio::sync::broadcast::Receiver<ControlEvent>,
    ) -> Self {
        Self { handle, managed_events_tx, engine_control }
    }
}

#[async_trait]
impl SupervisorExt for SupervisorRpcServerExt {
    type Error = tokio::sync::broadcast::error::SendError<ManagedEvent>;

    async fn send_event(&self, event: ManagedEvent) -> Result<(), Self::Error> {
        self.managed_events_tx.send(event).map(|_| ())
    }

    fn subscribe_control_events(&self) -> impl Stream<Item = ControlEvent> + Send {
        tokio_stream::wrappers::BroadcastStream::new(self.engine_control.resubscribe())
            .filter_map(|event| async move { event.ok() })
    }
}
