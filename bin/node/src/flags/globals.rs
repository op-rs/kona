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
    use clap::Parser;

    #[test]
    fn test_genesis_signer() {
        let args = GlobalArgs { l2_chain_id: 10.into(), ..Default::default() };
        assert_eq!(
            args.genesis_signer().unwrap(),
            alloy_primitives::address!("aaaa45d9549eda09e70937013520214382ffc4a2")
        );
    }

    #[test]
    fn test_l2_chain_id_parse_numeric() {
        // Test parsing numeric chain IDs
        
        // Optimism mainnet (chain ID 10)
        let args = GlobalArgs::try_parse_from(["test", "--l2-chain-id", "10"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 10);

        // Ethereum mainnet (chain ID 1)
        let args = GlobalArgs::try_parse_from(["test", "--l2-chain-id", "1"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 1);

        // Base mainnet (chain ID 8453)
        let args = GlobalArgs::try_parse_from(["test", "--l2-chain-id", "8453"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 8453);

        // Unknown chain ID should still parse as numeric ID
        let args = GlobalArgs::try_parse_from(["test", "--l2-chain-id", "999999"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 999999);
    }

    #[test]
    fn test_l2_chain_id_parse_string() {
        // Test parsing string chain names
        
        // Optimism by name
        let args = GlobalArgs::try_parse_from(["test", "--l2-chain-id", "optimism"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 10);

        // Ethereum mainnet by name
        let args = GlobalArgs::try_parse_from(["test", "--l2-chain-id", "mainnet"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 1);

        // Base by name
        let args = GlobalArgs::try_parse_from(["test", "--l2-chain-id", "base"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 8453);
    }

    #[test]
    fn test_l2_chain_id_parse_invalid() {
        // Test that invalid string chain names fail to parse
        let result = GlobalArgs::try_parse_from(["test", "--l2-chain-id", "invalid_chain"]);
        assert!(result.is_err());
        
        // The error should be related to parsing
        let err = result.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("invalid"));
    }

    #[test]
    fn test_l2_chain_id_short_flag() {
        // Test that the short flag (-c) also works
        let args = GlobalArgs::try_parse_from(["test", "-c", "10"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 10);

        let args = GlobalArgs::try_parse_from(["test", "-c", "optimism"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 10);
    }

    #[test]
    fn test_l2_chain_id_env_var() {
        // Test that environment variable parsing works
        std::env::set_var("KONA_NODE_L2_CHAIN_ID", "10");
        let args = GlobalArgs::try_parse_from(["test"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 10);
        std::env::remove_var("KONA_NODE_L2_CHAIN_ID");

        std::env::set_var("KONA_NODE_L2_CHAIN_ID", "optimism");
        let args = GlobalArgs::try_parse_from(["test"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 10);
        std::env::remove_var("KONA_NODE_L2_CHAIN_ID");
    }

    #[test]
    fn test_l2_chain_id_default() {
        // Test that the default value is chain ID 10 (Optimism)
        let args = GlobalArgs::try_parse_from(["test"]).unwrap();
        assert_eq!(args.l2_chain_id.id(), 10);
    }
}
