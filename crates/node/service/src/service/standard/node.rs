//! Contains the [`RollupNode`] implementation.

use crate::{
    EngineLauncher, L1WatcherRpc, NodeMode, RollupNodeBuilder, RollupNodeError, RollupNodeService,
    RuntimeLauncher, SupervisorRpcServerExt,
};
use alloy_primitives::Address;
use alloy_provider::RootProvider;
use async_trait::async_trait;
use kona_protocol::BlockInfo;
use op_alloy_network::Optimism;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use kona_genesis::RollupConfig;
use kona_p2p::{Config, Network, NetworkBuilder};
use kona_providers_alloy::{
    AlloyChainProvider, AlloyL2ChainProvider, OnlineBeaconClient, OnlineBlobProvider,
    OnlinePipeline,
};
use kona_rpc::{
    L1WatcherQueries, NetworkRpc, RpcLauncher, SupervisorRpcConfig, SupervisorRpcServer,
};

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
    pub(crate) p2p_config: Config,
    /// The [`RuntimeLauncher`] for the runtime loading service.
    pub(crate) runtime_launcher: RuntimeLauncher,
    /// The supervisor rpc server config.
    pub(crate) supervisor_rpc: SupervisorRpcConfig,
}

impl RollupNode {
    /// Creates a new [RollupNodeBuilder], instantiated with the given [RollupConfig].
    pub fn builder(config: RollupConfig) -> RollupNodeBuilder {
        RollupNodeBuilder::new(config)
    }
}

#[async_trait]
impl RollupNodeService for RollupNode {
    type DataAvailabilityWatcher = L1WatcherRpc;
    type DerivationPipeline = OnlinePipeline;
    type SupervisorExt = SupervisorRpcServerExt;
    type Error = RollupNodeError;

    fn mode(&self) -> NodeMode {
        self.mode
    }

    fn config(&self) -> &RollupConfig {
        &self.config
    }

    fn new_da_watcher(
        &self,
        head_updates: watch::Sender<Option<BlockInfo>>,
        finalized_updates: watch::Sender<Option<BlockInfo>>,
        block_signer_tx: mpsc::Sender<Address>,
        cancellation: CancellationToken,
        l1_watcher_inbound_queries: Option<tokio::sync::mpsc::Receiver<L1WatcherQueries>>,
    ) -> Self::DataAvailabilityWatcher {
        L1WatcherRpc::new(
            self.config.clone(),
            self.l1_provider.clone(),
            head_updates,
            finalized_updates,
            block_signer_tx,
            cancellation,
            l1_watcher_inbound_queries,
        )
    }

    async fn supervisor_ext(&self) -> Option<Self::SupervisorExt> {
        if self.supervisor_rpc.is_disabled() {
            return None;
        }
        let (events_tx, events_rx) = tokio::sync::broadcast::channel(1024);
        let (control_tx, control_rx) = tokio::sync::broadcast::channel(1024);
        let server = SupervisorRpcServer::new(
            events_rx,
            control_tx,
            self.supervisor_rpc.jwt_secret,
            self.supervisor_rpc.socket_address,
        );
        // TODO: handle this error properly by encapsulating this logic in a trait-abstracted
        // launcher.
        let handle = server.launch().await.ok()?;
        Some(SupervisorRpcServerExt::new(handle, events_tx, control_rx))
    }

    fn runtime(&self) -> RuntimeLauncher {
        self.runtime_launcher.clone()
    }

    fn engine(&self) -> EngineLauncher {
        self.engine_launcher.clone()
    }

    fn rpc(&self) -> RpcLauncher {
        self.rpc_launcher.clone()
    }

    async fn init_network(&self) -> Result<(Network, NetworkRpc), Self::Error> {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let p2p_module = NetworkRpc::new(tx);
        let builder = NetworkBuilder::from(self.p2p_config.clone())
            .with_rpc_receiver(rx)
            .build()
            .map_err(RollupNodeError::Network)?;
        Ok((builder, p2p_module))
    }

    async fn init_derivation(&self) -> Result<OnlinePipeline, Self::Error> {
        // Create the caching L1/L2 EL providers for derivation.
        let l1_derivation_provider =
            AlloyChainProvider::new(self.l1_provider.clone(), DERIVATION_PROVIDER_CACHE_SIZE);
        let l2_derivation_provider = AlloyL2ChainProvider::new(
            self.l2_provider.clone(),
            self.config.clone(),
            DERIVATION_PROVIDER_CACHE_SIZE,
        );

        let pipeline = OnlinePipeline::new_uninitialized(
            self.config.clone(),
            OnlineBlobProvider::init(self.l1_beacon.clone()).await,
            l1_derivation_provider,
            l2_derivation_provider,
        );

        Ok(pipeline)
    }
}
