//! Node Subcommand.

use crate::{cli::globals::GlobalArgs, node::RollupNode};
use alloy_rpc_types_engine::JwtSecret;
use anyhow::{Result, bail};
use clap::Parser;
use kona_genesis::RollupConfig;
use kona_registry::ROLLUP_CONFIGS;
use serde_json::from_reader;
use std::{fs::File, path::PathBuf};
use url::Url;

/// The Node subcommand.
///
/// For compatibility with the [op-node], relevant flags retain an alias that matches that
/// of the [op-node] CLI.
///
/// [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/flags.go
#[derive(Parser, Debug, Clone)]
#[command(about = "Runs the consensus node")]
pub struct NodeCommand {
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
}

impl NodeCommand {
    /// Run the Node subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let cfg = self.get_l2_config(args)?;
        let jwt_secret = self.jwt_secret().ok_or(anyhow::anyhow!("Invalid JWT secret"))?;

        RollupNode::builder(cfg)
            .with_jwt_secret(jwt_secret)
            .with_l1_provider_rpc_url(self.l1_eth_rpc)
            .with_l1_beacon_api_url(self.l1_beacon)
            .with_l2_provider_rpc_url(self.l2_provider_rpc)
            .with_l2_engine_rpc_url(self.l2_engine_rpc)
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
