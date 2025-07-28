use super::RollupConfigSet;
use crate::syncnode::ClientConfig;
use alloy_primitives::ChainId;
use kona_interop::DependencySet;
use std::{net::SocketAddr, path::PathBuf};
use thiserror::Error;

/// Configuration for the Supervisor service.
#[derive(Debug, Clone)]
pub struct Config {
    /// The URL of the L1 RPC endpoint.
    pub l1_rpc: String,

    /// L2 consensus nodes configuration.
    pub l2_consensus_nodes_config: Vec<ClientConfig>,

    /// Directory where the database files are stored.
    pub datadir: PathBuf,

    /// The socket address for the RPC server to listen on.
    pub rpc_addr: SocketAddr,

    /// The loaded dependency set configuration.
    pub dependency_set: DependencySet,

    /// The rollup configuration set.
    pub rollup_config_set: RollupConfigSet,
}

impl Config {
    /// Validates that the provided timestamps and chain IDs are eligible for interop execution.
    ///
    /// Checks:
    /// - Interop is enabled on both initiating and executing chains at their respective timestamps.
    /// - The initiating timestamp is not after the executing timestamp.
    /// - The message has not expired based on the expiry window and optional timeout.
    pub fn validate_interop_timestamps(
        &self,
        initiating_chain_id: ChainId,
        initiating_timestamp: u64,
        executing_chain_id: ChainId,
        executing_timestamp: u64,
        timeout: Option<u64>,
    ) -> Result<(), ConfigError> {
        // Interop must be active on both chains at the relevant times
        if !self.rollup_config_set.is_interop_enabled(initiating_chain_id, initiating_timestamp) ||
            !self.rollup_config_set.is_interop_enabled(executing_chain_id, executing_timestamp)
        {
            return Err(ConfigError::InteropNotEnabled);
        }

        // Executing timestamp must not be earlier than the initiating timestamp
        if initiating_timestamp > executing_timestamp {
            return Err(ConfigError::InvalidTimestampInvariant {
                initiating: initiating_timestamp,
                executing: executing_timestamp,
            });
        }

        // Ensure the message has not expired by the time of execution
        let expiry_window = self.dependency_set.get_message_expiry_window();
        let expires_at = initiating_timestamp.saturating_add(expiry_window);
        let execution_deadline = executing_timestamp.saturating_add(timeout.unwrap_or(0));

        if expires_at < execution_deadline {
            return Err(ConfigError::InvalidInteropTimestamp(executing_timestamp));
        }

        Ok(())
    }
}

/// Custom error type for interop validation via config.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    /// Interop is not enabled on one or both chains at the required timestamp.
    #[error("interop not enabled")]
    InteropNotEnabled,

    /// Executing timestamp is earlier than the initiating timestamp.
    #[error(
        "executing timestamp is earlier than initiating timestamp, executing: {executing}, initiating: {initiating}"
    )]
    InvalidTimestampInvariant {
        /// Executing timestamp of the message
        executing: u64,
        /// Initiating timestamp of the message
        initiating: u64,
    },

    /// Timestamp is outside the allowed interop expiry window.
    #[error("timestamp outside allowed interop window, timestamp: {0}")]
    InvalidInteropTimestamp(u64),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RollupConfig, core_config::ConfigError};
    use kona_interop::DependencySet;
    use std::{collections::HashMap, net::SocketAddr, path::PathBuf};

    fn mock_rollup_config_set() -> RollupConfigSet {
        let chain1 =
            RollupConfig { genesis: Default::default(), block_time: 2, interop_time: Some(100) };
        let chain2 =
            RollupConfig { genesis: Default::default(), block_time: 2, interop_time: Some(105) };
        let mut config_set = HashMap::<ChainId, RollupConfig>::new();
        config_set.insert(1, chain1);
        config_set.insert(2, chain2);

        RollupConfigSet { rollups: config_set }
    }

    fn mock_config() -> Config {
        Config {
            l1_rpc: Default::default(),
            l2_consensus_nodes_config: vec![],
            datadir: PathBuf::new(),
            rpc_addr: SocketAddr::from(([127, 0, 0, 1], 8545)),
            dependency_set: DependencySet {
                dependencies: Default::default(),
                override_message_expiry_window: Some(10),
            },
            rollup_config_set: mock_rollup_config_set(),
        }
    }

    #[test]
    fn test_valid_case() {
        let cfg = mock_config();
        let res = cfg.validate_interop_timestamps(1, 200, 2, 202, None);
        assert_eq!(res, Ok(()));
    }
    #[test]
    fn test_valid_with_timeout() {
        let cfg = mock_config();
        let res = cfg.validate_interop_timestamps(1, 200, 2, 202, Some(5));
        assert_eq!(res, Ok(()));
    }

    #[test]
    fn test_chain_id_doesnt_exist() {
        let cfg = mock_config();
        let res = cfg.validate_interop_timestamps(1, 200, 3, 215, Some(20));
        assert_eq!(res, Err(ConfigError::InteropNotEnabled));
    }
    #[test]
    fn test_interop_not_enabled_chain1() {
        let cfg = mock_config();
        let res = cfg.validate_interop_timestamps(1, 100, 2, 215, Some(20));
        assert_eq!(res, Err(ConfigError::InteropNotEnabled));
    }

    #[test]
    fn test_invalid_timestamp_invariant() {
        let cfg = mock_config();
        let res = cfg.validate_interop_timestamps(1, 200, 2, 195, Some(20));
        assert_eq!(
            res,
            Err(ConfigError::InvalidTimestampInvariant { initiating: 200, executing: 195 })
        );
    }

    #[test]
    fn test_expired_message_with_timeout() {
        let cfg = mock_config();
        let res = cfg.validate_interop_timestamps(1, 200, 2, 250, Some(20));
        assert_eq!(res, Err(ConfigError::InvalidInteropTimestamp(250)));
    }

    #[test]
    fn test_expired_message_without_timeout() {
        let cfg = mock_config();
        let res = cfg.validate_interop_timestamps(1, 200, 2, 215, None);
        assert_eq!(res, Err(ConfigError::InvalidInteropTimestamp(215)));
    }
}
