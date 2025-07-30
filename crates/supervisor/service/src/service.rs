//! Contains the main Supervisor service runner.

use anyhow::Result;
use jsonrpsee::server::{ServerBuilder, ServerHandle};
use kona_supervisor_core::{Supervisor, SupervisorRpc, config::Config};
use kona_supervisor_rpc::SupervisorApiServer;
use kona_supervisor_storage::ChainDbFactory;
use std::sync::Arc;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::actors::{MetricWorker, SupervisorActor};

/// The main service structure for the Kona
/// [`SupervisorService`](`kona_supervisor_core::SupervisorService`). Orchestrates the various
/// components of the supervisor.
#[derive(Debug)]
pub struct Service<T = Supervisor> {
    config: Config,
    supervisor: Option<Arc<T>>,
    rpc_server_handle: Option<ServerHandle>,
    cancel_token: CancellationToken,
    // TODO:: add other actors
}

impl Service {
    /// Creates a new Supervisor service instance.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            supervisor: None,
            rpc_server_handle: None,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Runs the Supervisor service.
    /// This function will typically run indefinitely until interrupted.
    pub async fn run(&mut self) -> Result<()> {
        info!(target: "supervisor::service",
            address=%self.config.rpc_addr,
            "Attempting to start Supervisor RPC server on address"
        );

        // Initialize the core Supervisor logic
        // In the future, this might take configuration or client connections
        // This creates an Arc<Supervisor>

        let database_factory =
            Arc::new(ChainDbFactory::new(self.config.datadir.clone()).with_metrics());

        // todo: run metric worker only if metrics are enabled
        MetricWorker::new(
            Duration::from_secs(30),
            vec![database_factory.clone()],
            self.cancel_token.clone(),
        )
        .start()
        .await
        .map_err(|err| {
            warn!(target: "supervisor::service",
                %err,
                "Failed to start MetricReporter actor"
            );
            anyhow::anyhow!("failed to start MetricReporter actor: {}", err)
        })?;

        let mut supervisor =
            Supervisor::new(self.config.clone(), database_factory, self.cancel_token.clone());

        supervisor.initialise().await.map_err(|err| {
            warn!(target: "supervisor::service",
                %err,
                "Failed to initialise Supervisor"
            );
            anyhow::anyhow!("failed to initialise Supervisor: {}", err)
        })?;

        let supervisor = Arc::new(supervisor);
        self.supervisor = Some(supervisor.clone());

        // Create the RPC implementation, sharing the core logic
        // SupervisorRpc::new expects Arc<dyn kona_supervisor_core::SupervisorService + ...>
        let rpc_impl = SupervisorRpc::new(supervisor.clone());
        let server = ServerBuilder::default().build(self.config.rpc_addr).await?;
        self.rpc_server_handle = Some(server.start(rpc_impl.clone().into_rpc()));

        info!(target: "supervisor::service",
            addr=%self.config.rpc_addr,
            "Supervisor RPC server started successfully and listening on address",
        );

        Ok(())
    }

    pub async fn shutdown(mut self) -> Result<()> {
        self.cancel_token.cancel(); // Signal cancellation to all tasks

        // If the RPC server handle is present, stop it gracefully
        // This will signal the server to stop accepting new connections
        // and wait for existing connections to finish.
        if let Some(handle) = self.rpc_server_handle.take() {
            info!(target: "supervisor::service", "Sending stop signal to RPC server...");
            handle.stop()?; // Signal the server to stop accepting new connections
            info!(target: "supervisor::service",
                "Waiting for RPC server to shut down completely..."
            );
            handle.stopped().await; // Wait for the server to fully stop
            info!(target: "supervisor::service", "Supervisor RPC server shut down gracefully.");
        } else {
            warn!(target: "supervisor::service",
                "Shutdown called, but RPC server handle was not present. Was run() called?"
            );
        }
        // TODO: Add shutdown logic for other components if any are added.
        Ok(())
    }
}
