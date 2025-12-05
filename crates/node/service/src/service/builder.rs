//! Contains the builder for the [`RollupNode`].

use crate::{
    EngineConfig, InteropMode, NetworkConfig, RollupNode, SequencerConfig,
    service::node::L1NodeConfig,
};
use alloy_primitives::Bytes;
use alloy_provider::RootProvider;
use alloy_rpc_client::RpcClient;
use alloy_transport_http::{
    AuthLayer, Http, HyperClient,
    hyper_util::{client::legacy::Client, rt::TokioExecutor},
};
use http_body_util::Full;
use op_alloy_network::Optimism;
use std::sync::Arc;
use tower::ServiceBuilder;
use url::Url;

use kona_genesis::{L1ChainConfig, RollupConfig};
use kona_providers_alloy::OnlineBeaconClient;
use kona_rpc::RpcBuilder;

/// The L1 configuration for the [`RollupNodeBuilder`].
#[derive(Debug)]
pub struct L1BuilderConfig {
    /// The L1 Chain Configuration.
    pub chain_config: L1ChainConfig,
    /// Whether to trust the L1 RPC.
    pub trust_rpc: bool,
    /// The L1 beacon API URL.
    pub beacon: Url,
    /// The L1 engine API URL.
    pub engine: Url,
    /// The duration in seconds of an L1 slot. This can be used to hardcode a fixed slot
    /// duration if the l1-beacon's slot configuration is not available.
    pub slot_duration: Option<u64>,
}

/// The [`RollupNodeBuilder`] is used to construct a [`RollupNode`] service.
#[derive(Debug)]
pub struct RollupNodeBuilder {
    /// The rollup configuration.
    pub config: RollupConfig,
    /// The L1 chain configuration.
    pub l1_config: L1BuilderConfig,
    /// Whether to trust the L2 RPC.
    pub l2_trust_rpc: bool,
    /// Engine builder configuration.
    pub engine_config: EngineConfig,
    /// The [`NetworkConfig`].
    pub p2p_config: NetworkConfig,
    /// An RPC Configuration.
    pub rpc_config: Option<RpcBuilder>,
    /// The [`SequencerConfig`].
    pub sequencer_config: Option<SequencerConfig>,
    /// Whether to run the node in interop mode.
    pub interop_mode: InteropMode,
}

impl RollupNodeBuilder {
    /// Creates a new [`RollupNodeBuilder`] with the given [`RollupConfig`].
    pub fn new(
        config: RollupConfig,
        l1_config: L1BuilderConfig,
        l2_trust_rpc: bool,
        engine_config: EngineConfig,
        p2p_config: NetworkConfig,
        rpc_config: Option<RpcBuilder>,
    ) -> Self {
        Self {
            config,
            l1_config,
            l2_trust_rpc,
            engine_config,
            p2p_config,
            rpc_config,
            interop_mode: InteropMode::default(),
            sequencer_config: None,
        }
    }

    /// Sets the [`EngineConfig`] on the [`RollupNodeBuilder`].
    pub fn with_engine_config(self, engine_config: EngineConfig) -> Self {
        Self { engine_config, ..self }
    }

    /// Sets the [`RpcBuilder`] on the [`RollupNodeBuilder`].
    pub fn with_rpc_config(self, rpc_config: Option<RpcBuilder>) -> Self {
        Self { rpc_config, ..self }
    }

    /// Appends the [`SequencerConfig`] to the builder.
    pub fn with_sequencer_config(self, sequencer_config: SequencerConfig) -> Self {
        Self { sequencer_config: Some(sequencer_config), ..self }
    }

    /// Assembles the [`RollupNode`] service.
    ///
    /// ## Panics
    ///
    /// Panics if:
    /// - The L1 provider RPC URL is not set.
    /// - The L1 beacon API URL is not set.
    /// - The L2 provider RPC URL is not set.
    /// - The L2 engine URL is not set.
    /// - The jwt secret is not set.
    /// - The P2P config is not set.
    /// - The rollup boost args are not set.
    pub fn build(self) -> RollupNode {
        let l1_provider = RootProvider::new_http(self.l1_config.engine.clone());
        let mut l1_beacon = OnlineBeaconClient::new_http(self.l1_config.beacon.to_string());

        if let Some(l1_slot_duration) = self.l1_config.slot_duration {
            l1_beacon = l1_beacon.with_l1_slot_duration(l1_slot_duration);
        }

        let jwt_secret = self.engine_config.l2_jwt_secret;
        let hyper_client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();

        let auth_layer = AuthLayer::new(jwt_secret);
        let service = ServiceBuilder::new().layer(auth_layer).service(hyper_client);

        let layer_transport = HyperClient::with_service(service);
        let http_hyper = Http::with_client(layer_transport, self.engine_config.l2_url.clone());
        let rpc_client = RpcClient::new(http_hyper, false);
        let l2_provider = RootProvider::<Optimism>::new(rpc_client);

        let rollup_config = Arc::new(self.config);
        let l1_config = Arc::new(self.l1_config.chain_config);

        let p2p_config = self.p2p_config;
        let sequencer_config = self.sequencer_config.unwrap_or_default();

        RollupNode {
            config: rollup_config,
            interop_mode: self.interop_mode,
            l1_config: L1NodeConfig {
                chain_config: l1_config,
                trust_rpc: self.l1_config.trust_rpc,
                beacon: l1_beacon,
                provider: l1_provider,
            },
            l2_provider,
            l2_trust_rpc: self.l2_trust_rpc,
            engine_config: self.engine_config,
            rpc_builder: self.rpc_config,
            p2p_config,
            sequencer_config,
        }
    }
}
