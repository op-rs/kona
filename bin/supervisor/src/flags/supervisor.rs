use clap::Args;
use std::net::IpAddr;

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
    pub dependency_set: Option<String>,

    /// IP address for the Supervisor RPC server to listen on.
    #[arg(long = "rpc.addr", env = "RPC_ADDR", default_value = "0.0.0.0")]
    pub rpc_address: IpAddr,

    /// Port for the Supervisor RPC server to listen on.
    #[arg(long = "rpc.port", env = "RPC_PORT", default_value_t = 8545)]
    pub rpc_port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    use std::net::Ipv4Addr;
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
        assert_eq!(cli.supervisor.dependency_set, None);
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
        assert_eq!(cli.supervisor.dependency_set, Some("/path/to/deps.json".to_string()));
        assert_eq!(cli.supervisor.rpc_address, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)));
        assert_eq!(cli.supervisor.rpc_port, 9001);
    }

    #[test]
    fn test_supervisor_args_from_env_vars() {
        unsafe {
            // Set environment variables
            std::env::set_var("L1_RPC", "env_l1_rpc");
            std::env::set_var("L2_CONSENSUS_NODES", "env_node1,env_node2");
            std::env::set_var("L2_CONSENSUS_JWT_SECRET", "env_jwt1,env_jwt2");
            std::env::set_var("DATADIR", "env_datadir");
            std::env::set_var("DATADIR_SYNC_ENDPOINT", "env_sync_endpoint");
            std::env::set_var("DEPENDENCY_SET", "env_dependency_set_path");
            std::env::set_var("RPC_ADDR", "10.0.0.1");
            std::env::set_var("RPC_PORT", "9002");
        }
        // Parse without CLI args, should pick up from env
        let cli = TestCli::parse_from(["test_app"]);

        assert_eq!(cli.supervisor.l1_rpc, "env_l1_rpc");
        assert_eq!(
            cli.supervisor.l2_consensus_nodes,
            vec!["env_node1".to_string(), "env_node2".to_string()]
        );
        assert_eq!(
            cli.supervisor.l2_consensus_jwt_secret,
            vec!["env_jwt1".to_string(), "env_jwt2".to_string()]
        );
        assert_eq!(cli.supervisor.datadir, "env_datadir");
        assert_eq!(cli.supervisor.datadir_sync_endpoint, Some("env_sync_endpoint".to_string()));
        assert_eq!(cli.supervisor.dependency_set, Some("env_dependency_set_path".to_string()));
        assert_eq!(cli.supervisor.rpc_address, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(cli.supervisor.rpc_port, 9002);

        unsafe {
            // Clean up environment variables
            std::env::remove_var("L1_RPC");
            std::env::remove_var("L2_CONSENSUS_NODES");
            std::env::remove_var("L2_CONSENSUS_JWT_SECRET");
            std::env::remove_var("DATADIR");
            std::env::remove_var("DATADIR_SYNC_ENDPOINT");
            std::env::remove_var("DEPENDENCY_SET");
            std::env::remove_var("RPC_ADDR");
            std::env::remove_var("RPC_PORT");
        }
    }
}
