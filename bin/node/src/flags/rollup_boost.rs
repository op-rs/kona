//! Rollup-boost CLI arguments.

use clap::Parser;
use http::Uri;
use std::path::PathBuf;

/// Configuration arguments for rollup-boost integration.
///
/// These arguments control how the node integrates with rollup-boost for
/// enhanced block building capabilities. Rollup-boost can be used to
/// improve block building performance by leveraging external builders.
#[derive(Parser, PartialEq, Debug, Clone, Default)]
pub struct RollupBoostArgs {
    /// Enable rollup-boost integration.
    /// When enabled, the node will use rollup-boost for payload building instead of direct L2 calls.
    #[arg(
        long = "rollup-boost",
        env = "KONA_NODE_ROLLUP_BOOST_ENABLED",
        help = "Enable rollup-boost integration for enhanced block building"
    )]
    pub enabled: bool,

    /// URL of the external builder service (rollup-boost server).
    /// This is the endpoint where rollup-boost is running.
    #[arg(
        long = "rollup-boost-builder-url",
        env = "KONA_NODE_ROLLUP_BOOST_BUILDER_URL",
        help = "URL of the external builder service endpoint",
        requires = "enabled"
    )]
    pub builder_url: Option<Uri>,

    /// Path to the JWT secret file for builder authentication.
    /// This file should contain the hex-encoded JWT secret used to authenticate with the builder.
    #[arg(
        long = "rollup-boost-builder-jwt-secret",
        env = "KONA_NODE_ROLLUP_BOOST_BUILDER_JWT_SECRET",
        help = "Path to JWT secret file for builder authentication",
        requires = "enabled"
    )]
    pub builder_jwt_secret_file: Option<PathBuf>,

    /// Timeout for builder requests in milliseconds.
    /// How long to wait for the builder to respond before falling back to L2.
    #[arg(
        long = "rollup-boost-builder-timeout",
        env = "KONA_NODE_ROLLUP_BOOST_BUILDER_TIMEOUT",
        default_value = "5000",
        help = "Timeout for builder requests in milliseconds",
        requires = "enabled"
    )]
    pub builder_timeout_ms: u64,

    /// Timeout for L2 fallback requests in milliseconds.
    /// How long to wait for L2 when builder fails.
    #[arg(
        long = "rollup-boost-l2-timeout",
        env = "KONA_NODE_ROLLUP_BOOST_L2_TIMEOUT",
        default_value = "5000",
        help = "Timeout for L2 fallback requests in milliseconds",
        requires = "enabled"
    )]
    pub l2_timeout_ms: u64,

    /// Execution mode for rollup-boost.
    /// Controls how rollup-boost behaves when interacting with builders.
    #[arg(
        long = "rollup-boost-execution-mode",
        env = "KONA_NODE_ROLLUP_BOOST_EXECUTION_MODE",
        default_value = "enabled",
        help = "Execution mode: 'enabled' (use builder with L2 fallback), 'dry-run' (request from both, always use L2), 'disabled' (only use L2)",
        requires = "enabled"
    )]
    pub execution_mode: RollupBoostExecutionMode,

    /// Block selection policy for rollup-boost.
    /// Determines which block to select when both builder and L2 provide blocks.
    #[arg(
        long = "rollup-boost-block-selection",
        env = "KONA_NODE_ROLLUP_BOOST_BLOCK_SELECTION",
        default_value = "builder-preference",
        help = "Block selection policy: 'builder-preference' (prefer builder blocks), 'gas-used' (select block with higher gas usage)",
        requires = "enabled"
    )]
    pub block_selection_policy: RollupBoostBlockSelectionPolicy,

    /// Whether state root computation is external (OP Stack specific).
    /// Set to true if the state root is computed by the sequencer rather than locally.
    #[arg(
        long = "rollup-boost-external-state-root",
        env = "KONA_NODE_ROLLUP_BOOST_EXTERNAL_STATE_ROOT",
        default_value = "false",
        help = "Whether state root computation is external (OP Stack specific)",
        requires = "enabled"
    )]
    pub external_state_root: bool,

    /// Whether to ignore unhealthy builders.
    /// When enabled, rollup-boost will skip builder calls when health checks fail.
    #[arg(
        long = "rollup-boost-ignore-unhealthy-builders",
        env = "KONA_NODE_ROLLUP_BOOST_IGNORE_UNHEALTHY_BUILDERS",
        default_value = "false",
        help = "Skip builder calls when health checks fail",
        requires = "enabled"
    )]
    pub ignore_unhealthy_builders: bool,

    /// Health check interval in seconds.
    /// How often to check if the builder is healthy.
    #[arg(
        long = "rollup-boost-health-check-interval",
        env = "KONA_NODE_ROLLUP_BOOST_HEALTH_CHECK_INTERVAL",
        default_value = "60",
        help = "Health check interval in seconds",
        requires = "enabled"
    )]
    pub health_check_interval_secs: u64,

    /// Health check threshold for marking builder as unhealthy.
    /// Number of consecutive failed health checks before marking builder as unhealthy.
    #[arg(
        long = "rollup-boost-health-check-threshold",
        env = "KONA_NODE_ROLLUP_BOOST_HEALTH_CHECK_THRESHOLD",
        default_value = "10",
        help = "Number of consecutive failed health checks before marking builder as unhealthy",
        requires = "enabled"
    )]
    pub health_check_threshold: u32,

    /// Enable Prometheus metrics export for rollup-boost.
    /// When enabled, rollup-boost metrics will be exported to Prometheus.
    #[arg(
        long = "rollup-boost-metrics",
        env = "KONA_NODE_ROLLUP_BOOST_METRICS",
        default_value = "false",
        help = "Enable Prometheus metrics export for rollup-boost",
        requires = "enabled"
    )]
    pub metrics_enabled: bool,

    /// Address for Prometheus metrics server.
    /// Address where the Prometheus metrics server should listen.
    #[arg(
        long = "rollup-boost-metrics-addr",
        env = "KONA_NODE_ROLLUP_BOOST_METRICS_ADDR",
        default_value = "0.0.0.0:9090",
        help = "Address for Prometheus metrics server",
        requires_all = ["enabled", "metrics_enabled"]
    )]
    pub metrics_addr: Option<String>,
}

/// Execution mode for rollup-boost.
#[derive(Debug, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum RollupBoostExecutionMode {
    /// Use builder with L2 fallback (production mode).
    #[value(name = "enabled")]
    Enabled,
    /// Request from both, always use L2 (testing mode).
    #[value(name = "dry-run")]
    DryRun,
    /// Only use L2, no builder calls (emergency mode).
    #[value(name = "disabled")]
    Disabled,
}

impl From<RollupBoostExecutionMode> for rollup_boost::ExecutionMode {
    fn from(mode: RollupBoostExecutionMode) -> Self {
        match mode {
            RollupBoostExecutionMode::Enabled => rollup_boost::ExecutionMode::Enabled,
            RollupBoostExecutionMode::DryRun => rollup_boost::ExecutionMode::DryRun,
            RollupBoostExecutionMode::Disabled => rollup_boost::ExecutionMode::Disabled,
        }
    }
}

/// Block selection policy for rollup-boost.
#[derive(Debug, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum RollupBoostBlockSelectionPolicy {
    /// Prefer builder blocks (default).
    #[value(name = "builder-preference")]
    BuilderPreference,
    /// Select block with higher gas usage.
    #[value(name = "gas-used")]
    GasUsed,
}

impl From<RollupBoostBlockSelectionPolicy> for Option<rollup_boost::BlockSelectionPolicy> {
    fn from(policy: RollupBoostBlockSelectionPolicy) -> Self {
        match policy {
            RollupBoostBlockSelectionPolicy::BuilderPreference => None,
            RollupBoostBlockSelectionPolicy::GasUsed => Some(rollup_boost::BlockSelectionPolicy::GasUsed),
        }
    }
}

impl RollupBoostArgs {
    /// Validates the rollup-boost configuration.
    pub fn validate(&self) -> anyhow::Result<()> {
        if !self.enabled {
            return Ok(());
        }

        if self.builder_url.is_none() {
            anyhow::bail!("rollup-boost-builder-url is required when rollup-boost is enabled");
        }

        if self.builder_jwt_secret_file.is_none() {
            anyhow::bail!("rollup-boost-builder-jwt-secret is required when rollup-boost is enabled");
        }

        if self.health_check_interval_secs == 0 {
            anyhow::bail!("rollup-boost-health-check-interval must be greater than 0");
        }

        if self.health_check_threshold == 0 {
            anyhow::bail!("rollup-boost-health-check-threshold must be greater than 0");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollup_boost_args_defaults() {
        let args = RollupBoostArgs::default();
        assert!(!args.enabled);
        assert_eq!(args.execution_mode, RollupBoostExecutionMode::Enabled);
        assert_eq!(args.block_selection_policy, RollupBoostBlockSelectionPolicy::BuilderPreference);
        assert!(!args.external_state_root);
        assert!(!args.ignore_unhealthy_builders);
        assert_eq!(args.health_check_interval_secs, 60);
        assert_eq!(args.health_check_threshold, 10);
        assert!(!args.metrics_enabled);
    }

    #[test]
    fn test_rollup_boost_execution_mode_conversion() {
        use rollup_boost::ExecutionMode;

        assert_eq!(rollup_boost::ExecutionMode::from(RollupBoostExecutionMode::Enabled), ExecutionMode::Enabled);
        assert_eq!(rollup_boost::ExecutionMode::from(RollupBoostExecutionMode::DryRun), ExecutionMode::DryRun);
        assert_eq!(rollup_boost::ExecutionMode::from(RollupBoostExecutionMode::Disabled), ExecutionMode::Disabled);
    }

    #[test]
    fn test_rollup_boost_block_selection_policy_conversion() {
        use rollup_boost::BlockSelectionPolicy;

        assert_eq!(<Option<BlockSelectionPolicy>>::from(RollupBoostBlockSelectionPolicy::BuilderPreference), None);
        assert_eq!(<Option<BlockSelectionPolicy>>::from(RollupBoostBlockSelectionPolicy::GasUsed), Some(BlockSelectionPolicy::GasUsed));
    }

    #[test]
    fn test_rollup_boost_args_validation_disabled() {
        let args = RollupBoostArgs::default();
        assert!(args.validate().is_ok());
    }

    #[test]
    fn test_rollup_boost_args_validation_enabled_missing_builder_url() {
        let mut args = RollupBoostArgs::default();
        args.enabled = true;
        args.builder_jwt_secret_file = Some(PathBuf::from("jwt.hex"));

        assert!(args.validate().is_err());
    }

    #[test]
    fn test_rollup_boost_args_validation_enabled_missing_jwt_secret() {
        let mut args = RollupBoostArgs::default();
        args.enabled = true;
        args.builder_url = Some(Uri::from_static("http://localhost:8551"));

        assert!(args.validate().is_err());
    }

    #[test]
    fn test_rollup_boost_args_validation_enabled_valid() {
        let mut args = RollupBoostArgs::default();
        args.enabled = true;
        args.builder_url = Some(Uri::from_static("http://localhost:8551"));
        args.builder_jwt_secret_file = Some(PathBuf::from("jwt.hex"));

        assert!(args.validate().is_ok());
    }
}
