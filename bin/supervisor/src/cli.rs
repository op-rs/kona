//! Contains the supervisor CLI.

use crate::flags::{GlobalArgs, SupervisorArgs};
use anyhow::Result;
use clap::Parser;
use kona_cli::{cli_styles, metrics_args::MetricsArgs};
use kona_supervisor_service::Service;
use tracing::info;

/// CLI for the Rust implementation of the OP Supervisor.
#[derive(Parser, Debug)]
#[command(name = "op-supervisor", about = "Rust implementation of the OP Supervisor", styles = cli_styles())]
pub struct Cli {
    /// Global args
    #[command(flatten)]
    pub global: GlobalArgs,

    /// Prometheus metrics args
    #[command(flatten)]
    pub metrics: MetricsArgs,

    /// Supervisor args
    #[command(flatten)]
    pub supervisor: SupervisorArgs,
}

impl Cli {
    /// Runs the CLI.
    pub fn run(self) -> Result<()> {
        self.metrics.init_metrics()?;
        self.init_logs(&self.global)?;

        Self::run_until_ctrl_c(async move {
            let config = self.supervisor.init_config().await?;
            let mut service = Service::new(config);
            service.run().await?; // run() now returns Result<()> and populates the handle internally

            tokio::signal::ctrl_c().await?;
            info!("Shutdown signal received. Initiating service shutdown...");

            service.shutdown().await?; // Call shutdown on the service instance itself

            info!("Supervisor service shut down gracefully.");
            Ok(())
        })
    }

    /// Run until ctrl-c is pressed.
    pub fn run_until_ctrl_c<F>(fut: F) -> Result<()>
    where
        F: std::future::Future<Output = Result<()>>,
    {
        let rt = Self::tokio_runtime().map_err(|e| anyhow::anyhow!(e))?;
        rt.block_on(fut)
    }

    /// Creates a new default tokio multi-thread [`Runtime`](tokio::runtime::Runtime) with all
    /// features enabled
    pub fn tokio_runtime() -> Result<tokio::runtime::Runtime, std::io::Error> {
        tokio::runtime::Builder::new_multi_thread().enable_all().build()
    }

    /// Initializes the telemetry stack and Prometheus metrics recorder.
    pub fn init_logs(&self, args: &GlobalArgs) -> anyhow::Result<()> {
        // Filter out discovery warnings since they're very very noisy.
        let filter = tracing_subscriber::EnvFilter::from_default_env();

        args.init_tracing(Some(filter))?;
        Ok(())
    }
}
