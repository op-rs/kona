//! RPC Server Actor

use crate::{NodeActor, actors::CancellableContext};
use async_trait::async_trait;
use jsonrpsee::{
    RpcModule,
    core::RegisterMethodError,
    server::{HttpBody, HttpRequest, HttpResponse, Server, ServerHandle, middleware::http::ProxyGetRequestLayer},
};
use kona_engine::EngineQueries;
use kona_gossip::P2pRpcRequest;
use kona_rpc::{
    AdminApiServer, AdminRpc, DevEngineApiServer, DevEngineRpc, HealthzApiServer, HealthzRpc,
    L1WatcherQueries, NetworkAdminQuery, OpP2PApiServer, P2pRpc, RollupBoostAdminQuery,
    RollupBoostHealth, RollupBoostHealthQuery, RollupNodeApiServer, RollupRpc, RpcBuilder,
    SequencerAdminAPIClient, WsRPC, WsServer,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};
use tower::{Layer, Service};

/// The path for the rollup boost healthz endpoint.
const ROLLUP_BOOST_HEALTHZ_PATH: &str = "/kona-rollup-boost/healthz";

/// A tower layer that intercepts requests to `/kona-rollup-boost/healthz`
/// and returns the appropriate HTTP response based on health status.
#[derive(Debug, Clone)]
struct RollupBoostHealthLayer {
    /// The rollup boost health query sender.
    health_tx: mpsc::Sender<RollupBoostHealthQuery>,
}

impl RollupBoostHealthLayer {
    /// Constructs a new [`RollupBoostHealthLayer`].
    const fn new(health_tx: mpsc::Sender<RollupBoostHealthQuery>) -> Self {
        Self { health_tx }
    }
}

impl<S> Layer<S> for RollupBoostHealthLayer {
    type Service = RollupBoostHealthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RollupBoostHealthMiddleware { inner, health_tx: self.health_tx.clone() }
    }
}

/// The middleware service that handles `/kona-rollup-boost/healthz` requests.
///
/// ## Health Status Determination
///
/// ```text
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// | Execution Mode | Healthy                       | PartialContent                       | Service Unavailable           |
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// | Enabled        | - Request-path: L2 succeeds   | - Request-path: builder fails/stale  | - Request-path: L2 fails      |
/// |                |   (get/new payload) → 200     |   while L2 succeeds → 206            |   (error from L2) → 503       |
/// |                | - Background: builder         | - Background: builder fetch fails or | - Background: never sets 503  |
/// |                |   latest-unsafe is fresh →    |   latest-unsafe is stale → 206       |                               |
/// |                |   200                         |                                      |                               |
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// | DryRun         | - Request-path: L2 succeeds   | - Never set in DryRun                | - Request-path: L2 fails      |
/// |                |   (always returns L2) → 200   |   (degrade only in Enabled)          |   (error from L2) → 503       |
/// |                | - Background: builder stale   |                                      | - Background: never sets 503  |
/// |                |   ignored (remains 200)       |                                      |                               |
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// | Disabled       | - Request-path: L2 succeeds   | - Never set in Disabled              | - Request-path: L2 fails      |
/// |                |   (builder skipped) → 200     |   (degrade only in Enabled)          |   (error from L2) → 503       |
/// |                | - Background: N/A             |                                      | - Background: never sets 503  |
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// ```
#[derive(Debug, Clone)]
struct RollupBoostHealthMiddleware<S> {
    /// The inner service.
    inner: S,
    /// The rollup boost health query sender.
    health_tx: mpsc::Sender<RollupBoostHealthQuery>,
}

impl<S, ReqBody> Service<HttpRequest<ReqBody>> for RollupBoostHealthMiddleware<S>
where
    S: Service<HttpRequest<ReqBody>, Response = HttpResponse> + Clone + Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
    S::Future: Send,
    ReqBody: Send + 'static,
{
    type Response = HttpResponse;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: HttpRequest<ReqBody>) -> Self::Future {
        // Check if this is a GET request to the rollup boost healthz endpoint.
        if req.method() == http::Method::GET && req.uri().path() == ROLLUP_BOOST_HEALTHZ_PATH {
            let health_tx = self.health_tx.clone();

            Box::pin(async move {
                // Query the health status via the engine actor.
                let (tx, rx) = oneshot::channel();
                let health = match health_tx.send(RollupBoostHealthQuery { sender: tx }).await {
                    Ok(()) => rx.await.ok(),
                    Err(_) => None,
                };

                // Build the appropriate response.
                let (status, body_str) = match health {
                    Some(RollupBoostHealth::Healthy) => (http::StatusCode::OK, "OK"),
                    Some(RollupBoostHealth::PartialContent) => {
                        (http::StatusCode::PARTIAL_CONTENT, "Partial Content")
                    }
                    Some(RollupBoostHealth::ServiceUnavailable) | None => {
                        (http::StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable")
                    }
                };

                let response = http::Response::builder()
                    .status(status)
                    .header(http::header::CONTENT_TYPE, "text/plain")
                    .body(HttpBody::from(body_str))
                    .expect("Failed to build rollup boost healthz response");

                Ok(response)
            })
        } else {
            // Pass through to the inner service.
            let fut = self.inner.call(req);
            Box::pin(async move { fut.await })
        }
    }
}

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
pub struct RpcActor<S: SequencerAdminAPIClient> {
    /// A launcher for the rpc.
    config: RpcBuilder,

    phantom: std::marker::PhantomData<S>,
}

impl<S: SequencerAdminAPIClient> RpcActor<S> {
    /// Constructs a new [`RpcActor`] given the [`RpcBuilder`].
    pub const fn new(config: RpcBuilder) -> Self {
        Self { config, phantom: std::marker::PhantomData }
    }
}

/// The communication context used by the RPC actor.
#[derive(Debug)]
pub struct RpcContext<SequencerAdminApiClient> {
    /// The network p2p rpc sender.
    pub p2p_network: mpsc::Sender<P2pRpcRequest>,
    /// The network admin rpc sender.
    pub network_admin: mpsc::Sender<NetworkAdminQuery>,
    /// The sequencer admin rpc sender.
    pub sequencer_admin: Option<SequencerAdminApiClient>,
    /// The l1 watcher queries sender.
    pub l1_watcher_queries: mpsc::Sender<L1WatcherQueries>,
    /// The engine query sender.
    pub engine_query: mpsc::Sender<EngineQueries>,
    /// The cancellation token, shared between all tasks.
    pub cancellation: CancellationToken,
    /// The rollup boost admin rpc sender.
    pub rollup_boost_admin: mpsc::Sender<RollupBoostAdminQuery>,
    /// The rollup boost health rpc sender.
    pub rollup_boost_health: mpsc::Sender<RollupBoostHealthQuery>,
}

impl<S: SequencerAdminAPIClient> CancellableContext for RpcContext<S> {
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
    health_tx: mpsc::Sender<RollupBoostHealthQuery>,
) -> Result<ServerHandle, std::io::Error> {
    let middleware = tower::ServiceBuilder::new()
        .layer(RollupBoostHealthLayer::new(health_tx))
        .layer(
            ProxyGetRequestLayer::new([("/healthz", "healthz")])
                .expect("Critical: Failed to build GET method proxy"),
        )
        .timeout(Duration::from_secs(2));
    let server = Server::builder().set_http_middleware(middleware).build(config.socket).await?;

    if let Ok(addr) = server.local_addr() {
        info!(target: "rpc", addr = ?addr, "RPC server bound to address");
    } else {
        error!(target: "rpc", "Failed to get local address for RPC server");
    }

    Ok(server.start(module))
}

#[async_trait]
impl<S: SequencerAdminAPIClient + 'static> NodeActor for RpcActor<S> {
    type Error = RpcActorError;
    type StartData = RpcContext<S>;

    async fn start(
        mut self,
        RpcContext {
            cancellation,
            p2p_network,
            l1_watcher_queries,
            engine_query,
            network_admin,
            sequencer_admin,
            rollup_boost_admin,
            rollup_boost_health,
        }: Self::StartData,
    ) -> Result<(), Self::Error> {
        let mut modules = RpcModule::new(());

        modules.merge(HealthzRpc::new().into_rpc())?;

        // Build the p2p rpc module.
        modules.merge(P2pRpc::new(p2p_network).into_rpc())?;

        // Build the admin rpc module.
        modules.merge(
            AdminRpc::new(sequencer_admin, network_admin, Some(rollup_boost_admin)).into_rpc(),
        )?;

        // Create context for communication between actors.
        let rollup_rpc = RollupRpc::new(engine_query.clone(), l1_watcher_queries);
        modules.merge(rollup_rpc.into_rpc())?;

        // Add development RPC module for engine state introspection if enabled
        if self.config.dev_enabled() {
            let dev_rpc = DevEngineRpc::new(engine_query.clone());
            modules.merge(dev_rpc.into_rpc())?;
        }

        if self.config.ws_enabled() {
            modules.merge(WsRPC::new(engine_query).into_rpc())?;
        }

        let restarts = self.config.restart_count();

        let mut handle = launch(&self.config, modules.clone(), rollup_boost_health.clone()).await?;

        for _ in 0..=restarts {
            tokio::select! {
                _ = handle.clone().stopped() => {
                    match launch(&self.config, modules.clone(), rollup_boost_health.clone()).await {
                        Ok(h) => handle = h,
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

    fn test_health_tx() -> mpsc::Sender<RollupBoostHealthQuery> {
        let (tx, _rx) = mpsc::channel(1);
        tx
    }

    #[tokio::test]
    async fn test_launch_no_modules() {
        let launcher = RpcBuilder {
            socket: SocketAddr::from(([127, 0, 0, 1], 8080)),
            no_restart: false,
            enable_admin: false,
            admin_persistence: None,
            ws_enabled: false,
            dev_enabled: false,
        };
        let result = launch(&launcher, RpcModule::new(()), test_health_tx()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_launch_with_modules() {
        let launcher = RpcBuilder {
            socket: SocketAddr::from(([127, 0, 0, 1], 8081)),
            no_restart: false,
            enable_admin: false,
            admin_persistence: None,
            ws_enabled: false,
            dev_enabled: false,
        };
        let mut modules = RpcModule::new(());

        modules.merge(RpcModule::new(())).expect("module merge");
        modules.merge(RpcModule::new(())).expect("module merge");
        modules.merge(RpcModule::new(())).expect("module merge");

        let result = launch(&launcher, modules, test_health_tx()).await;
        assert!(result.is_ok());
    }
}
