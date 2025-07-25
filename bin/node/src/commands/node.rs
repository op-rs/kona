//! Node Subcommand.

use crate::{
    flags::{GlobalArgs, P2PArgs, RpcArgs, SequencerArgs},
    metrics::{CliMetrics, init_rollup_config_metrics},
};
use alloy_provider::{Provider, ProviderBuilder};
use alloy_rpc_types_engine::JwtSecret;
use anyhow::{Result, bail};
use clap::Parser;
use kona_cli::{LogConfig, metrics_args::MetricsArgs};
use kona_genesis::RollupConfig;
use kona_node_service::{NodeMode, RollupNode, RollupNodeService};
use kona_registry::scr_rollup_config_by_alloy_ident;
use op_alloy_provider::ext::engine::OpEngineApi;
use serde_json::from_reader;
use std::{fs::File, path::PathBuf, sync::Arc, time::Duration};
use strum::IntoEnumIterator;
use tracing::{debug, error, info};
use url::Url;

/// The Node subcommand.
///
/// For compatibility with the [op-node], relevant flags retain an alias that matches that
/// of the [op-node] CLI.
///
/// [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/flags.go
#[derive(Parser, PartialEq, Debug, Clone)]
#[command(about = "Runs the consensus node")]
pub struct NodeCommand {
    /// The mode to run the node in.
    #[arg(
        long = "mode",
        default_value_t = NodeMode::Validator,
        env = "KONA_NODE_MODE",
        help = format!(
            "The mode to run the node in. Supported modes are: {}",
            NodeMode::iter()
                .map(|mode| format!("\"{}\"", mode.to_string()))
                .collect::<Vec<_>>()
                .join(", ")
        )
    )]
    pub node_mode: NodeMode,
    /// URL of the L1 execution client RPC API.
    #[arg(
        long,
        visible_alias = "l1",
        env = "KONA_NODE_L1_ETH_RPC",
        default_value = "http://localhost:8545"
    )]
    pub l1_eth_rpc: Url,
    /// URL of the L1 beacon API.
    #[arg(
        long,
        visible_alias = "l1.beacon",
        env = "KONA_NODE_L1_BEACON",
        default_value = "http://localhost:5052"
    )]
    pub l1_beacon: Url,
    /// URL of the engine API endpoint of an L2 execution client.
    #[arg(
        long,
        visible_alias = "l2",
        env = "KONA_NODE_L2_ENGINE_RPC",
        default_value = "http://localhost:8551"
    )]
    pub l2_engine_rpc: Url,
    /// An L2 RPC Url.
    #[arg(
        long,
        visible_alias = "l2.provider",
        env = "KONA_NODE_L2_ETH_RPC",
        default_value = "http://localhost:8546"
    )]
    pub l2_provider_rpc: Url,
    /// JWT secret for the auth-rpc endpoint of the execution client.
    /// This MUST be a valid path to a file containing the hex-encoded JWT secret.
    #[arg(long, visible_alias = "l2.jwt-secret", env = "KONA_NODE_L2_ENGINE_AUTH")]
    pub l2_engine_jwt_secret: Option<PathBuf>,
    /// Path to a custom L2 rollup configuration file
    /// (overrides the default rollup configuration from the registry)
    #[arg(long, visible_alias = "rollup-cfg", env = "KONA_NODE_ROLLUP_CONFIG")]
    pub l2_config_file: Option<PathBuf>,
    /// P2P CLI arguments.
    #[command(flatten)]
    pub p2p_flags: P2PArgs,
    /// RPC CLI arguments.
    #[command(flatten)]
    pub rpc_flags: RpcArgs,
    /// SEQUENCER CLI arguments.
    #[command(flatten)]
    pub sequencer_flags: SequencerArgs,
}

impl Default for NodeCommand {
    fn default() -> Self {
        Self {
            l1_eth_rpc: Url::parse("http://localhost:8545").unwrap(),
            l1_beacon: Url::parse("http://localhost:5052").unwrap(),
            l2_engine_rpc: Url::parse("http://localhost:8551").unwrap(),
            l2_provider_rpc: Url::parse("http://localhost:8546").unwrap(),
            l2_engine_jwt_secret: None,
            l2_config_file: None,
            node_mode: NodeMode::Validator,
            p2p_flags: P2PArgs::default(),
            rpc_flags: RpcArgs::default(),
            sequencer_flags: SequencerArgs::default(),
        }
    }
}

impl NodeCommand {
    /// Initializes the logging system based on global arguments.
    pub fn init_logs(&self, args: &GlobalArgs) -> anyhow::Result<()> {
        // Filter out discovery warnings since they're very very noisy.
        let filter = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("discv5=error".parse()?);

        LogConfig::new(args.log_args.clone()).init_tracing_subscriber(Some(filter))?;
        Ok(())
    }

    /// Initializes CLI metrics for the Node subcommand.
    pub fn init_cli_metrics(&self, args: &MetricsArgs) -> anyhow::Result<()> {
        if !args.enabled {
            debug!("CLI metrics are disabled");
            return Ok(());
        }
        metrics::gauge!(
            CliMetrics::IDENTIFIER,
            &[
                (CliMetrics::P2P_PEER_SCORING_LEVEL, self.p2p_flags.scoring.to_string()),
                (CliMetrics::P2P_TOPIC_SCORING_ENABLED, self.p2p_flags.topic_scoring.to_string()),
                (CliMetrics::P2P_BANNING_ENABLED, self.p2p_flags.ban_enabled.to_string()),
                (
                    CliMetrics::P2P_PEER_REDIALING,
                    self.p2p_flags.peer_redial.unwrap_or(0).to_string()
                ),
                (CliMetrics::P2P_FLOOD_PUBLISH, self.p2p_flags.gossip_flood_publish.to_string()),
                (CliMetrics::P2P_DISCOVERY_INTERVAL, self.p2p_flags.discovery_interval.to_string()),
                (
                    CliMetrics::P2P_ADVERTISE_IP,
                    self.p2p_flags
                        .advertise_ip
                        .map(|ip| ip.to_string())
                        .unwrap_or(String::from("0.0.0.0"))
                ),
                (CliMetrics::P2P_ADVERTISE_TCP_PORT, self.p2p_flags.advertise_tcp_port.to_string()),
                (CliMetrics::P2P_ADVERTISE_UDP_PORT, self.p2p_flags.advertise_udp_port.to_string()),
                (CliMetrics::P2P_PEERS_LO, self.p2p_flags.peers_lo.to_string()),
                (CliMetrics::P2P_PEERS_HI, self.p2p_flags.peers_hi.to_string()),
                (CliMetrics::P2P_GOSSIP_MESH_D, self.p2p_flags.gossip_mesh_d.to_string()),
                (CliMetrics::P2P_GOSSIP_MESH_D_LO, self.p2p_flags.gossip_mesh_dlo.to_string()),
                (CliMetrics::P2P_GOSSIP_MESH_D_HI, self.p2p_flags.gossip_mesh_dhi.to_string()),
                (CliMetrics::P2P_GOSSIP_MESH_D_LAZY, self.p2p_flags.gossip_mesh_dlazy.to_string()),
                (CliMetrics::P2P_BAN_DURATION, self.p2p_flags.ban_duration.to_string()),
            ]
        )
        .set(1);
        Ok(())
    }

    /// Get the JWT secret for the engine API.
    /// JWT validation is now performed as part of RPC endpoint validation.
    pub async fn validate_jwt(&self, _config: &RollupConfig) -> anyhow::Result<JwtSecret> {
        self.jwt_secret().ok_or_else(|| {
            anyhow::anyhow!("No JWT secret available. Please provide a JWT secret file using --l2-engine-jwt-secret or ensure jwt.hex exists in the current directory.")
        })
    }

    /// Validate that all RPC endpoints are reachable by performing simple RPC calls.
    /// This helps catch configuration issues early with helpful error messages.
    pub async fn validate_rpc_endpoints(&self) -> anyhow::Result<()> {
        // Validate L1 ETH RPC
        self.validate_eth_rpc(&self.l1_eth_rpc, "L1 ETH RPC", "--l1-eth-rpc").await?;

        // Validate L2 Provider RPC
        self.validate_eth_rpc(&self.l2_provider_rpc, "L2 Provider RPC", "--l2-provider-rpc")
            .await?;

        // Validate L2 Engine RPC using Engine API
        self.validate_engine_rpc(&self.l2_engine_rpc, "L2 Engine RPC", "--l2-engine-rpc").await?;

        // Validate L1 Beacon API
        self.validate_beacon_api(&self.l1_beacon, "L1 Beacon API", "--l1-beacon").await?;

        Ok(())
    }

    /// Validate an ETH RPC endpoint by making a simple chain ID call.
    async fn validate_eth_rpc(&self, url: &Url, name: &str, flag: &str) -> anyhow::Result<()> {
        let provider = ProviderBuilder::new().connect_http(url.clone());

        // Use a timeout for the RPC call
        let call_result =
            tokio::time::timeout(Duration::from_secs(10), provider.get_chain_id()).await;

        match call_result {
            Ok(Ok(_)) => {
                debug!("{} endpoint {} is reachable", name, url);
                Ok(())
            }
            Ok(Err(transport_err)) => {
                let is_default = self.is_default_url(url, flag);
                self.format_rpc_error(name, url, flag, is_default, &transport_err.to_string())
            }
            Err(_) => {
                let is_default = self.is_default_url(url, flag);
                self.format_rpc_error(
                    name,
                    url,
                    flag,
                    is_default,
                    "connection timeout after 10 seconds",
                )
            }
        }
    }

    /// Validate an Engine API endpoint by making a simple exchange capabilities call.
    /// Uses the actual JWT token to properly validate the engine endpoint.
    async fn validate_engine_rpc(&self, url: &Url, name: &str, flag: &str) -> anyhow::Result<()> {
        // Get the actual JWT secret that will be used for the engine connection
        let jwt_secret = self.jwt_secret().ok_or_else(|| {
            anyhow::anyhow!("No JWT secret available for Engine API validation. Please provide a JWT secret file using --l2-engine-jwt-secret or ensure jwt.hex exists in the current directory.")
        })?;

        // Create a minimal rollup config for the engine client
        // We only need this for the client construction, not for actual operation
        let dummy_config = Arc::new(RollupConfig::default());

        let engine_client = kona_engine::EngineClient::new_http(
            url.clone(),
            url.clone(), // Use same URL for l2_provider as we're only testing engine endpoint
            url.clone(), // Use same URL for l1_provider as we're only testing engine endpoint
            dummy_config,
            jwt_secret,
        );

        // Use a timeout for the RPC call
        let call_result = tokio::time::timeout(
            Duration::from_secs(10),
            engine_client.exchange_capabilities(vec![]),
        )
        .await;

        match call_result {
            Ok(Ok(_)) => {
                debug!("{} endpoint {} is reachable and JWT authentication is valid", name, url);
                Ok(())
            }
            Ok(Err(transport_err)) => {
                let error_msg = transport_err.to_string();

                // If the error is about JWT/signature, provide specific guidance
                if error_msg.contains("signature invalid") ||
                    error_msg.contains("unauthorized") ||
                    error_msg.contains("401")
                {
                    let is_default = self.is_default_url(url, flag);
                    let auth_error = format!(
                        "{} endpoint is reachable but JWT authentication failed: {}{}\n\
                         URL: {}\n\
                         Error: {}\n\
                         \n\
                         To fix this:\n\
                         - Ensure the JWT secret file contains the correct token for the engine\n\
                         - Use --l2-engine-jwt-secret to specify the correct JWT secret file\n\
                         - Verify the engine is configured with the same JWT secret",
                        name,
                        url,
                        if is_default { " (using default value)" } else { "" },
                        url,
                        error_msg
                    );
                    bail!("{}", auth_error)
                } else {
                    // Other errors indicate the endpoint is unreachable
                    let is_default = self.is_default_url(url, flag);
                    self.format_rpc_error(name, url, flag, is_default, &error_msg)
                }
            }
            Err(_) => {
                let is_default = self.is_default_url(url, flag);
                self.format_rpc_error(
                    name,
                    url,
                    flag,
                    is_default,
                    "connection timeout after 10 seconds",
                )
            }
        }
    }

    /// Validate a Beacon API endpoint by making a simple config call.
    async fn validate_beacon_api(&self, url: &Url, name: &str, flag: &str) -> anyhow::Result<()> {
        let client = reqwest::Client::new();
        let endpoint = format!("{}/eth/v1/config/spec", url);

        let call_result =
            tokio::time::timeout(Duration::from_secs(10), client.get(&endpoint).send()).await;

        match call_result {
            Ok(Ok(response)) if response.status().is_success() => {
                debug!("{} endpoint {} is reachable", name, url);
                Ok(())
            }
            Ok(Ok(response)) => {
                let is_default = self.is_default_url(url, flag);
                let error_msg = format!(
                    "HTTP {} - {}",
                    response.status(),
                    response.status().canonical_reason().unwrap_or("Unknown")
                );
                self.format_rpc_error(name, url, flag, is_default, &error_msg)
            }
            Ok(Err(req_err)) => {
                let is_default = self.is_default_url(url, flag);
                self.format_rpc_error(name, url, flag, is_default, &req_err.to_string())
            }
            Err(_) => {
                let is_default = self.is_default_url(url, flag);
                self.format_rpc_error(
                    name,
                    url,
                    flag,
                    is_default,
                    "connection timeout after 10 seconds",
                )
            }
        }
    }

    /// Check if the given URL matches the default for this flag.
    fn is_default_url(&self, url: &Url, flag: &str) -> bool {
        let default_cmd = Self::default();
        match flag {
            "--l1-eth-rpc" => url == &default_cmd.l1_eth_rpc,
            "--l1-beacon" => url == &default_cmd.l1_beacon,
            "--l2-engine-rpc" => url == &default_cmd.l2_engine_rpc,
            "--l2-provider-rpc" => url == &default_cmd.l2_provider_rpc,
            _ => false,
        }
    }

    /// Format a helpful error message for RPC validation failures.
    fn format_rpc_error(
        &self,
        name: &str,
        url: &Url,
        flag: &str,
        is_default: bool,
        error_msg: &str,
    ) -> anyhow::Result<()> {
        let default_note =
            if is_default { " (using default value)".to_string() } else { String::new() };

        let error = format!(
            "{} endpoint is unreachable or invalid: {}{}\n\
             URL: {}\n\
             Error: {}\n\
             \n\
             To fix this:\n\
             - Ensure the service is running and accessible\n\
             - Use {} to specify a different endpoint URL",
            name, url, default_note, url, error_msg, flag
        );

        bail!("{}", error)
    }

    /// Run the Node subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let cfg = self.get_l2_config(args)?;

        // Validate all RPC endpoints early to provide helpful error messages
        if let Err(e) = self.validate_rpc_endpoints().await {
            eprintln!("{}", e);
            std::process::abort();
        }

        // If metrics are enabled, initialize the global cli metrics.
        args.metrics.enabled.then(|| init_rollup_config_metrics(&cfg));

        let jwt_secret = self.validate_jwt(&cfg).await?;

        self.p2p_flags.check_ports()?;
        let p2p_config = self.p2p_flags.config(&cfg, args, Some(self.l1_eth_rpc.clone())).await?;
        let rpc_config = self.rpc_flags.into();

        info!(
            target: "rollup_node",
            chain_id = cfg.l2_chain_id.id(),
            "Starting rollup node services"
        );
        for hf in cfg.hardforks.to_string().lines() {
            info!(target: "rollup_node", "{hf}");
        }

        RollupNode::builder(cfg)
            .with_mode(self.node_mode)
            .with_jwt_secret(jwt_secret)
            .with_l1_provider_rpc_url(self.l1_eth_rpc)
            .with_l1_beacon_api_url(self.l1_beacon)
            .with_l2_provider_rpc_url(self.l2_provider_rpc)
            .with_l2_engine_rpc_url(self.l2_engine_rpc)
            .with_p2p_config(p2p_config)
            .with_rpc_config(rpc_config)
            .with_sequencer_config(self.sequencer_flags.config())
            .build()
            .start()
            .await
            .map_err(|e| {
                error!(target: "rollup_node", "Failed to start rollup node service: {e}");
                anyhow::anyhow!("{}", e)
            })?;

        Ok(())
    }

    /// Get the L2 rollup config, either from a file or the superchain registry.
    pub fn get_l2_config(&self, args: &GlobalArgs) -> Result<RollupConfig> {
        match &self.l2_config_file {
            Some(path) => {
                debug!("Loading l2 config from file: {:?}", path);
                let file = File::open(path)
                    .map_err(|e| anyhow::anyhow!("Failed to open l2 config file: {}", e))?;
                from_reader(file).map_err(|e| anyhow::anyhow!("Failed to parse l2 config: {}", e))
            }
            None => {
                debug!("Loading l2 config from superchain registry");
                let Some(cfg) = scr_rollup_config_by_alloy_ident(&args.l2_chain_id) else {
                    bail!("Failed to find l2 config for chain ID {}", args.l2_chain_id);
                };
                Ok(cfg.clone())
            }
        }
    }

    /// Returns the JWT secret for the engine API
    /// using the provided [PathBuf]. If the file is not found,
    /// it will return the default JWT secret.
    pub fn jwt_secret(&self) -> Option<JwtSecret> {
        if let Some(path) = &self.l2_engine_jwt_secret {
            if let Ok(secret) = std::fs::read_to_string(path) {
                return JwtSecret::from_hex(secret).ok();
            }
        }
        Self::default_jwt_secret()
    }

    /// Uses the current directory to attempt to read
    /// the JWT secret from a file named `jwt.hex`.
    /// If the file is not found, it will return `None`.
    pub fn default_jwt_secret() -> Option<JwtSecret> {
        let cur_dir = std::env::current_dir().ok()?;
        std::fs::read_to_string(cur_dir.join("jwt.hex")).map_or_else(
            |_| {
                use std::io::Write;
                let secret = JwtSecret::random();
                if let Ok(mut file) = File::create("jwt.hex") {
                    if let Err(e) =
                        file.write_all(alloy_primitives::hex::encode(secret.as_bytes()).as_bytes())
                    {
                        tracing::error!("Failed to write JWT secret to file: {:?}", e);
                    }
                }
                Some(secret)
            },
            |content| JwtSecret::from_hex(content).ok(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_cli_defaults() {
        let args = NodeCommand::parse_from(["node"]);
        assert_eq!(args.node_mode, NodeMode::Validator);
    }

    #[test]
    fn test_node_cli_default_values() {
        let args = NodeCommand::parse_from(["node"]);
        assert_eq!(args.l1_eth_rpc.as_str(), "http://localhost:8545/");
        assert_eq!(args.l1_beacon.as_str(), "http://localhost:5052/");
        assert_eq!(args.l2_engine_rpc.as_str(), "http://localhost:8551/");
        assert_eq!(args.l2_provider_rpc.as_str(), "http://localhost:8546/");
    }

    #[test]
    fn test_node_cli_custom_values() {
        let args = NodeCommand::parse_from([
            "node",
            "--l1-eth-rpc",
            "http://custom:8545",
            "--l1-beacon",
            "http://custom:5052",
            "--l2-engine-rpc",
            "http://custom:8551",
            "--l2-provider-rpc",
            "http://custom:8546",
        ]);
        assert_eq!(args.l1_eth_rpc.as_str(), "http://custom:8545/");
        assert_eq!(args.l1_beacon.as_str(), "http://custom:5052/");
        assert_eq!(args.l2_engine_rpc.as_str(), "http://custom:8551/");
        assert_eq!(args.l2_provider_rpc.as_str(), "http://custom:8546/");
    }

    #[test]
    fn test_is_default_url() {
        let args = NodeCommand::default();
        assert!(args.is_default_url(&args.l1_eth_rpc, "--l1-eth-rpc"));
        assert!(args.is_default_url(&args.l1_beacon, "--l1-beacon"));
        assert!(args.is_default_url(&args.l2_engine_rpc, "--l2-engine-rpc"));
        assert!(args.is_default_url(&args.l2_provider_rpc, "--l2-provider-rpc"));

        let custom_url = Url::parse("http://custom:8546").unwrap();
        assert!(!args.is_default_url(&custom_url, "--l1-eth-rpc"));
    }
}
