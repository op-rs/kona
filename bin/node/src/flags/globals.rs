//! Global arguments for the CLI.

use alloy_primitives::Address;
use clap::{ArgAction, Parser};
use kona_registry::ROLLUP_CONFIGS;

/// Global arguments for the CLI.
#[derive(Parser, Clone, Debug)]
pub struct GlobalArgs {
    /// Verbosity level (0-2)
    #[arg(long, short, action = ArgAction::Count)]
    pub v: u8,
    /// The L2 chain ID to use.
    #[arg(long, short = 'c', default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,
    /// A port to serve prometheus metrics on.
    #[arg(
        long,
        short = 'm',
        default_value = "9090",
        help = "The port to serve prometheus metrics on"
    )]
    pub metrics_port: u16,
}

impl GlobalArgs {
    /// Returns the signer [`Address`] from the rollup config for the given l2 chain id.
    pub fn genesis_signer(&self) -> anyhow::Result<Address> {
        let id = self.l2_chain_id;
        ROLLUP_CONFIGS
            .get(&id)
            .ok_or(anyhow::anyhow!("No rollup config found for chain ID: {id}"))?
            .genesis
            .system_config
            .as_ref()
            .map(|c| c.batcher_address)
            .ok_or(anyhow::anyhow!("No system config found for chain ID: {id}"))
    }
}
