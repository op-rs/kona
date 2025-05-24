//! RPC Server Actor

use crate::NodeActor;
use async_trait::async_trait;
use jsonrpsee::server::ServerHandle;
use tokio_util::sync::CancellationToken;

/// An error returned by the [`RpcActor`].
#[derive(Debug, thiserror::Error)]
pub enum RpcActorError {
    /// The [`RpcActor`]'s RPC server stopped unexpectedly.
    #[error("RPC server stopped unexpectedly")]
    ServerStopped,
    /// Failed to stop the RPC server.
    #[error("Failed to stop the RPC server")]
    StopFailed,
}

/// An actor that handles the RPC server for the rollup node.
#[derive(Debug)]
pub struct RpcActor {
    /// The handle to the RPC server.
    server: ServerHandle,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl RpcActor {
    /// Constructs a new [`RpcActor`] given the [`ServerHandle`] and [`CancellationToken`].
    pub const fn new(server: ServerHandle, cancellation: CancellationToken) -> Self {
        Self { server, cancellation }
    }
}

#[async_trait]
impl NodeActor for RpcActor {
    type InboundEvent = ();
    type Error = RpcActorError;

    async fn start(mut self) -> Result<(), Self::Error> {
        let server = self.server.clone();
        tokio::select! {
            _ = server.stopped() => {
                // The server has stopped, so we should stop as well.
                self.cancellation.cancel();
                return Err(RpcActorError::ServerStopped);
            }
            _ = self.cancellation.cancelled() => {
                // The cancellation token has been triggered, so we should stop the server.
                self.server.stop().map_err(|_| RpcActorError::StopFailed)?;
                // Since the RPC Server didn't originate the error, we should return Ok.
                return Ok(());
            }
        }
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}
