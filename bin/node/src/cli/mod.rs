//! Contains the node CLI.

pub mod disc;
pub mod globals;
pub mod gossip;
pub mod node;
pub mod telemetry;

use anyhow::Result;
use clap::{Parser, Subcommand};
use kona_cli::cli_styles;

/// Subcommands for the CLI.
#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Discovery service command.
    Disc(disc::DiscCommand),
    /// Gossip service command.
    Gossip(gossip::GossipCommand),
    /// Runs the consensus node.
    Node(node::NodeCommand),
}

/// The node CLI.
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, styles = cli_styles(), long_about = None)]
pub struct Cli {
    /// Global arguments for the CLI.
    #[clap(flatten)]
    pub global: globals::GlobalArgs,
    /// The subcommand to run.
    #[clap(subcommand)]
    pub subcommand: Commands,
}

impl Cli {
    /// Runs the CLI.
    pub fn run(self) -> Result<()> {
        // Initialize the telemetry stack.
        telemetry::init_stack(self.global.v, self.global.metrics_port)?;

        match self.subcommand {
            Commands::Disc(disc) => Self::run_until_ctrl_c(disc.run(&self.global)),
            Commands::Gossip(gossip) => Self::run_until_ctrl_c(gossip.run(&self.global)),
            Commands::Node(node) => Self::run_until_ctrl_c(node.run(&self.global)),
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
