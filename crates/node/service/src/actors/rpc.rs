//! RPC Server Actor

use crate::{NodeActor, actors::CancellableContext};
use async_trait::async_trait;
use kona_p2p::P2pRpcRequest;
use kona_rpc::{HealthzResponse, OpP2PApiServer, RollupNodeApiServer, WsRPC, WsServer};

use jsonrpsee::{
    RpcModule,
    core::RegisterMethodError,
    server::{Server, ServerHandle},
};
use kona_engine::EngineQueries;
use kona_rpc::{L1WatcherQueries, NetworkRpc, RollupRpc, RpcBuilder};
use tokio::sync::mpsc;
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

/// An error returned by the [`RpcActor`].
#[derive(Debug, thiserror::Error)]
pub enum RpcActorError {
    /// Failed to register the healthz endpoint.
    #[error("Failed to register the healthz endpoint")]
    RegisterHealthz(#[from] RegisterMethodError),
    /// Failed to launch the RPC server.
    #[error(transparent)]
    LaunchFailed(#[from] std::io::Error),
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
    config: RpcBuilder,
}

impl RpcActor {
    /// Constructs a new [`RpcActor`] given the [`RpcBuilder`].
    pub const fn new(config: RpcBuilder) -> Self {
        Self { config }
    }
}

/// The communication context used by the RPC actor.
#[derive(Debug)]
pub struct RpcContext {
    /// The network rpc sender.
    pub network: mpsc::Sender<P2pRpcRequest>,
    /// The l1 watcher queries sender.
    pub l1_watcher_queries: mpsc::Sender<L1WatcherQueries>,
    /// The engine query sender.
    pub engine_query: mpsc::Sender<EngineQueries>,
    /// The cancellation token, shared between all tasks.
    pub cancellation: CancellationToken,
}

impl CancellableContext for RpcContext {
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}

/// Launches the jsonrpsee [`Server`].
///
/// If the RPC server is disabled, this will return `Ok(None)`.
///
/// ## Errors
///
/// - [`std::io::Error`] if the server fails to start.
async fn launch(
    config: &RpcBuilder,
    module: RpcModule<()>,
) -> Result<Option<ServerHandle>, std::io::Error> {
    if config.disabled {
        return Ok(None);
    }

    let server = Server::builder().build(config.socket).await?;
    Ok(Some(server.start(module)))
}

#[async_trait]
impl NodeActor for RpcActor {
    type Error = RpcActorError;
    type OutboundData = RpcContext;
    type InboundData = ();
    type Builder = RpcBuilder;

    fn build(config: Self::Builder) -> (Self::InboundData, Self) {
        ((), Self::new(config))
    }

    async fn start(
        mut self,
        RpcContext { cancellation, network, l1_watcher_queries, engine_query }: Self::OutboundData,
    ) -> Result<(), Self::Error> {
        let mut modules = RpcModule::new(());

        modules.register_method("healthz", |_, _, _| {
            let response = HealthzResponse { version: std::env!("CARGO_PKG_VERSION").to_string() };
            jsonrpsee::core::RpcResult::Ok(response)
        })?;

        modules.merge(NetworkRpc::new(network).into_rpc())?;

        // Create context for communication between actors.
        let rollup_rpc = RollupRpc::new(engine_query.clone(), l1_watcher_queries);
        modules.merge(rollup_rpc.into_rpc())?;

        if self.config.ws_enabled() {
            modules.merge(WsRPC::new(engine_query).into_rpc())?;
        }

        let restarts = self.config.restart_count();

        let Some(mut handle) = launch(&self.config, modules.clone()).await? else {
            // The RPC server is disabled, so we can return Ok.
            return Ok(());
        };

        for _ in 0..=restarts {
            tokio::select! {
                _ = handle.clone().stopped() => {
                    match launch(&self.config, modules.clone()).await {
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

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;

    #[tokio::test]
    async fn test_launch_no_modules() {
        let launcher = RpcBuilder {
            disabled: false,
            socket: SocketAddr::from(([127, 0, 0, 1], 8080)),
            no_restart: false,
            enable_admin: false,
            admin_persistence: None,
            ws_enabled: false,
        };
        let result = launch(&launcher, RpcModule::new(())).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_launch_with_modules() {
        let launcher = RpcBuilder {
            disabled: false,
            socket: SocketAddr::from(([127, 0, 0, 1], 8081)),
            no_restart: false,
            enable_admin: false,
            admin_persistence: None,
            ws_enabled: false,
        };
        let mut modules = RpcModule::new(());

        modules.merge(RpcModule::new(())).expect("module merge");
        modules.merge(RpcModule::new(())).expect("module merge");
        modules.merge(RpcModule::new(())).expect("module merge");

        let result = launch(&launcher, modules).await;
        assert!(result.is_ok());
    }
}
