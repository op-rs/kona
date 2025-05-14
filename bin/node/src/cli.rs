//! Contains the node CLI.

use crate::{
    commands::{BootstoreCommand, InfoCommand, NetCommand, NodeCommand, RegistryCommand},
    flags::{GlobalArgs, MetricsArgs},
    version,
};
use anyhow::Result;
use clap::{Parser, Subcommand};
use kona_cli::cli_styles;

/// Subcommands for the CLI.
#[derive(Debug, PartialEq, Clone, Subcommand)]
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
    /// Utility tool to interact with local bootstores.
    #[command(alias = "b", alias = "boot", alias = "store")]
    Bootstore(BootstoreCommand),
    /// Get info about op chain.
    Info(InfoCommand),
}

/// The node CLI.
#[derive(Parser, Clone, Debug)]
#[command(
    author,
    version = version::SHORT_VERSION,
    long_version = version::LONG_VERSION,
    about,
    styles = cli_styles(),
    long_about = None
)]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub subcommand: Commands,
    /// Global arguments for the CLI.
    #[command(flatten)]
    pub global: GlobalArgs,
    /// Prometheus CLI arguments.
    #[command(flatten)]
    pub metrics: MetricsArgs,
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
            Commands::Bootstore(ref bootstore) => {
                bootstore.init_telemetry(&self.global, &self.metrics)?
            }
            Commands::Info(ref info) => info.init_telemetry(&self.global, &self.metrics)?,
        }

        // Run the subcommand.
        match self.subcommand {
            Commands::Node(node) => Self::run_until_ctrl_c(node.run(&self.global)),
            Commands::Net(net) => Self::run_until_ctrl_c(net.run(&self.global)),
            Commands::Registry(registry) => registry.run(&self.global),
            Commands::Bootstore(bootstore) => bootstore.run(&self.global),
            Commands::Info(info) => info.run(&self.global),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::node_subcommand_long(Commands::Node(Default::default()), "node")]
    #[case::node_subcommand_short(Commands::Node(Default::default()), "n")]
    #[case::net_subcommand_extra_long(Commands::Net(Default::default()), "network")]
    #[case::net_subcommand_long(Commands::Net(Default::default()), "net")]
    #[case::net_subcommand_short(Commands::Net(Default::default()), "p2p")]
    #[case::registry_subcommand_short(Commands::Registry(Default::default()), "r")]
    #[case::registry_subcommand_long(Commands::Registry(Default::default()), "scr")]
    #[case::bootstore_subcommand_short(Commands::Bootstore(Default::default()), "b")]
    #[case::bootstore_subcommand_long(Commands::Bootstore(Default::default()), "boot")]
    #[case::bootstore_subcommand_long2(Commands::Bootstore(Default::default()), "store")]
    #[case::info_subcommand(Commands::Info(Default::default()), "info")]
    fn test_parse_cli(#[case] subcommand: Commands, #[case] subcommand_alias: &str) {
        let args = vec!["kona-node", subcommand_alias, "--help"];
        let cli = Cli::parse_from(args);
        assert_eq!(cli.subcommand, subcommand);
    }
}
