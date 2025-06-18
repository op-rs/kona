//! Supervisor RPC CLI Flags

use std::{fs, net::IpAddr, path::PathBuf};

use alloy_rpc_types_engine::JwtSecret;
use anyhow::{Ok, anyhow};
use clap::Parser;
use kona_rpc::SupervisorRpcConfig;
use std::net::SocketAddr;

/// Supervisor CLI Flags
#[derive(Parser, Clone, Debug, PartialEq, Eq)]
pub struct SupervisorArgs {
    /// Enable Supervisor Websocket
    #[arg(
        long = "supervisor.rpc-enabled",
        default_value = "false",
        env = "KONA_NODE_SUPERVISOR_RPC_ENABLED"
    )]
    pub rpc_enabled: bool,

    /// IP to bind Supervisor Websocket RPC server to.
    #[arg(
        long = "supervisor.ip.address",
        default_value = "0.0.0.0",
        env = "KONA_NODE_SUPERVISOR_IP"
    )]
    pub ip_address: IpAddr,

    /// TCP port to serve the supervisor rpc. Any available system port if set to 0.
    #[arg(long = "supervisor.port", default_value = "9333", env = "KONA_NODE_SEQUENCER_PORT")]
    pub port: u16,

    /// JWT secret for supervisor websocket authentication
    #[arg(
        long = "supervisor.jwt.secret",
        env = "KONA_NODE_SUPERVISOR_JWT_SECRET",
        conflicts_with = "jwt_secret_file"
    )]
    pub jwt_secret: Option<String>,

    /// Path to file containing JWT secret for supervisor websocket authentication
    #[arg(
        long = "supervisor.jwt.secret.file",
        env = "KONA_NODE_SUPERVISOR_JWT_SECRET_FILE",
        conflicts_with = "jwt_secret"
    )]
    pub jwt_secret_file: Option<PathBuf>,
}

impl Default for SupervisorArgs {
    fn default() -> Self {
        // Construct default values using the clap parser.
        // This works since none of the cli flags are required.
        Self::parse_from::<[_; 0], &str>([])
    }
}

impl SupervisorArgs {
    /// Returns the [`SocketAddr`] for the supervisor rpc.
    pub const fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip_address, self.port)
    }

    /// Load the JWT secret for the supervisor websocket authentication.
    pub fn load_jwt_secret(&self) -> anyhow::Result<JwtSecret> {
        match (&self.jwt_secret, &self.jwt_secret_file) {
            (None, None) => Err(anyhow!("JWT secret required for websocket authentication")),
            (None, Some(file)) => {
                let secret = fs::read_to_string(file)
                    .map_err(|e| {
                        anyhow!("Failed to read JWT secret file {}: {}", file.display(), e)
                    })?
                    .trim()
                    .to_string();
                JwtSecret::from_hex(secret).map_err(|e| anyhow::anyhow!(e))
            }
            (Some(secret), _) => JwtSecret::from_hex(secret).map_err(|e| anyhow::anyhow!(e)),
        }
    }

    /// Constructs the [`SupervisorRpcConfig`] from the [`SupervisorArgs`].
    ///
    /// ## Errors
    ///
    /// This method errors if the JWT secret is unable to be loaded using
    /// [`SupervisorArgs::load_jwt_secret`].
    pub fn as_rpc_config(&self) -> anyhow::Result<SupervisorRpcConfig> {
        let jwt_secret = self.load_jwt_secret()?;
        let socket_address = self.socket_addr();
        Ok(SupervisorRpcConfig { rpc_disabled: !self.rpc_enabled, socket_address, jwt_secret })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// A mock command that uses the SupervisorArgs.
    #[derive(Parser, Debug, Clone)]
    #[command(about = "Mock command")]
    struct MockCommand {
        /// Supervisor CLI Flags
        #[clap(flatten)]
        pub supervisor: SupervisorArgs,
    }

    #[test]
    fn test_supervisor_args_defaults() {
        let args = MockCommand::parse_from(["test"]);
        assert_eq!(args.supervisor, SupervisorArgs::default());
        assert!(!args.supervisor.rpc_enabled);
        let expected_ip = "0.0.0.0".parse::<IpAddr>().unwrap();
        assert_eq!(args.supervisor.ip_address, expected_ip);
        assert_eq!(args.supervisor.port, 9333);
        assert_eq!(args.supervisor.jwt_secret, None);
        assert_eq!(args.supervisor.jwt_secret_file, None);

        let expected_addr = SocketAddr::new(expected_ip, 9333);
        assert_eq!(args.supervisor.socket_addr(), expected_addr);

        args.supervisor.as_rpc_config().unwrap_err();
    }

    #[test]
    fn test_supervisor_args_rpc_enabled() {
        let args = MockCommand::parse_from(["test", "--supervisor.rpc-enabled"]);
        assert!(args.supervisor.rpc_enabled);

        args.supervisor.as_rpc_config().unwrap_err();
    }

    #[test]
    fn test_supervisor_args_ip_address() {
        let args = MockCommand::parse_from(["test", "--supervisor.ip.address", "127.0.0.1"]);
        assert_eq!(args.supervisor.ip_address, "127.0.0.1".parse::<IpAddr>().unwrap());

        let args = MockCommand::parse_from(["test", "--supervisor.ip.address", "::1"]);
        assert_eq!(args.supervisor.ip_address, "::1".parse::<IpAddr>().unwrap());

        let args = MockCommand::parse_from(["test", "--supervisor.ip.address", "192.168.1.100"]);
        assert_eq!(args.supervisor.ip_address, "192.168.1.100".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_supervisor_args_port() {
        let args = MockCommand::parse_from(["test", "--supervisor.port", "8080"]);
        assert_eq!(args.supervisor.port, 8080);

        let args = MockCommand::parse_from(["test", "--supervisor.port", "0"]);
        assert_eq!(args.supervisor.port, 0);

        let args = MockCommand::parse_from(["test", "--supervisor.port", "65535"]);
        assert_eq!(args.supervisor.port, 65535);
    }

    #[test]
    fn test_supervisor_args_jwt_secret() {
        let args = MockCommand::parse_from([
            "test",
            "--supervisor.jwt.secret",
            "1fceaf7306ed8342ac35f9eb00b7291e19f737b1e632aff53ffeeac87a536f9d",
        ]);
        assert_eq!(
            args.supervisor.jwt_secret,
            Some("1fceaf7306ed8342ac35f9eb00b7291e19f737b1e632aff53ffeeac87a536f9d".to_string())
        );
        assert_eq!(args.supervisor.jwt_secret_file, None);

        let cfg = args.supervisor.as_rpc_config().unwrap();
        assert!(cfg.rpc_disabled);
    }

    #[test]
    fn test_supervisor_args_jwt_conflicts() {
        // This should fail due to conflicts_with constraint
        let result = MockCommand::try_parse_from([
            "test",
            "--supervisor.jwt.secret",
            "direct-secret",
            "--supervisor.jwt.secret.file",
            "/path/to/file",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_supervisor_args_invalid_ip() {
        let result = MockCommand::try_parse_from(["test", "--supervisor.ip.address", "invalid-ip"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_supervisor_args_invalid_port() {
        let result = MockCommand::try_parse_from([
            "test",
            "--supervisor.port",
            "65536", // Port out of range
        ]);
        assert!(result.is_err());

        let result = MockCommand::try_parse_from([
            "test",
            "--supervisor.port",
            "-1", // Negative port
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_supervisor_args_empty_jwt_secret() {
        let args = MockCommand::parse_from(["test", "--supervisor.jwt.secret", ""]);
        assert_eq!(args.supervisor.jwt_secret, Some("".to_string()));
    }

    #[test]
    fn test_supervisor_args_nonexistent_jwt_file() {
        // This should parse successfully, but file validation would happen at runtime
        let args = MockCommand::parse_from([
            "test",
            "--supervisor.jwt.secret.file",
            "/nonexistent/path/to/secret",
        ]);
        assert_eq!(args.supervisor.jwt_secret_file, Some("/nonexistent/path/to/secret".into()));
    }

    #[test]
    fn test_supervisor_args_clone_and_debug() {
        let args = MockCommand::parse_from([
            "test",
            "--supervisor.rpc-enabled",
            "--supervisor.jwt.secret",
            "1fceaf7306ed8342ac35f9eb00b7291e19f737b1e632aff53ffeeac87a536f9d",
        ]);

        // Test Debug trait
        let debug_output = format!("{:?}", args.supervisor);
        assert!(debug_output.contains("SupervisorArgs"));
        assert!(debug_output.contains("rpc_enabled: true"));

        let cfg = args.supervisor.as_rpc_config().unwrap();
        assert!(!cfg.rpc_disabled);
    }

    #[test]
    fn test_supervisor_args_partial_eq() {
        let args1 = SupervisorArgs {
            rpc_enabled: true,
            ip_address: "127.0.0.1".parse().unwrap(),
            port: 8080,
            jwt_secret: Some("secret".to_string()),
            jwt_secret_file: None,
        };

        let args2 = SupervisorArgs {
            rpc_enabled: true,
            ip_address: "127.0.0.1".parse().unwrap(),
            port: 8080,
            jwt_secret: Some("secret".to_string()),
            jwt_secret_file: None,
        };

        let args3 = SupervisorArgs {
            rpc_enabled: false,
            ip_address: "127.0.0.1".parse().unwrap(),
            port: 8080,
            jwt_secret: Some("secret".to_string()),
            jwt_secret_file: None,
        };

        assert_eq!(args1, args2);
        assert_ne!(args1, args3);
    }
}
