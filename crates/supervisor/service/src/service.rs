//! Contains the main Supervisor service runner.

use anyhow::Result;
use jsonrpsee::server::{ServerBuilder, ServerHandle};
use kona_supervisor_core::{Supervisor, SupervisorRpc, SupervisorService};
use kona_supervisor_rpc::SupervisorApiServer;
use std::{net::SocketAddr, sync::Arc};
use tracing::{info, warn};

/// Configuration for the Supervisor service.
#[derive(Debug, Clone)]
pub struct Config {
    /// The socket address for the RPC server to listen on.
    // TODO:: refactoring required. RPC config should be managed in it's domain
    pub rpc_addr: SocketAddr,
    // Add other configuration fields as needed (e.g., connection details for L1/L2 nodes)
}

/// The main service structure for the Kona [`SupervisorService`]. Orchestrates the various
/// components of the supervisor.
#[derive(Debug)]
pub struct Service<T = Supervisor> {
    config: Config,
    _supervisor: Arc<T>,
    rpc_impl: SupervisorRpc<T>,
    rpc_server_handle: Option<ServerHandle>,
    // TODO:: add other actors
}

impl Service {
    /// Creates a new Supervisor service instance.
    pub fn new(config: Config) -> Result<Self> {
        // Initialize the core Supervisor logic
        // In the future, this might take configuration or client connections
        // This creates an Arc<Supervisor>
        let supervisor = Arc::new(Supervisor::new());

        // Create the RPC implementation, sharing the core logic
        // SupervisorRpc::new expects Arc<dyn kona_supervisor_core::SupervisorService + ...>
        let rpc_impl = SupervisorRpc::new(supervisor.clone());

        Ok(Self { config, _supervisor: supervisor, rpc_impl, rpc_server_handle: None })
    }
}

impl<T> Service<T>
where
    T: SupervisorService + 'static,
{
    /// Runs the Supervisor service.
    /// This function will typically run indefinitely until interrupted.
    pub async fn run(&mut self) -> Result<()> {
        info!(target: "supervisor_service",
            address=%self.config.rpc_addr,
            "Attempting to start Supervisor RPC server on address"
        );

        let server = ServerBuilder::default().build(self.config.rpc_addr).await?;

        self.rpc_server_handle = Some(server.start(self.rpc_impl.clone().into_rpc()));

        info!(target: "supervisor_service",
            addr=%self.config.rpc_addr,
            "Supervisor RPC server started successfully and listening on address",
        );

        Ok(())
    }

    pub async fn shutdown(mut self) -> Result<()> {
        if let Some(handle) = self.rpc_server_handle.take() {
            info!(target: "supervisor_service", "Sending stop signal to RPC server...");
            handle.stop()?; // Signal the server to stop accepting new connections
            info!(target: "supervisor_service",
                "Waiting for RPC server to shut down completely..."
            );
            handle.stopped().await; // Wait for the server to fully stop
            info!(target: "supervisor_service", "Supervisor RPC server shut down gracefully.");
        } else {
            warn!(target: "supervisor_service",
                "Shutdown called, but RPC server handle was not present. Was run() called?"
            );
        }
        // TODO: Add shutdown logic for other components if any are added.
        Ok(())
    }
}
