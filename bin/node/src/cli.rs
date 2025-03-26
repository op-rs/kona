//! Contains the node CLI.

use crate::{
    commands::{NetCommand, NodeCommand},
    flags::GlobalArgs,
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use kona_cli::{cli_styles, init_prometheus_server, init_tracing_subscriber};
use tracing_subscriber::EnvFilter;

/// Subcommands for the CLI.
#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Runs the consensus node.
    Node(NodeCommand),
    /// Runs the networking stack for the node.
    Net(NetCommand),
}

/// The node CLI.
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, styles = cli_styles(), long_about = None)]
pub struct Cli {
    /// Global arguments for the CLI.
    #[command(flatten)]
    pub global: GlobalArgs,
    /// The subcommand to run.
    #[command(subcommand)]
    pub subcommand: Commands,
}

impl Cli {
    /// Runs the CLI.
    pub fn run(self) -> Result<()> {
        // Initialize the telemetry stack.
        Self::init_stack(self.global.v, self.global.metrics_port)?;

        match self.subcommand {
            Commands::Node(node) => Self::run_until_ctrl_c(node.run(&self.global)),
            Commands::Net(net) => Self::run_until_ctrl_c(net.run(&self.global)),
        }
    }

    /// Initialize the tracing stack and Prometheus metrics recorder.
    ///
    /// This function should be called at the beginning of the program.
    pub fn init_stack(verbosity: u8, metrics_port: u16) -> Result<()> {
        // Initialize the tracing subscriber.
        init_tracing_subscriber(verbosity, None::<EnvFilter>)?;

        // Start the Prometheus metrics server.
        init_prometheus_server(metrics_port)?;

        Ok(())
    }

    /// Run until ctrl-c is pressed.
    pub fn run_until_ctrl_c<F>(fut: F) -> Result<()>
    where
        F: std::future::Future<Output = Result<()>>,
    {
        let rt = Self::tokio_runtime().map_err(|e| anyhow::anyhow!(e))?;
        rt.block_on(fut)
    }

    /// Creates a new default tokio multi-thread [Runtime](tokio::runtime::Runtime) with all
    /// features enabled
    pub fn tokio_runtime() -> Result<tokio::runtime::Runtime, std::io::Error> {
        tokio::runtime::Builder::new_multi_thread().enable_all().build()
    }
}
