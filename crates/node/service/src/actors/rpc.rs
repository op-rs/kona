//! RPC Server Actor

use crate::{NodeActor, actors::CancellableContext};
use async_trait::async_trait;
use jsonrpsee::core::RegisterMethodError;
use kona_rpc::{RpcBuilder, RpcLauncherError};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

/// An error returned by the [`RpcActor`].
#[derive(Debug, thiserror::Error)]
pub enum RpcActorError {
    /// Failed to register the healthz endpoint.
    #[error("Failed to register the healthz endpoint")]
    RegisterHealthz(#[from] RegisterMethodError),
    /// Failed to launch the RPC server.
    #[error("Failed to launch the RPC server")]
    LaunchFailed(#[from] RpcLauncherError),
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
    /// A launcher for the rpc.
    launcher: RpcBuilder,
}

impl RpcActor {
    /// Constructs a new [`RpcActor`] given the [`RpcBuilder`] and [`CancellationToken`].
    pub const fn new(launcher: RpcBuilder) -> Self {
        Self { launcher }
    }
}

/// The communication context used by the RPC actor.
#[derive(Debug)]
pub struct RpcContext {
    /// The cancellation token, shared between all tasks.
    pub cancellation: CancellationToken,
}

impl CancellableContext for RpcContext {
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}

#[async_trait]
impl NodeActor for RpcActor {
    type Error = RpcActorError;
    type InboundData = RpcContext;
    type OutboundData = ();
    type Builder = RpcBuilder;

    fn build(state: Self::Builder) -> (Self::OutboundData, Self) {
        ((), Self { launcher: state })
    }

    async fn start(
        mut self,
        RpcContext { cancellation }: Self::InboundData,
    ) -> Result<(), Self::Error> {
        let restarts = self.launcher.restart_count();

        let Some(mut handle) = self.launcher.clone().launch().await? else {
            // The RPC server is disabled, so we can return Ok.
            return Ok(());
        };

        for _ in 0..=restarts {
            tokio::select! {
                _ = handle.clone().stopped() => {
                    match self.launcher.clone().launch().await {
                        Ok(Some(h)) => handle = h,
                        Ok(None) => {
                            // The RPC server is disabled, so we can return Ok.
                            return Ok(());
                        }
                        Err(err) => {
                            error!(target: "rpc", ?err, "Failed to launch rpc server");
                            cancellation.cancel();
                            return Err(RpcActorError::ServerStopped);
                        }
                    }
                }
                _ = cancellation.cancelled() => {
                    // The cancellation token has been triggered, so we should stop the server.
                    handle.stop().map_err(|_| RpcActorError::StopFailed)?;
                    // Since the RPC Server didn't originate the error, we should return Ok.
                    return Ok(());
                }
            }
        }

        // Stop the node if there has already been 3 rpc restarts.
        cancellation.cancel();
        return Err(RpcActorError::ServerStopped);
    }
}
