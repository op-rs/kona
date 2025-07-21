//! Contains the [`RollupNode`] implementation.
use crate::{
    DerivationActor, DerivationBuilder, EngineActor, EngineBuilder, InteropMode, L1WatcherRpc,
    L1WatcherRpcState, NetworkActor, NetworkBuilder, NetworkConfig, NodeMode, RollupNodeBuilder,
    RollupNodeService, RpcActor, RuntimeActor, SequencerConfig, SupervisorActor,
    SupervisorRpcServerExt,
    actors::{RuntimeState, SequencerActor, SequencerBuilder},
};
use alloy_provider::RootProvider;
use async_trait::async_trait;
use kona_derive::StatefulAttributesBuilder;
use op_alloy_network::Optimism;
use std::sync::Arc;

use kona_genesis::RollupConfig;
use kona_providers_alloy::{
    AlloyChainProvider, AlloyL2ChainProvider, OnlineBeaconClient, OnlinePipeline,
};
use kona_rpc::{RpcBuilder, SupervisorRpcConfig, SupervisorRpcServer};

/// The standard implementation of the [RollupNode] service, using the governance approved OP Stack
/// configuration of components.
#[derive(Debug)]
pub struct RollupNode {
    /// The rollup configuration.
    pub(crate) config: Arc<RollupConfig>,
    /// The interop mode for the node.
    pub(crate) interop_mode: InteropMode,
    /// The L1 EL provider.
    pub(crate) l1_provider: RootProvider,
    /// The L1 beacon API.
    pub(crate) l1_beacon: OnlineBeaconClient,
    /// The L2 EL provider.
    pub(crate) l2_provider: RootProvider<Optimism>,
    /// The [`EngineBuilder`] for the node.
    pub(crate) engine_builder: EngineBuilder,
    /// The [`RpcBuilder`] for the node.
    pub(crate) rpc_builder: Option<RpcBuilder>,
    /// The P2P [`NetworkConfig`] for the node.
    pub(crate) p2p_config: NetworkConfig,
    /// The [`RuntimeState`] for the runtime loading service.
    pub(crate) runtime_builder: Option<RuntimeState>,
    /// The [`SequencerConfig`] for the node.
    pub(crate) sequencer_config: SequencerConfig,
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

    type AttributesBuilder = StatefulAttributesBuilder<AlloyChainProvider, AlloyL2ChainProvider>;
    type SequencerActor = SequencerActor<SequencerBuilder>;

    type DerivationPipeline = OnlinePipeline;
    type DerivationActor = DerivationActor<DerivationBuilder>;

    type SupervisorExt = SupervisorRpcServerExt;
    type SupervisorActor = SupervisorActor<Self::SupervisorExt>;

    type RuntimeActor = RuntimeActor;
    type RpcActor = RpcActor;
    type EngineActor = EngineActor;
    type NetworkActor = NetworkActor;

    fn mode(&self) -> NodeMode {
        self.engine_builder.mode
    }

    fn da_watcher_builder(&self) -> L1WatcherRpcState {
        L1WatcherRpcState { rollup: self.config.clone(), l1_provider: self.l1_provider.clone() }
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

    fn runtime_builder(&self) -> Option<RuntimeState> {
        self.runtime_builder.clone()
    }

    fn engine_builder(&self) -> EngineBuilder {
        self.engine_builder.clone()
    }

    fn sequencer_builder(&self) -> SequencerBuilder {
        SequencerBuilder {
            seq_cfg: self.sequencer_config.clone(),
            rollup_cfg: self.config.clone(),
            l1_provider: self.l1_provider.clone(),
            l2_provider: self.l2_provider.clone(),
        }
    }

    fn rpc_builder(&self) -> Option<RpcBuilder> {
        self.rpc_builder.clone()
    }

    fn network_builder(&self) -> NetworkBuilder {
        NetworkBuilder::from(self.p2p_config.clone())
    }

    fn derivation_builder(&self) -> DerivationBuilder {
        DerivationBuilder {
            l1_provider: self.l1_provider.clone(),
            l1_beacon: self.l1_beacon.clone(),
            l2_provider: self.l2_provider.clone(),
            rollup_config: self.config.clone(),
            interop_mode: self.interop_mode,
        }
    }
}
