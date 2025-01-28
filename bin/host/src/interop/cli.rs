//! This module contains all CLI-specific code for the interop entrypoint.

use super::local_kv::DEFAULT_CHAIN_ID;
use crate::cli::{cli_styles, parse_b256, parse_bytes};
use alloy_primitives::{Bytes, B256};
use alloy_rlp::Decodable;
use anyhow::{anyhow, Result};
use clap::Parser;
use kona_proof_interop::PreState;
use maili_genesis::RollupConfig;
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf};

/// The host binary CLI application arguments.
#[derive(Default, Parser, Serialize, Clone, Debug)]
#[command(styles = cli_styles())]
pub struct InteropHostCli {
    /// Hash of the L1 head block, marking a static, trusted cutoff point for reading data from the
    /// L1 chain.
    #[clap(long, value_parser = parse_b256, env)]
    pub l1_head: B256,
    /// Agreed [PreState] to start from. Can be a [PreState::SuperRoot] or
    /// [PreState::TransitionState].
    ///
    /// [PreState]: kona_proof_interop::PreState
    #[clap(long, visible_alias = "l2-pre-state", value_parser = parse_bytes, env)]
    pub agreed_l2_pre_state: Bytes,
    /// Claimed L2 post-state to validate.
    #[clap(long, visible_alias = "l2-claim", value_parser = parse_b256, env)]
    pub claimed_l2_post_state: B256,
    /// Claimed L2 timestamp, corresponding to the L2 post-state.
    #[clap(long, visible_alias = "l2-timestamp", env)]
    pub claimed_l2_timestamp: u64,
    /// Addresses of L2 JSON-RPC endpoints to use (eth and debug namespace required).
    #[clap(
        long,
        visible_alias = "l2s",
        requires = "l1_node_address",
        requires = "l1_beacon_address",
        value_delimiter = ',',
        env
    )]
    pub l2_node_addresses: Option<Vec<String>>,
    /// Address of L1 JSON-RPC endpoint to use (eth and debug namespace required)
    #[clap(
        long,
        visible_alias = "l1",
        requires = "l2_node_address",
        requires = "l1_beacon_address",
        env
    )]
    pub l1_node_address: Option<String>,
    /// Address of the L1 Beacon API endpoint to use.
    #[clap(
        long,
        visible_alias = "beacon",
        requires = "l1_node_address",
        requires = "l2_node_addresses",
        env
    )]
    pub l1_beacon_address: Option<String>,
    /// The Data Directory for preimage data storage. Optional if running in online mode,
    /// required if running in offline mode.
    #[clap(
        long,
        visible_alias = "db",
        required_unless_present_all = ["l2_node_addresses", "l1_node_address", "l1_beacon_address"],
        env
    )]
    pub data_dir: Option<PathBuf>,
    /// Run the client program natively.
    #[clap(long, conflicts_with = "server", required_unless_present = "server")]
    pub native: bool,
    /// Run in pre-image server mode without executing any client program. If not provided, the
    /// host will run the client program in the host process.
    #[clap(long, conflicts_with = "native", required_unless_present = "native")]
    pub server: bool,
    /// Path to rollup configs. If provided, the host will use this config instead of attempting to
    /// look up the configs in the superchain registry.
    #[clap(long, alias = "rollup-cfgs", value_delimiter = ',', env)]
    pub rollup_config_paths: Option<Vec<PathBuf>>,
}

impl InteropHostCli {
    /// Returns `true` if the host is running in offline mode.
    pub const fn is_offline(&self) -> bool {
        self.l1_node_address.is_none() &&
            self.l2_node_addresses.is_none() &&
            self.l1_beacon_address.is_none()
    }

    /// Returns the active L2 chain ID based on the agreed L2 pre-state.
    pub fn active_l2_chain_id(&self) -> Result<u64> {
        let pre_state = match PreState::decode(&mut self.agreed_l2_pre_state.as_ref()) {
            Ok(pre_state) => pre_state,
            // If the pre-state is invalid, return a dummy chain ID.
            Err(_) => return Ok(DEFAULT_CHAIN_ID),
        };

        match pre_state {
            PreState::SuperRoot(super_root) => Ok(super_root
                .output_roots
                .first()
                .ok_or(anyhow!("output roots are empty"))?
                .chain_id),
            PreState::TransitionState(transition_state) => Ok(transition_state
                .pre_state
                .output_roots
                .get(
                    (transition_state.step as usize)
                        .min(transition_state.pre_state.output_roots.len() - 1),
                )
                .ok_or(anyhow!("no output root found"))?
                .chain_id),
        }
    }

    /// Reads the [RollupConfig]s from the file system and returns a map of L2 chain ID ->
    /// [RollupConfig]s.
    pub fn read_rollup_configs(&self) -> Result<HashMap<u64, RollupConfig>> {
        let rollup_config_paths = self.rollup_config_paths.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "No rollup config paths provided. Please provide a path to the rollup configs."
            )
        })?;

        rollup_config_paths.iter().try_fold(HashMap::default(), |mut acc, path| {
            // Read the serialized config from the file system.
            let ser_config = std::fs::read_to_string(path)
                .map_err(|e| anyhow!("Error reading RollupConfig file: {e}"))?;

            // Deserialize the config and return it.
            let cfg: RollupConfig = serde_json::from_str(&ser_config)
                .map_err(|e| anyhow!("Error deserializing RollupConfig: {e}"))?;

            acc.insert(cfg.l2_chain_id, cfg);
            Ok(acc)
        })
    }
}
