use alloy_network::Ethereum;
use alloy_provider::{Provider, RootProvider};
use anyhow::{Context as _, Ok, Result};
use clap::Args;
use glob::glob;
use kona_genesis::RollupConfig;
use kona_supervisor_core::config::RollupConfigSet;

use kona_interop::DependencySet;
use std::{net::IpAddr, path::PathBuf};
use tokio::{fs::File, io::AsyncReadExt};

/// Supervisor configuration arguments.
#[derive(Args, Debug)]
pub struct SupervisorArgs {
    /// L1 RPC source
    #[arg(long, env = "L1_RPC")]
    pub l1_rpc: String,

    /// L2 consensus rollup node RPC addresses.
    #[arg(long = "l2-consensus.nodes", env = "L2_CONSENSUS_NODES", value_delimiter = ',')]
    pub l2_consensus_nodes: Vec<String>,

    /// JWT secrets for L2 consensus nodes.
    #[arg(
        long = "l2-consensus.jwt-secret",
        env = "L2_CONSENSUS_JWT_SECRET",
        value_delimiter = ','
    )]
    pub l2_consensus_jwt_secret: Vec<String>,

    /// Directory to store supervisor data.
    #[arg(long, env = "DATADIR")]
    pub datadir: String,

    /// Optional endpoint to sync data from another supervisor.
    #[arg(long = "datadir.sync-endpoint", env = "DATADIR_SYNC_ENDPOINT")]
    pub datadir_sync_endpoint: Option<String>,

    /// Path to the dependency-set JSON config file.
    #[arg(long = "dependency-set", env = "DEPENDENCY_SET")]
    pub dependency_set: PathBuf,

    /// Path pattern to op-node rollup.json configs to load as a rollup config set.
    /// The pattern should use the glob sytax, e.g. '/configs/rollup-*.json'
    /// When using this flag, the L1 timestamps are loaded from the provided L1 RPC.
    #[arg(long = "rollup-config-paths", env = "ROLLUP_CONFIG_PATHS")]
    pub rollup_config_paths: PathBuf,

    /// IP address for the Supervisor RPC server to listen on.
    #[arg(long = "rpc.addr", env = "RPC_ADDR", default_value = "0.0.0.0")]
    pub rpc_address: IpAddr,

    /// Port for the Supervisor RPC server to listen on.
    #[arg(long = "rpc.port", env = "RPC_PORT", default_value_t = 8545)]
    pub rpc_port: u16,
}

impl SupervisorArgs {
    /// initialise and return the [`DependencySet`].
    pub async fn init_dependency_set(&self) -> Result<DependencySet> {
        let mut file = File::open(&self.dependency_set).await.with_context(|| {
            format!("Failed to open dependency set file '{}'", self.dependency_set.display())
        })?;

        let mut contents = String::new();
        file.read_to_string(&mut contents).await.with_context(|| {
            format!(
                "Failed to read content from dependency set file '{}'",
                self.dependency_set.display()
            )
        })?;

        let dependency_set: DependencySet = serde_json::from_str(&contents).with_context(|| {
            format!(
                "Failed to parse JSON from dependency set file '{}'",
                self.dependency_set.display()
            )
        })?;
        Ok(dependency_set)
    }

    async fn get_rollup_configs(&self) -> Result<Vec<RollupConfig>> {
        let pattern = self
            .rollup_config_paths
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("rollup_config_paths contains invalid UTF-8"))?;
        if pattern.is_empty() {
            return Err(anyhow::anyhow!("rollup_config_paths pattern is empty"));
        }

        let mut rollup_configs = Vec::new();
        for entry in glob(pattern)? {
            let path = entry?;
            let mut file = File::open(&path).await.with_context(|| {
                format!("Failed to open rollup config file '{}'", path.display())
            })?;
            let mut contents = String::new();
            file.read_to_string(&mut contents).await.with_context(|| {
                format!("Failed to read rollup config file '{}'", path.display())
            })?;

            let rollup_config: RollupConfig =
                serde_json::from_str(&contents).with_context(|| {
                    format!("Failed to parse JSON from rollup config file '{}'", path.display())
                })?;

            rollup_configs.push(rollup_config);
        }
        Ok(rollup_configs)
    }

    /// Initialise and return the rollup config set.
    pub async fn init_rollup_config_set(&self) -> Result<RollupConfigSet> {
        let l1_url = self
            .l1_rpc
            .parse()
            .with_context(|| format!("Failed to parse L1 RPC URL '{}'", &self.l1_rpc))?;
        let provider = RootProvider::<Ethereum>::new_http(l1_url);

        let mut rollup_config_set = RollupConfigSet::default();

        // Use the helper to get all configs
        let rollup_configs = self.get_rollup_configs().await?;

        for rollup_config in rollup_configs {
            let chain_id = rollup_config.l2_chain_id;

            let l1_genesis = provider
                .get_block_by_hash(rollup_config.genesis.l1.hash)
                .await
                .with_context(|| {
                    format!(
                        "Failed to fetch L1 genesis block for hash {}",
                        rollup_config.genesis.l1.hash
                    )
                })?;

            let l1_genesis = l1_genesis.ok_or_else(|| {
                anyhow::anyhow!(
                    "L1 genesis block not found for hash {}",
                    rollup_config.genesis.l1.hash
                )
            })?;

            rollup_config_set.add_from_rollup_config(
                chain_id,
                rollup_config,
                l1_genesis.header.timestamp,
            );
        }

        Ok(rollup_config_set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use kona_interop::{ChainDependency, DependencySet};
    use kona_registry::HashMap;
    use std::{fs::File, io::Write, net::Ipv4Addr};
    use tempfile::{NamedTempFile, tempdir};

    // Helper struct to parse SupervisorArgs within a test CLI structure
    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(flatten)]
        supervisor: SupervisorArgs,
    }

    #[test]
    fn test_supervisor_args_from_cli_required_only() {
        let cli = TestCli::parse_from([
            "test_app",
            "--l1-rpc",
            "http://localhost:8545",
            "--l2-consensus.nodes",
            "http://node1:8551,http://node2:8551",
            "--l2-consensus.jwt-secret",
            "secret1,secret2",
            "--datadir",
            "/tmp/supervisor_data",
            "--dependency-set",
            "/path/to/deps.json",
            "--rollup-config-paths",
            "/configs/rollup-*.json",
        ]);

        assert_eq!(cli.supervisor.l1_rpc, "http://localhost:8545");
        assert_eq!(
            cli.supervisor.l2_consensus_nodes,
            vec!["http://node1:8551".to_string(), "http://node2:8551".to_string()]
        );
        assert_eq!(
            cli.supervisor.l2_consensus_jwt_secret,
            vec!["secret1".to_string(), "secret2".to_string()]
        );
        assert_eq!(cli.supervisor.datadir, "/tmp/supervisor_data");
        assert_eq!(cli.supervisor.datadir_sync_endpoint, None);
        assert_eq!(cli.supervisor.dependency_set, PathBuf::from("/path/to/deps.json"));
        assert_eq!(cli.supervisor.rollup_config_paths, PathBuf::from("/configs/rollup-*.json"));
        assert_eq!(cli.supervisor.rpc_address, IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
        assert_eq!(cli.supervisor.rpc_port, 8545);
    }

    #[test]
    fn test_supervisor_args_from_cli_all_args() {
        let cli = TestCli::parse_from([
            "test_app",
            "--l1-rpc",
            "http://l1.example.com",
            "--l2-consensus.nodes",
            "http://consensus1",
            "--l2-consensus.jwt-secret",
            "jwt_secret_value",
            "--datadir",
            "/data",
            "--datadir.sync-endpoint",
            "http://sync.example.com",
            "--dependency-set",
            "/path/to/deps.json",
            "--rollup-config-paths",
            "/configs/rollup-*.json",
            "--rpc.addr",
            "192.168.1.100",
            "--rpc.port",
            "9001",
        ]);

        assert_eq!(cli.supervisor.l1_rpc, "http://l1.example.com");
        assert_eq!(cli.supervisor.l2_consensus_nodes, vec!["http://consensus1".to_string()]);
        assert_eq!(cli.supervisor.l2_consensus_jwt_secret, vec!["jwt_secret_value".to_string()]);
        assert_eq!(cli.supervisor.datadir, "/data");
        assert_eq!(
            cli.supervisor.datadir_sync_endpoint,
            Some("http://sync.example.com".to_string())
        );
        assert_eq!(cli.supervisor.dependency_set, PathBuf::from("/path/to/deps.json"));
        assert_eq!(cli.supervisor.rollup_config_paths, PathBuf::from("/configs/rollup-*.json"));
        assert_eq!(cli.supervisor.rpc_address, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)));
        assert_eq!(cli.supervisor.rpc_port, 9001);
    }

    #[tokio::test]
    async fn test_init_dependency_set_success() -> anyhow::Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        let json_content = r#"
        {
            "dependencies": {
                "1": {
                    "chainIndex": 10,
                    "activationTime": 1678886400,
                    "historyMinTime": 1609459200
                },
                "2": {
                    "chainIndex": 20,
                    "activationTime": 1678886401,
                    "historyMinTime": 1609459201
                }
            },
            "overrideMessageExpiryWindow": 3600
        }
        "#;
        temp_file.write_all(json_content.as_bytes())?;

        let args = SupervisorArgs {
            l1_rpc: "dummy".to_string(),
            l2_consensus_nodes: vec![],
            l2_consensus_jwt_secret: vec![],
            datadir: "dummy".to_string(),
            datadir_sync_endpoint: None,
            dependency_set: temp_file.path().to_path_buf(),
            rollup_config_paths: PathBuf::from("dummy/rollup_config_*.json"),
            rpc_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            rpc_port: 8545,
        };

        let result = args.init_dependency_set().await;
        assert!(result.is_ok(), "init_dependency_set should succeed");

        let loaded_depset = result.unwrap();
        let mut expected_dependencies = HashMap::default();
        expected_dependencies.insert(
            1,
            ChainDependency {
                chain_index: 10,
                activation_time: 1678886400,
                history_min_time: 1609459200,
            },
        );
        expected_dependencies.insert(
            2,
            ChainDependency {
                chain_index: 20,
                activation_time: 1678886401,
                history_min_time: 1609459201,
            },
        );

        let expected_depset = DependencySet {
            dependencies: expected_dependencies,
            override_message_expiry_window: 3600,
        };

        assert_eq!(loaded_depset, expected_depset);
        Ok(())
    }

    #[tokio::test]
    async fn test_init_dependency_set_file_not_found() -> anyhow::Result<()> {
        let args = SupervisorArgs {
            l1_rpc: "dummy".to_string(),
            l2_consensus_nodes: vec![],
            l2_consensus_jwt_secret: vec![],
            datadir: "dummy".to_string(),
            datadir_sync_endpoint: None,
            dependency_set: PathBuf::from("/path/to/non_existent_file.json"),
            rollup_config_paths: PathBuf::from("dummy/rollup_config_*.json"),
            rpc_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            rpc_port: 8545,
        };

        let result = args.init_dependency_set().await;
        let err = result.expect_err("init_dependency_set should have failed due to file not found");
        let io_error = err.downcast_ref::<std::io::Error>();
        assert!(io_error.is_some(), "Error should be an std::io::Error, but was: {:?}", err);
        assert_eq!(io_error.unwrap().kind(), std::io::ErrorKind::NotFound);
        Ok(())
    }

    #[tokio::test]
    async fn test_init_dependency_set_invalid_json() -> anyhow::Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(b"{ \"invalid_json\": ")?; // Malformed JSON

        let args = SupervisorArgs {
            l1_rpc: "dummy".to_string(),
            l2_consensus_nodes: vec![],
            l2_consensus_jwt_secret: vec![],
            datadir: "dummy".to_string(),
            datadir_sync_endpoint: None,
            dependency_set: temp_file.path().to_path_buf(),
            rollup_config_paths: PathBuf::from("dummy/rollup_config_*.json"),
            rpc_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            rpc_port: 8545,
        };

        let result = args.init_dependency_set().await;
        let err = result.expect_err("init_dependency_set should have failed due to invalid JSON");
        let json_error = err.downcast_ref::<serde_json::Error>();
        assert!(json_error.is_some(), "Error should be a serde_json::Error, but was: {:?}", err);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_rollup_configs_success() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("rollup-1.json");
        let mut file = File::create(&config_path)?;
        let json_content = r#"
        {
            "genesis": {
                "l1": {
                    "hash": "0x6c61a74b17fc1b6dc8ae9a2197e83871a20f57d1adf9c9acbf920bc44225744b",
                    "number": 18
                },
                "l2": {
                    "hash": "0xcde85e0f40c4c9921d40f2d4ee1a8794e76d615044a1176ae71fff0ee8cb2f40",
                    "number": 0
                },
                "l2_time": 1748932228,
                "system_config": {
                    "batcherAddr": "0xd3f2c5afb2d76f5579f326b0cd7da5f5a4126c35",
                    "overhead": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "scalar": "0x010000000000000000000000000000000000000000000000000c5fc500000558",
                    "gasLimit": 60000000,
                    "eip1559Params": "0x0000000000000000",
                    "operatorFeeParams": "0x0000000000000000000000000000000000000000000000000000000000000000"
                }
            },
            "block_time": 2,
            "max_sequencer_drift": 600,
            "seq_window_size": 3600,
            "channel_timeout": 300,
            "l1_chain_id": 3151908,
            "l2_chain_id": 2151908,
            "regolith_time": 0,
            "canyon_time": 0,
            "delta_time": 0,
            "ecotone_time": 0,
            "fjord_time": 0,
            "granite_time": 0,
            "holocene_time": 0,
            "isthmus_time": 0,
            "interop_time": 0,
            "batch_inbox_address": "0x00a4fe4c6aaa0729d7699c387e7f281dd64afa2a",
            "deposit_contract_address": "0xea0a3ca38bca6eb69cb7463b3fda7aa1616f9e09",
            "l1_system_config_address": "0x67872e274ce2d6f2dc937196f8ec9f7af82fae7e",
            "protocol_versions_address": "0xb74bb6ae1a1804d283d17e95620da9b9b0e6e0da",
            "chain_op_config": {
                "eip1559Elasticity": 6,
                "eip1559Denominator": 50,
                "eip1559DenominatorCanyon": 250
            }
        }
    "#;
        file.write_all(json_content.as_bytes())?;

        let args = SupervisorArgs {
            l1_rpc: "dummy".to_string(),
            l2_consensus_nodes: vec![],
            l2_consensus_jwt_secret: vec![],
            datadir: "dummy".to_string(),
            datadir_sync_endpoint: None,
            dependency_set: PathBuf::from("dummy.json"),
            rollup_config_paths: dir.path().join("rollup-*.json"),
            rpc_address: "127.0.0.1".parse().unwrap(),
            rpc_port: 8545,
        };

        let configs = args.get_rollup_configs().await?;
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].l2_chain_id, 2151908);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_rollup_configs_no_files() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let args = SupervisorArgs {
            l1_rpc: "dummy".to_string(),
            l2_consensus_nodes: vec![],
            l2_consensus_jwt_secret: vec![],
            datadir: "dummy".to_string(),
            datadir_sync_endpoint: None,
            dependency_set: PathBuf::from("dummy.json"),
            rollup_config_paths: dir.path().join("rollup-*.json"),
            rpc_address: "127.0.0.1".parse().unwrap(),
            rpc_port: 8545,
        };

        let configs = args.get_rollup_configs().await?;
        assert!(configs.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_get_rollup_configs_invalid_json() -> anyhow::Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("rollup-1.json");
        let mut file = File::create(&config_path)?;
        file.write_all(b"{ invalid json }")?;

        let args = SupervisorArgs {
            l1_rpc: "dummy".to_string(),
            l2_consensus_nodes: vec![],
            l2_consensus_jwt_secret: vec![],
            datadir: "dummy".to_string(),
            datadir_sync_endpoint: None,
            dependency_set: PathBuf::from("dummy.json"),
            rollup_config_paths: dir.path().join("rollup-*.json"),
            rpc_address: "127.0.0.1".parse().unwrap(),
            rpc_port: 8545,
        };

        let result = args.get_rollup_configs().await;
        assert!(result.is_err(), "Should fail on invalid JSON");
        Ok(())
    }
}
