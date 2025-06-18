//! Node Subcommand.

use crate::{
    flags::{GlobalArgs, P2PArgs, RpcArgs, SequencerArgs, SupervisorArgs},
    metrics::CliMetrics,
};
use alloy_rpc_types_engine::JwtSecret;
use anyhow::{Result, bail};
use backon::{ExponentialBuilder, Retryable};
use clap::Parser;
use kona_cli::metrics_args::MetricsArgs;
use kona_engine::EngineKind;
use kona_genesis::RollupConfig;
use kona_node_service::{RollupNode, RollupNodeService};
use kona_rpc::SupervisorRpcConfig;
use op_alloy_provider::ext::engine::OpEngineApi;
use serde_json::from_reader;
use std::{fs::File, path::PathBuf, sync::Arc};
use tracing::{debug, error};
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
    /// URL of the L1 execution client RPC API.
    #[arg(long, visible_alias = "l1", env = "KONA_NODE_L1_ETH_RPC")]
    pub l1_eth_rpc: Url,
    /// URL of the L1 beacon API.
    #[arg(long, visible_alias = "l1.beacon", env = "KONA_NODE_L1_BEACON")]
    pub l1_beacon: Url,
    /// URL of the engine API endpoint of an L2 execution client.
    #[arg(long, visible_alias = "l2", env = "KONA_NODE_L2_ENGINE_RPC")]
    pub l2_engine_rpc: Url,
    /// An L2 RPC Url.
    #[arg(long, visible_alias = "l2.provider", env = "KONA_NODE_L2_ETH_RPC")]
    pub l2_provider_rpc: Url,
    /// JWT secret for the auth-rpc endpoint of the execution client.
    /// This MUST be a valid path to a file containing the hex-encoded JWT secret.
    #[arg(long, visible_alias = "l2.jwt-secret", env = "KONA_NODE_L2_ENGINE_AUTH")]
    pub l2_engine_jwt_secret: Option<PathBuf>,
    /// Path to a custom L2 rollup configuration file
    /// (overrides the default rollup configuration from the registry)
    #[arg(long, visible_alias = "rollup-cfg", env = "KONA_NODE_ROLLUP_CONFIG")]
    pub l2_config_file: Option<PathBuf>,
    /// Engine kind.
    #[arg(
        long,
        visible_alias = "l2.enginekind",
        default_value = "geth",
        env = "KONA_NODE_L2_ENGINE_KIND",
        help = "DEPRECATED. The kind of engine client, used to control the behavior of optimism in respect to different types of engine clients. Supported engine clients are: [\"geth\", \"reth\", \"erigon\"]."
    )]
    pub l2_engine_kind: EngineKind,
    /// Poll interval (in seconds) for reloading the runtime config.
    /// Provides a backup for when config events are not being picked up.
    /// Disabled if `0`.
    #[arg(
        long,
        visible_alias = "l1.runtime-config-reload-interval",
        default_value = "600", // 10 minutes in seconds
        env = "KONA_NODE_L1_RUNTIME_CONFIG_RELOAD_INTERVAL",
    )]
    pub l1_runtime_config_reload_interval: u64,
    /// P2P CLI arguments.
    #[command(flatten)]
    pub p2p_flags: P2PArgs,
    /// RPC CLI arguments.
    #[command(flatten)]
    pub rpc_flags: RpcArgs,
    /// SEQUENCER CLI arguments.
    #[command(flatten)]
    pub sequencer_flags: SequencerArgs,
    /// SUPERVISOR CLI arguments.
    #[command(flatten)]
    pub supervisor_flags: SupervisorArgs,
}

impl Default for NodeCommand {
    fn default() -> Self {
        Self {
            l1_eth_rpc: Url::parse("http://localhost:8545").unwrap(),
            l1_beacon: Url::parse("http://localhost:5052").unwrap(),
            l2_engine_rpc: Url::parse("http://localhost:8551").unwrap(),
            l2_provider_rpc: Url::parse("http://localhost:8545").unwrap(),
            l2_engine_jwt_secret: None,
            l2_config_file: None,
            l1_runtime_config_reload_interval: 600,
            p2p_flags: P2PArgs::default(),
            rpc_flags: RpcArgs::default(),
            sequencer_flags: SequencerArgs::default(),
            l2_engine_kind: EngineKind::Geth,
            supervisor_flags: SupervisorArgs::default(),
        }
    }
}

impl NodeCommand {
    /// Initializes the logging system based on global arguments.
    pub fn init_logs(&self, args: &GlobalArgs) -> anyhow::Result<()> {
        // Filter out discovery warnings since they're very very noisy.
        let filter = tracing_subscriber::EnvFilter::from_default_env()
            .add_directive("discv5=error".parse()?);

        args.init_tracing(Some(filter))?;
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

    /// Validate the jwt secret if specified by exchanging capabilities with the engine.
    /// Since the engine client will fail if the jwt token is invalid, this allows to ensure
    /// that the jwt token passed as a cli arg is correct.
    pub async fn validate_jwt(&self, config: &RollupConfig) -> anyhow::Result<JwtSecret> {
        let jwt_secret = self.jwt_secret().ok_or(anyhow::anyhow!("Invalid JWT secret"))?;
        let engine_client = kona_engine::EngineClient::new_http(
            self.l2_engine_rpc.clone(),
            self.l2_provider_rpc.clone(),
            self.l1_eth_rpc.clone(),
            Arc::new(config.clone()),
            jwt_secret,
        );

        let exchange = || async {
            match engine_client.exchange_capabilities(vec![]).await {
                Ok(_) => {
                    debug!("Successfully exchanged capabilities with engine");
                    Ok(jwt_secret)
                }
                Err(e) => {
                    if e.to_string().contains("signature invalid") {
                        error!(
                            "Engine API JWT secret differs from the one specified by --l2.jwt-secret"
                        );
                        error!(
                            "Ensure that the JWT secret file specified is correct (by default it is `jwt.hex` in the current directory)"
                        );
                    }
                    bail!("Failed to exchange capabilities with engine: {}", e);
                }
            }
        };

        exchange
            .retry(ExponentialBuilder::default())
            .when(|e| !e.to_string().contains("signature invalid"))
            .notify(|_, duration| {
                debug!("Retrying engine capability handshake after {duration:?}");
            })
            .await
    }

    /// Run the Node subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let cfg = self.get_l2_config(args)?;
        let jwt_secret = self.validate_jwt(&cfg).await?;

        let supervisor_rpc_config =
            match (self.supervisor_flags.as_rpc_config(), self.supervisor_flags.rpc_enabled) {
                (Ok(cfg), true) => Some(cfg),
                (Err(e), true) => return Err(e),
                (_, false) => None,
            };
        let supervisor_rpc_config =
            supervisor_rpc_config.unwrap_or(SupervisorRpcConfig::default().disable());

        self.p2p_flags.check_ports()?;
        let disabled = self.p2p_flags.disabled;
        let p2p_config = self.p2p_flags.config(&cfg, args, Some(self.l1_eth_rpc.clone())).await?;
        let rpc_config = self.rpc_flags.into();

        let runtime_interval =
            std::time::Duration::from_secs(self.l1_runtime_config_reload_interval);

        RollupNode::builder(cfg)
            .with_jwt_secret(jwt_secret)
            .with_l1_provider_rpc_url(self.l1_eth_rpc)
            .with_l1_beacon_api_url(self.l1_beacon)
            .with_l2_provider_rpc_url(self.l2_provider_rpc)
            .with_l2_engine_rpc_url(self.l2_engine_rpc)
            .with_runtime_load_interval(runtime_interval)
            .with_p2p_config(p2p_config)
            .with_network_disabled(disabled)
            .with_rpc_config(rpc_config)
            .with_supervisor_rpc_config(supervisor_rpc_config)
            .build()
            .start()
            .await
            .map_err(Into::into)
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
                let Some(cfg) = args.rollup_config() else {
                    bail!("Failed to find l2 config for chain ID {}", args.l2_chain_id);
                };
                Ok(cfg)
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

    const fn default_flags() -> &'static [&'static str] {
        &[
            "--l1-eth-rpc",
            "http://localhost:8545",
            "--l1-beacon",
            "http://localhost:5052",
            "--l2-engine-rpc",
            "http://localhost:8551",
            "--l2-provider-rpc",
            "http://localhost:8545",
        ]
    }

    #[test]
    fn test_node_cli_missing_l1_eth_rpc() {
        let err = NodeCommand::try_parse_from(["node"]).unwrap_err();
        assert!(err.to_string().contains("--l1-eth-rpc"));
    }

    #[test]
    fn test_node_cli_missing_l1_beacon() {
        let err = NodeCommand::try_parse_from(["node", "--l1-eth-rpc", "http://localhost:8545"])
            .unwrap_err();
        assert!(err.to_string().contains("--l1-beacon"));
    }

    #[test]
    fn test_node_cli_missing_l2_engine_rpc() {
        let err = NodeCommand::try_parse_from([
            "node",
            "--l1-eth-rpc",
            "http://localhost:8545",
            "--l1-beacon",
            "http://localhost:5052",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("--l2-engine-rpc"));
    }

    #[test]
    fn test_node_cli_missing_l2_provider_rpc() {
        let err = NodeCommand::try_parse_from([
            "node",
            "--l1-eth-rpc",
            "http://localhost:8545",
            "--l1-beacon",
            "http://localhost:5052",
            "--l2-engine-rpc",
            "http://localhost:8551",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("--l2-provider-rpc"));
    }

    #[test]
    fn test_node_cli_defaults() {
        let args = NodeCommand::parse_from(["node"].iter().chain(default_flags().iter()).copied());
        assert_eq!(args.l2_engine_kind, EngineKind::Geth);
        assert_eq!(args.l1_runtime_config_reload_interval, 600);
    }

    #[test]
    fn test_node_cli_runtime_config_default() {
        let args = NodeCommand::parse_from(
            ["node", "--l1.runtime-config-reload-interval", "0"]
                .iter()
                .chain(default_flags().iter())
                .copied(),
        );
        assert_eq!(args.l1_runtime_config_reload_interval, 0);
    }

    #[test]
    fn test_node_cli_engine_kind() {
        let args = NodeCommand::parse_from(
            ["node", "--l2.enginekind", "reth"].iter().chain(default_flags().iter()).copied(),
        );
        assert_eq!(args.l2_engine_kind, EngineKind::Reth);
    }
}
