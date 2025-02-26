//! Node Subcommand.

use crate::{cli::globals::GlobalArgs, pilot::Pilot};
use alloy_rpc_types_engine::JwtSecret;
use anyhow::{bail, Result};
use clap::Args;
use kona_genesis::RollupConfig;
use kona_registry::ROLLUP_CONFIGS;
use serde_json::from_reader;
use std::{fs::File, path::PathBuf, sync::Arc};
use url::Url;

/// The Node subcommand.
#[derive(Debug, Clone, Args)]
#[non_exhaustive]
pub struct NodeCommand {
    /// URL of the engine API endpoint of an L2 execution client.
    ///
    /// This argument is read in as the `--l2` flag or the `L2_ENGINE_RPC`
    /// environment variable, simplifying compatibility with the [op-node][op-node].
    ///
    /// [op-node]: https://github.com/ethereum-optimism/optimism/blob/develop/op-node/flags/flags.go
    #[clap(long = "l2", env = "L2_ENGINE_RPC")]
    pub l2_engine_rpc: Url,
    /// An L2 RPC Url.
    #[clap(long = "l2-provider", env = "L2_PROVIDER_RPC")]
    pub l2_provider_rpc: Url,
    /// Path to a custom L2 rollup configuration file
    /// (overrides the default rollup configuration from the registry)
    #[clap(long = "l2-config-file")]
    pub l2_config_file: Option<PathBuf>,
    /// JWT secret for the auth-rpc endpoint of the execution client.
    /// This MUST be a valid path to a file containing the hex-encoded JWT secret.
    #[clap(long = "l2-engine-jwt-secret", env = "L2_ENGINE_JWT_SECRET")]
    pub l2_engine_jwt_secret: Option<PathBuf>,
}

impl NodeCommand {
    /// Run the Node subcommand.
    pub async fn run(self, args: &GlobalArgs) -> anyhow::Result<()> {
        let cfg = self.get_l2_config(args)?;
        let jwt_secret = self.jwt_secret().ok_or(anyhow::anyhow!("Invalid JWT secret"))?;
        let pilot = Pilot::new(
            self.l2_engine_rpc.clone(),
            self.l2_provider_rpc.clone(),
            Arc::new(cfg),
            jwt_secret,
        );
        pilot.sync().await
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
