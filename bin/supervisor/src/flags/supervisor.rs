use alloy_network::Ethereum;
use alloy_provider::{Provider, RootProvider};
use anyhow::{Context as _, Ok, Result};
use clap::Args;
use glob::glob;
use kona_interop::DependencySet;
use kona_supervisor_core::{
    config::{Config, RollupConfigSet},
    syncnode::ManagedNodeConfig,
};
use std::{
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};
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
    pub datadir: PathBuf,

    /// Optional endpoint to sync data from another supervisor.
    #[arg(long = "datadir.sync-endpoint", env = "DATADIR_SYNC_ENDPOINT")]
    pub datadir_sync_endpoint: Option<String>,

    /// Path to the dependency-set JSON config file.
    #[arg(long = "dependency-set", env = "DEPENDENCY_SET")]
    pub dependency_set: PathBuf,

    /// Path pattern to op-node rollup.json configs to load as a rollup config set.
    /// The pattern should use the glob sytax, e.g. '/configs/rollup-*.json'
    /// When using this flag, the L1 timestamps are loaded from the provided L1 RPC",
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

    /// initialise and return the rollup config set.
    pub async fn init_rollup_config_set(&self) -> Result<RollupConfigSet> {
        let pattern = self.rollup_config_paths.to_string_lossy();
        if pattern.is_empty() {
            return Err(anyhow::anyhow!("rollup_config_paths pattern is empty"));
        }

        let l1_url = self
            .l1_rpc
            .parse()
            .with_context(|| format!("Failed to parse L1 RPC URL '{}'", &self.l1_rpc))?;
        let provider = RootProvider::<Ethereum>::new_http(l1_url);

        let mut rollup_config_set = RollupConfigSet::default();
        for entry in glob(&pattern)? {
            let path = entry?;
            let mut file = File::open(&path).await.with_context(|| {
                format!("Failed to open rollup config file '{}'", path.display())
            })?;
            let mut contents = String::new();
            file.read_to_string(&mut contents).await.with_context(|| {
                format!("Failed to read rollup config file '{}'", path.display())
            })?;

            let rollup_config: kona_genesis::RollupConfig = serde_json::from_str(&contents)
                .with_context(|| {
                    format!("Failed to parse JSON from rollup config file '{}'", path.display())
                })?;

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

    /// initialise and return the managed nodes configuration.
    pub fn init_managed_nodes_config(&self) -> Vec<ManagedNodeConfig> {
        let mut managed_nodes = Vec::new();
        let default_secret = self.l2_consensus_jwt_secret.first().unwrap();
        for (i, rpc_url) in self.l2_consensus_nodes.iter().enumerate() {
            let secret = self.l2_consensus_jwt_secret.get(i).unwrap_or(default_secret);

            managed_nodes
                .push(ManagedNodeConfig { url: rpc_url.clone(), jwt_path: secret.clone() });
        }
        managed_nodes
    }

    /// initialise and return the Supervisor [`Config`].
    pub async fn init_config(&self) -> Result<Config> {
        let dependency_set = self.init_dependency_set().await?;
        let rollup_config_set = self.init_rollup_config_set().await?;

        let rpc_addr = SocketAddr::new(self.rpc_address, self.rpc_port);

        Ok(Config {
            l1_rpc: self.l1_rpc.clone(),
            l2_consensus_nodes_config: self.init_managed_nodes_config(),
            datadir: self.datadir.clone(),
            rpc_addr,
            dependency_set,
            rollup_config_set,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use kona_interop::{ChainDependency, DependencySet};
    use kona_registry::HashMap;
    use std::{io::Write, net::Ipv4Addr};
    use tempfile::NamedTempFile;

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
        assert_eq!(cli.supervisor.datadir, PathBuf::from("/tmp/supervisor_data"));
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
        assert_eq!(cli.supervisor.datadir, PathBuf::from("/data"));
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
            datadir: PathBuf::from("dummy"),
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
            datadir: PathBuf::from("dummy"),
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
            datadir: PathBuf::from("dummy"),
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
}
