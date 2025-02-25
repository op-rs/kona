//! Contains the node CLI.

pub mod disc;
pub mod globals;
pub mod gossip;
pub mod telemetry;

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand};

/// Subcommands for the CLI.
#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Commands {
    /// Discovery service command.
    Disc(disc::DiscCommand),
    /// Gossip service command.
    Gossip(gossip::GossipCommand),
}

/// The node CLI.
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Verbosity level (0-2)
    #[arg(long, short, action = ArgAction::Count)]
    pub v: u8,
    /// Global arguments for the CLI.
    #[clap(flatten)]
    pub global: globals::GlobalArgs,
    /// The subcommand to run.
    #[clap(subcommand)]
    pub subcommand: Commands,
}

impl Cli {
    /// Runs the CLI.
    pub async fn run(self) -> Result<()> {
        // Initialize the telemetry stack.
        telemetry::init_stack(self.v, self.global.metrics_port)?;

        match self.subcommand {
            Commands::Disc(disc) => disc.run(&self.global).await,
            Commands::Gossip(gossip) => gossip.run(&self.global).await,
        }
    }
}
