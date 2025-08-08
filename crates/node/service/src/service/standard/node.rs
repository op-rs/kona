//! Contains the [`RollupNode`] implementation.
use crate::{
    DerivationActor, DerivationBuilder, EngineActor, EngineBuilder, InteropMode, L1WatcherRpc,
    L1WatcherRpcState, NetworkActor, NetworkBuilder, NetworkConfig, NodeMode, RollupNodeBuilder,
    RollupNodeService, RpcActor, SequencerConfig,
    actors::{SequencerActor, SequencerBuilder},
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
use kona_rpc::RpcBuilder;

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
    /// Whether to trust the L1 RPC.
    pub(crate) l1_trust_rpc: bool,
    /// The L1 beacon API.
    pub(crate) l1_beacon: OnlineBeaconClient,
    /// The L2 EL provider.
    pub(crate) l2_provider: RootProvider<Optimism>,
    /// Whether to trust the L2 RPC.
    pub(crate) l2_trust_rpc: bool,
    /// The [`EngineBuilder`] for the node.
    pub(crate) engine_builder: EngineBuilder,
    /// The [`RpcBuilder`] for the node.
    pub(crate) rpc_builder: Option<RpcBuilder>,
    /// The P2P [`NetworkConfig`] for the node.
    pub(crate) p2p_config: NetworkConfig,
    /// The [`SequencerConfig`] for the node.
    pub(crate) sequencer_config: SequencerConfig,
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

    type RpcActor = RpcActor;
    type EngineActor = EngineActor;
    type NetworkActor = NetworkActor;

    fn mode(&self) -> NodeMode {
        self.engine_builder.mode
    }

    fn da_watcher_builder(&self) -> L1WatcherRpcState {
        L1WatcherRpcState { rollup: self.config.clone(), l1_provider: self.l1_provider.clone() }
    }

    fn engine_builder(&self) -> EngineBuilder {
        self.engine_builder.clone()
    }

    fn sequencer_builder(&self) -> SequencerBuilder {
        SequencerBuilder {
            seq_cfg: self.sequencer_config.clone(),
            rollup_cfg: self.config.clone(),
            l1_provider: self.l1_provider.clone(),
            l1_trust_rpc: self.l1_trust_rpc,
            l2_provider: self.l2_provider.clone(),
            l2_trust_rpc: self.l2_trust_rpc,
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
            l1_trust_rpc: self.l1_trust_rpc,
            l1_beacon: self.l1_beacon.clone(),
            l2_provider: self.l2_provider.clone(),
            l2_trust_rpc: self.l2_trust_rpc,
            rollup_config: self.config.clone(),
            interop_mode: self.interop_mode,
        }
    }
}
