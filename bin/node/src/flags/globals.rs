//! Global arguments for the CLI.

use crate::metrics::CliMetrics;
use alloy_primitives::Address;
use clap::{ArgAction, Parser};
use kona_cli::init_tracing_subscriber;
use kona_genesis::RollupConfig;
use kona_registry::{OPCHAINS, ROLLUP_CONFIGS};
use tracing_subscriber::EnvFilter;

/// Global arguments for the CLI.
#[derive(Parser, Default, Clone, Debug)]
pub struct GlobalArgs {
    /// Verbosity level (0-5).
    /// If set to 0, no logs are printed.
    /// By default, the verbosity level is set to 3 (info level).
    #[arg(long, short, global = true, default_value = "3", action = ArgAction::Count)]
    pub v: u8,
    /// The L2 chain ID to use.
    #[arg(long, short = 'c', global = true, default_value = "10", help = "The L2 chain ID to use")]
    pub l2_chain_id: u64,
    /// Embed the override flags globally to provide override values adjacent to the configs.
    #[command(flatten)]
    pub override_args: super::OverrideArgs,
}

impl GlobalArgs {
    /// Initializes the telemetry stack and Prometheus metrics recorder.
    pub fn init_tracing(&self, filter: Option<EnvFilter>) -> anyhow::Result<()> {
        Ok(init_tracing_subscriber(self.v, filter)?)
    }

    /// Initializes cli metrics for global argument values.
    pub fn init_cli_metrics(&self) {
        metrics::describe_gauge!(
            CliMetrics::ROLLUP_CONFIG,
            "Rollup configuration settings for the OP Stack"
        );
        metrics::describe_gauge!(
            CliMetrics::HARDFORK_ACTIVATION_TIMES,
            "Activation times for hardforks in the OP Stack"
        );

        let config = self.rollup_config().unwrap_or_default();

        metrics::gauge!(
            CliMetrics::ROLLUP_CONFIG,
            &[
                ("l1_genesis_block_num", config.genesis.l1.number.to_string()),
                ("l2_genesis_block_num", config.genesis.l2.number.to_string()),
                ("genesis_l2_time", config.genesis.l2_time.to_string()),
                ("l1_chain_id", config.l1_chain_id.to_string()),
                ("l2_chain_id", config.l2_chain_id.to_string()),
                ("block_time", config.block_time.to_string()),
                ("max_sequencer_drift", config.max_sequencer_drift.to_string()),
                ("sequencer_window_size", config.seq_window_size.to_string()),
                ("channel_timeout", config.channel_timeout.to_string()),
                ("granite_channel_timeout", config.granite_channel_timeout.to_string()),
                ("batch_inbox_address", config.batch_inbox_address.to_string()),
                ("deposit_contract_address", config.deposit_contract_address.to_string()),
                ("l1_system_config_address", config.l1_system_config_address.to_string()),
                ("protocol_versions_address", config.protocol_versions_address.to_string()),
            ]
        )
        .set(1);

        for (fork_name, activation_time) in config.hardforks.iter() {
            // Set the value of the metric for the given hardfork, using `-1` as a signal that the
            // fork is not scheduled.
            metrics::gauge!(CliMetrics::HARDFORK_ACTIVATION_TIMES, "fork" => fork_name)
                .set(activation_time.map(|t| t as f64).unwrap_or(-1f64));
        }
    }

    /// Returns the [`RollupConfig`] for the [`GlobalArgs::l2_chain_id`] specified on the global
    /// arguments.
    pub fn rollup_config(&self) -> Option<RollupConfig> {
        ROLLUP_CONFIGS.get(&self.l2_chain_id).cloned().map(|c| self.apply_overrides(c))
    }

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
