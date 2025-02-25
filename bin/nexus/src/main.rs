#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand};

mod disc;
mod globals;
mod gossip;
mod telemetry;

/// The CLI Arguments.
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about = None)]
pub(crate) struct NexusArgs {
    /// Verbosity level (0-2)
    #[arg(long, short, action = ArgAction::Count)]
    pub v: u8,
    /// Global arguments for the CLI.
    #[clap(flatten)]
    pub global: globals::GlobalArgs,
    /// The subcommand to run.
    #[clap(subcommand)]
    pub subcommand: NexusSubcommand,
}

/// Subcommands for the CLI.
#[derive(Debug, Clone, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum NexusSubcommand {
    /// Discovery service command.
    Disc(disc::DiscCommand),
    /// Gossip service command.
    Gossip(gossip::GossipCommand),
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse arguments.
    let args = NexusArgs::parse();

    // Initialize the telemetry stack.
    telemetry::init_stack(args.v, args.global.metrics_port)?;

    // Dispatch on subcommand.
    match args.subcommand {
        NexusSubcommand::Disc(disc) => disc.run(&args.global).await,
        NexusSubcommand::Gossip(gossip) => gossip.run(&args.global).await,
    }
}
