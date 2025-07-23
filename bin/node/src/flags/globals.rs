//! Global arguments for the CLI.

use alloy_primitives::Address;
use clap::Parser;
use kona_cli::{log::LogArgs, metrics_args::MetricsArgs};
use kona_genesis::RollupConfig;
use kona_registry::OPCHAINS;

/// Global arguments for the CLI.
#[derive(Parser, Default, Clone, Debug)]
pub struct GlobalArgs {
    /// Logging arguments.
    #[command(flatten)]
    pub log_args: LogArgs,
    /// The L2 chain ID to use.
    #[arg(
        long,
        short = 'c',
        global = true,
        default_value = "10",
        env = "KONA_NODE_L2_CHAIN_ID",
        help = "The L2 chain ID to use"
    )]
    pub l2_chain_id: alloy_chains::Chain,
    /// Embed the override flags globally to provide override values adjacent to the configs.
    #[command(flatten)]
    pub override_args: super::OverrideArgs,
    /// Prometheus CLI arguments.
    #[command(flatten)]
    pub metrics: MetricsArgs,
}

impl GlobalArgs {
    /// Applies the specified overrides to the given rollup config.
    ///
    /// Transforms the rollup config and returns the updated config with the overrides applied.
    pub fn apply_overrides(&self, config: RollupConfig) -> RollupConfig {
        self.override_args.apply(config)
    }

    /// Returns the signer [`Address`] from the rollup config for the given l2 chain id.
    pub fn genesis_signer(&self) -> anyhow::Result<Address> {
        let id = self.l2_chain_id;
        OPCHAINS
            .get(&id.id())
            .ok_or(anyhow::anyhow!("No chain config found for chain ID: {id}"))?
            .roles
            .as_ref()
            .ok_or(anyhow::anyhow!("No roles found for chain ID: {id}"))?
            .unsafe_block_signer
            .ok_or(anyhow::anyhow!("No unsafe block signer found for chain ID: {id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_signer() {
        let args = GlobalArgs { l2_chain_id: 10.into(), ..Default::default() };
        assert_eq!(
            args.genesis_signer().unwrap(),
            alloy_primitives::address!("aaaa45d9549eda09e70937013520214382ffc4a2")
        );
    }
}
