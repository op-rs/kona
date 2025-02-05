//! This module contains all CLI-specific code for the interop entrypoint.

use super::{InteropHintHandler, InteropLocalInputs};
use crate::{
    cli::{
        cli_styles,
        parser::{parse_b256, parse_bytes},
    },
    eth::http_provider,
    DiskKeyValueStore, MemoryKeyValueStore, OfflineHostBackend, OnlineHostBackend,
    OnlineHostBackendCfg, PreimageServer, SharedKeyValueStore, SplitKeyValueStore,
};
use alloy_primitives::{Bytes, B256};
use alloy_provider::{Provider, RootProvider};
use anyhow::{anyhow, Result};
use clap::Parser;
use kona_preimage::{
    BidirectionalChannel, Channel, HintReader, HintWriter, OracleReader, OracleServer,
};
use kona_proof::Hint;
use kona_proof_interop::HintType;
use kona_providers_alloy::{OnlineBeaconClient, OnlineBlobProvider};
use kona_std_fpvm::{FileChannel, FileDescriptor};
use maili_genesis::RollupConfig;
use op_alloy_network::Optimism;
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use tokio::{
    sync::RwLock,
    task::{self, JoinHandle},
};

/// The interop host application.
#[derive(Default, Parser, Serialize, Clone, Debug)]
#[command(styles = cli_styles())]
pub struct InteropHost {
    /// Hash of the L1 head block, marking a static, trusted cutoff point for reading data from the
    /// L1 chain.
    #[clap(long, value_parser = parse_b256, env)]
    pub l1_head: B256,
    /// Agreed [PreState] to start from.
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

impl InteropHost {
    /// Starts the [InteropHost] application.
    pub async fn start(self) -> Result<()> {
        if self.server {
            let hint = FileChannel::new(FileDescriptor::HintRead, FileDescriptor::HintWrite);
            let preimage =
                FileChannel::new(FileDescriptor::PreimageRead, FileDescriptor::PreimageWrite);

            self.start_server(hint, preimage).await?.await?
        } else {
            self.start_native().await
        }
    }

    /// Starts the preimage server, communicating with the client over the provided channels.
    async fn start_server<C>(&self, hint: C, preimage: C) -> Result<JoinHandle<Result<()>>>
    where
        C: Channel + Send + Sync + 'static,
    {
        let kv_store = self.create_key_value_store()?;

        let task_handle = if self.is_offline() {
            task::spawn(
                PreimageServer::new(
                    OracleServer::new(preimage),
                    HintReader::new(hint),
                    Arc::new(OfflineHostBackend::new(kv_store)),
                )
                .start(),
            )
        } else {
            let providers = self.create_providers().await?;
            let backend = OnlineHostBackend::new(
                self.clone(),
                kv_store.clone(),
                providers,
                InteropHintHandler,
            );

            task::spawn(
                PreimageServer::new(
                    OracleServer::new(preimage),
                    HintReader::new(hint),
                    Arc::new(backend),
                )
                .start(),
            )
        };

        Ok(task_handle)
    }

    /// Starts the host in native mode, running both the client and preimage server in the same
    /// process.
    async fn start_native(&self) -> Result<()> {
        let hint = BidirectionalChannel::new()?;
        let preimage = BidirectionalChannel::new()?;

        let server_task = self.start_server(hint.host, preimage.host).await?;
        let client_task = task::spawn(kona_client::single::run(
            OracleReader::new(preimage.client),
            HintWriter::new(hint.client),
            None,
        ));

        let (_, client_result) = tokio::try_join!(server_task, client_task)?;

        // Bubble up the exit status of the client program if execution completes.
        std::process::exit(client_result.is_err() as i32)
    }

    /// Returns `true` if the host is running in offline mode.
    pub const fn is_offline(&self) -> bool {
        self.l1_node_address.is_none() &&
            self.l2_node_addresses.is_none() &&
            self.l1_beacon_address.is_none() &&
            self.data_dir.is_some()
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

    /// Creates the key-value store for the host backend.
    fn create_key_value_store(&self) -> Result<SharedKeyValueStore> {
        let local_kv_store = InteropLocalInputs::new(self.clone());

        let kv_store: SharedKeyValueStore = if let Some(ref data_dir) = self.data_dir {
            let disk_kv_store = DiskKeyValueStore::new(data_dir.clone());
            let split_kv_store = SplitKeyValueStore::new(local_kv_store, disk_kv_store);
            Arc::new(RwLock::new(split_kv_store))
        } else {
            let mem_kv_store = MemoryKeyValueStore::new();
            let split_kv_store = SplitKeyValueStore::new(local_kv_store, mem_kv_store);
            Arc::new(RwLock::new(split_kv_store))
        };

        Ok(kv_store)
    }

    /// Creates the providers required for the preimage server backend.
    async fn create_providers(&self) -> Result<InteropProviders> {
        let l1_provider =
            http_provider(self.l1_node_address.as_ref().ok_or(anyhow!("Provider must be set"))?);

        let blob_provider = OnlineBlobProvider::init(OnlineBeaconClient::new_http(
            self.l1_beacon_address.clone().ok_or(anyhow!("Beacon API URL must be set"))?,
        ))
        .await;

        // Resolve all chain IDs to their corresponding providers.
        let l2_node_addresses =
            self.l2_node_addresses.as_ref().ok_or(anyhow!("L2 node addresses must be set"))?;
        let mut l2_providers = HashMap::default();
        for l2_node_address in l2_node_addresses {
            let l2_provider = http_provider::<Optimism>(l2_node_address);
            let chain_id = l2_provider.get_chain_id().await?;
            l2_providers.insert(chain_id, l2_provider);
        }

        Ok(InteropProviders { l1: l1_provider, blobs: blob_provider, l2s: l2_providers })
    }
}

impl OnlineHostBackendCfg for InteropHost {
    type Hint = Hint<HintType>;
    type Providers = InteropProviders;
}

/// The providers required for the single chain host.
#[derive(Debug, Clone)]
pub struct InteropProviders {
    /// The L1 EL provider.
    pub l1: RootProvider,
    /// The L1 beacon node provider.
    pub blobs: OnlineBlobProvider<OnlineBeaconClient>,
    /// The L2 EL providers, keyed by chain ID.
    pub l2s: HashMap<u64, RootProvider<Optimism>>,
}

impl InteropProviders {
    /// Returns the L2 [RootProvider] for the given chain ID.
    pub fn l2(&self, chain_id: &u64) -> Result<&RootProvider<Optimism>> {
        self.l2s
            .get(chain_id)
            .ok_or_else(|| anyhow!("No provider found for chain ID: {}", chain_id))
    }
}
