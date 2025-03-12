//! Contains the node CLI.

use alloy_rpc_types_engine::JwtSecret;
use anyhow::{Result, bail};
use clap::Parser;
use kona_cli::{cli_styles, init_prometheus_server, init_tracing_subscriber};
use kona_engine::{EngineKind, SyncConfig, SyncMode};
use kona_genesis::RollupConfig;
use kona_node_service::{RollupNode, RollupNodeService};
use kona_registry::ROLLUP_CONFIGS;
use serde_json::from_reader;
use std::{fs::File, path::PathBuf};
use tracing::debug;
use tracing_subscriber::EnvFilter;
use url::Url;

use crate::flags::{GlobalArgs, P2PArgs};

/// Kona's Consensus Node CLI.
///
/// For compatibility with the [op-node], relevant flags retain an alias that matches that
/// of the [op-node] CLI.
///
/// [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/flags.go
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, styles = cli_styles(), long_about = None)]
pub struct Cli {
    /// Global arguments for the CLI.
    #[clap(flatten)]
    pub global: GlobalArgs,
    /// URL of the L1 execution client RPC API.
    #[clap(long, visible_alias = "l1", env = "L1_ETH_RPC")]
    pub l1_eth_rpc: Url,
    /// URL of the L1 beacon API.
    #[clap(long, visible_alias = "l1.beacon", env = "L1_BEACON")]
    pub l1_beacon: Url,
    /// URL of the engine API endpoint of an L2 execution client.
    #[clap(long, visible_alias = "l2", env = "L2_ENGINE_RPC")]
    pub l2_engine_rpc: Url,
    /// An L2 RPC Url.
    #[clap(long, visible_alias = "l2.provider", env = "L2_ETH_RPC")]
    pub l2_provider_rpc: Url,
    /// JWT secret for the auth-rpc endpoint of the execution client.
    /// This MUST be a valid path to a file containing the hex-encoded JWT secret.
    #[clap(long, visible_alias = "l2.jwt-secret", env = "L2_ENGINE_AUTH")]
    pub l2_engine_jwt_secret: Option<PathBuf>,
    /// Path to a custom L2 rollup configuration file
    /// (overrides the default rollup configuration from the registry)
    #[clap(long, visible_alias = "rollup-cfg")]
    pub l2_config_file: Option<PathBuf>,
    /// Engine kind.
    #[clap(
        long,
        visible_alias = "l2.enginekind",
        default_value = "geth",
        env = "L2_ENGINE_KIND",
        help = "The kind of engine client, used to control the behavior of optimism in respect to different types of engine clients. Supported engine clients are: [\"geth\", \"reth\", \"erigon\"]."
    )]
    pub l2_engine_kind: EngineKind,
    /// P2P CLI arguments.
    #[clap(flatten)]
    pub p2p_flags: P2PArgs,
}

impl Cli {
    /// Runs the CLI.
    pub fn run(self) -> Result<()> {
        // Initialize the telemetry stack.
        Self::init_stack(self.global.v, self.global.metrics_port)?;

        // Starts the node.
        Self::run_until_ctrl_c(self.start())
    }

    /// Initialize the tracing stack and Prometheus metrics recorder.
    ///
    /// This function should be called at the beginning of the program.
    pub fn init_stack(verbosity: u8, metrics_port: u16) -> Result<()> {
        // Initialize the tracing subscriber.
        init_tracing_subscriber(verbosity, None::<EnvFilter>)?;

        // Start the Prometheus metrics server.
        init_prometheus_server(metrics_port)?;

        Ok(())
    }

    /// Starts the node.
    pub async fn start(self) -> Result<()> {
        let cfg = self.get_l2_config(&self.global)?;
        let jwt_secret = self.jwt_secret().ok_or(anyhow::anyhow!("Invalid JWT secret"))?;
        let kind = self.l2_engine_kind;
        let sync_config = SyncConfig {
            sync_mode: SyncMode::ExecutionLayer,
            // Skip sync start check is deprecated in the op-node,
            // so set it to false here without needing a cli flag.
            skip_sync_start_check: false,
            supports_post_finalization_elsync: kind.supports_post_finalization_elsync(),
        };

        let ip = self.p2p_flags.listen_ip;
        let tcp = self.p2p_flags.listen_tcp_port;
        let udp = self.p2p_flags.listen_udp_port;
        let gossip_addr = std::net::SocketAddr::new(ip, tcp);
        let disc_addr = std::net::SocketAddr::new(ip, udp);

        let Some(mut private_key) = self.p2p_flags.private_key else {
            // TODO: try to read the private key from the path
            // self.p2p_flags.priv_path
            anyhow::bail!("Private key file not implemented");
        };
        let keypair = libp2p_identity::Keypair::secp256k1_from_der(&mut private_key.0)
            .map_err(|_| anyhow::anyhow!("Failed to parse private key"))?;

        RollupNode::builder(cfg)
            .with_jwt_secret(jwt_secret)
            .with_sync_config(sync_config)
            .with_l1_provider_rpc_url(self.l1_eth_rpc)
            .with_l1_beacon_api_url(self.l1_beacon)
            .with_l2_provider_rpc_url(self.l2_provider_rpc)
            .with_l2_engine_rpc_url(self.l2_engine_rpc)
            .with_gossip_addr(gossip_addr)
            .with_disc_addr(disc_addr)
            .with_keypair(keypair)
            .build()
            .start()
            .await
            .map_err(Into::into)
    }

    /// Run until ctrl-c is pressed.
    pub fn run_until_ctrl_c<F>(fut: F) -> Result<()>
    where
        F: std::future::Future<Output = Result<()>>,
    {
        let rt = Self::tokio_runtime().map_err(|e| anyhow::anyhow!(e))?;
        rt.block_on(fut)
    }

    /// Creates a new default tokio multi-thread [Runtime](tokio::runtime::Runtime) with all
    /// features enabled
    pub fn tokio_runtime() -> Result<tokio::runtime::Runtime, std::io::Error> {
        tokio::runtime::Builder::new_multi_thread().enable_all().build()
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
                let Some(cfg) = ROLLUP_CONFIGS.get(&args.l2_chain_id).cloned() else {
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
        match std::fs::read_to_string(cur_dir.join("jwt.hex")) {
            Ok(content) => JwtSecret::from_hex(content).ok(),
            Err(_) => {
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
            }
        }
    }
}
