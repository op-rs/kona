//! RPC Server Actor

use crate::NodeActor;
use async_trait::async_trait;
use jsonrpsee::core::RegisterMethodError;
use kona_rpc::{RpcLauncher, RpcLauncherError};
use tokio_util::sync::CancellationToken;

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
    launcher: RpcLauncher,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl RpcActor {
    /// Constructs a new [`RpcActor`] given the [`RpcLauncher`] and [`CancellationToken`].
    pub const fn new(launcher: RpcLauncher, cancellation: CancellationToken) -> Self {
        Self { launcher, cancellation }
    }
}

#[async_trait]
impl NodeActor for RpcActor {
    type InboundEvent = ();
    type Error = RpcActorError;

    async fn start(mut self) -> Result<(), Self::Error> {
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
                            self.cancellation.cancel();
                            return Err(RpcActorError::ServerStopped);
                        }
                    }
                }
                _ = self.cancellation.cancelled() => {
                    // The cancellation token has been triggered, so we should stop the server.
                    handle.stop().map_err(|_| RpcActorError::StopFailed)?;
                    // Since the RPC Server didn't originate the error, we should return Ok.
                    return Ok(());
                }
            }
        }

        // Stop the node if there has already been 3 rpc restarts.
        self.cancellation.cancel();
        return Err(RpcActorError::ServerStopped);
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}
