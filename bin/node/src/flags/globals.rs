//! Global arguments for the CLI.

use alloy_primitives::Address;
use clap::{ArgAction, Parser};
use kona_genesis::RollupConfig;
use kona_registry::{OPCHAINS, ROLLUP_CONFIGS};

/// Global arguments for the CLI.
#[derive(Parser, Default, Clone, Debug)]
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
    /// Returns the [`RollupConfig`] for the [`GlobalArgs::l2_chain_id`] specified on the global
    /// arguments.
    pub fn rollup_config(&self) -> Option<RollupConfig> {
        ROLLUP_CONFIGS.get(&self.l2_chain_id).cloned()
    }

    /// Returns the signer [`Address`] from the rollup config for the given l2 chain id.
    pub fn genesis_signer(&self) -> anyhow::Result<Address> {
        let id = self.l2_chain_id;
        OPCHAINS
            .get(&id)
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
        let args = GlobalArgs { l2_chain_id: 10, ..Default::default() };
        assert_eq!(
            args.genesis_signer().unwrap(),
            alloy_primitives::address!("aaaa45d9549eda09e70937013520214382ffc4a2")
        );
    }
}
