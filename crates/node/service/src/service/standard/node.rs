//! Contains the [`RollupNode`] implementation.

use crate::{
    EngineLauncher, L1WatcherRpc, L2ForkchoiceState, NodeMode, RollupNodeBuilder, RollupNodeError,
    RollupNodeService, SequencerNodeService, ValidatorNodeService, find_starting_forkchoice,
};
use alloy_primitives::Address;
use alloy_provider::RootProvider;
use async_trait::async_trait;
use kona_p2p::NetworkRpc;
use op_alloy_network::Optimism;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::info;

use kona_derive::traits::ChainProvider;
use kona_genesis::RollupConfig;
use kona_p2p::{Config, Network, NetworkBuilder};
use kona_protocol::BlockInfo;
use kona_providers_alloy::{
    AlloyChainProvider, AlloyL2ChainProvider, OnlineBeaconClient, OnlineBlobProvider,
    OnlinePipeline,
};
use kona_rpc::RpcLauncher;

/// The size of the cache used in the derivation pipeline's providers.
const DERIVATION_PROVIDER_CACHE_SIZE: usize = 1024;

/// The standard implementation of the [RollupNode] service, using the governance approved OP Stack
/// configuration of components.
#[derive(Debug)]
pub struct RollupNode {
    /// The rollup configuration.
    pub(crate) config: Arc<RollupConfig>,
    /// The mode of operation for the node.
    pub(crate) mode: NodeMode,
    /// The L1 EL provider.
    pub(crate) l1_provider: RootProvider,
    /// The L1 beacon API.
    pub(crate) l1_beacon: OnlineBeaconClient,
    /// The L2 EL provider.
    pub(crate) l2_provider: RootProvider<Optimism>,
    /// The [`EngineLauncher`] handles launching the engine api.
    pub(crate) engine_launcher: EngineLauncher,
    /// The [`RpcLauncher`] for the node.
    pub(crate) rpc_launcher: RpcLauncher,
    /// The P2P [`Config`] for the node.
    pub(crate) p2p_config: Option<Config>,
    /// Whether p2p networking is entirely disabled.
    pub(crate) network_disabled: bool,
}

impl RollupNode {
    /// Creates a new [RollupNodeBuilder], instantiated with the given [RollupConfig].
    pub fn builder(config: RollupConfig) -> RollupNodeBuilder {
        RollupNodeBuilder::new(config)
    }
}

#[async_trait]
impl RollupNodeService for RollupNode {
    fn mode(&self) -> NodeMode {
        self.mode
    }
}

#[async_trait]
impl SequencerNodeService for RollupNode {
    async fn start(&self) -> Result<(), Self::Error> {
        unimplemented!()
    }
}

#[async_trait]
impl ValidatorNodeService for RollupNode {
    type DataAvailabilityWatcher = L1WatcherRpc;
    type DerivationPipeline = OnlinePipeline;
    type Error = RollupNodeError;

    fn config(&self) -> &RollupConfig {
        &self.config
    }

    fn new_da_watcher(
        &self,
        new_da_tx: UnboundedSender<BlockInfo>,
        block_signer_tx: UnboundedSender<Address>,
        cancellation: CancellationToken,
    ) -> Self::DataAvailabilityWatcher {
        L1WatcherRpc::new(
            self.config.clone(),
            self.l1_provider.clone(),
            new_da_tx,
            block_signer_tx,
            cancellation,
        )
    }

    fn engine(&self) -> EngineLauncher {
        self.engine_launcher.clone()
    }

    fn rpc(&self) -> Option<RpcLauncher> {
        Some(self.rpc_launcher.clone())
    }

    async fn init_network(&self) -> Result<Option<(Network, NetworkRpc)>, Self::Error> {
        if self.network_disabled {
            return Ok(None);
        }
        let Some(ref p2p_config) = self.p2p_config else {
            warn!(
                target: "rollup_node",
                "No network configuration provided, skipping network initialization"
            );
            return Ok(None);
        };
        let chain_id = self.config.l2_chain_id;
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let p2p_module = NetworkRpc::new(tx);
        let builder = NetworkBuilder::from(p2p_config.clone())
            .with_chain_id(chain_id)
            .with_rpc_receiver(rx)
            .build()
            .map_err(RollupNodeError::Network)?;
        Ok(Some((builder, p2p_module)))
    }

    async fn init_derivation(&self) -> Result<(L2ForkchoiceState, OnlinePipeline), Self::Error> {
        // Create the caching L1/L2 EL providers for derivation.
        let mut l1_derivation_provider =
            AlloyChainProvider::new(self.l1_provider.clone(), DERIVATION_PROVIDER_CACHE_SIZE);
        let mut l2_derivation_provider = AlloyL2ChainProvider::new(
            self.l2_provider.clone(),
            self.config.clone(),
            DERIVATION_PROVIDER_CACHE_SIZE,
        );

        // Find the starting forkchoice state.
        let starting_forkchoice = find_starting_forkchoice(
            self.config.as_ref(),
            &mut l1_derivation_provider,
            &mut l2_derivation_provider,
        )
        .await?;

        info!(
            target: "rollup_node",
            unsafe = %starting_forkchoice.un_safe.block_info.number,
            safe = %starting_forkchoice.safe.block_info.number,
            finalized = %starting_forkchoice.finalized.block_info.number,
            "Found starting forkchoice state"
        );

        // Start the derivation pipeline's L1 origin 1 channel timeout before the L1 origin of the
        // safe head block.
        let starting_origin_num = starting_forkchoice.safe.l1_origin.number.saturating_sub(
            self.config.channel_timeout(starting_forkchoice.safe.block_info.timestamp),
        );
        let starting_origin =
            l1_derivation_provider.block_info_by_number(starting_origin_num).await?;

        let pipeline = OnlinePipeline::new(
            self.config.clone(),
            starting_forkchoice.safe,
            starting_origin,
            OnlineBlobProvider::init(self.l1_beacon.clone()).await,
            l1_derivation_provider,
            l2_derivation_provider,
        )
        .await?;

        Ok((starting_forkchoice, pipeline))
    }
}
