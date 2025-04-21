//! Contains the node CLI.

use crate::{
    commands::{NetCommand, NodeCommand, RegistryCommand},
    flags::{GlobalArgs, MetricsArgs},
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use kona_cli::cli_styles;

/// Subcommands for the CLI.
#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Runs the consensus node.
    #[command(alias = "n")]
    Node(NodeCommand),
    /// Runs the networking stack for the node.
    #[command(alias = "p2p", alias = "network")]
    Net(NetCommand),
    /// Lists the OP Stack chains available in the superchain-registry.
    #[command(alias = "r", alias = "scr")]
    Registry(RegistryCommand),
}

/// The node CLI.
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, styles = cli_styles(), long_about = None)]
pub struct Cli {
    /// Global arguments for the CLI.
    #[command(flatten)]
    pub global: GlobalArgs,
    /// Prometheus CLI arguments.
    #[command(flatten)]
    pub metrics: MetricsArgs,
    /// The subcommand to run.
    #[command(subcommand)]
    pub subcommand: Commands,
}

impl Cli {
    /// Runs the CLI.
    pub fn run(self) -> Result<()> {
        // Initialize telemetry - allow subcommands to customize the filter.
        match self.subcommand {
            Commands::Node(ref node) => node.init_telemetry(&self.global, &self.metrics)?,
            Commands::Net(ref net) => net.init_telemetry(&self.global, &self.metrics)?,
            Commands::Registry(ref registry) => {
                registry.init_telemetry(&self.global, &self.metrics)?
            }
        }

        // Run the subcommand.
        match self.subcommand {
            Commands::Node(node) => Self::run_until_ctrl_c(node.run(&self.global)),
            Commands::Net(net) => Self::run_until_ctrl_c(net.run(&self.global)),
            Commands::Registry(registry) => registry.run(&self.global),
        }
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
