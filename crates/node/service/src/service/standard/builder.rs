//! Contains the builder for the [`RollupNode`].

use crate::{
    EngineBuilder, InteropMode, NetworkConfig, NodeMode, RollupNode, SequencerConfig,
    actors::RuntimeState,
};
use alloy_primitives::Bytes;
use alloy_provider::RootProvider;
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_engine::JwtSecret;
use alloy_transport_http::{
    AuthLayer, Http, HyperClient,
    hyper_util::{client::legacy::Client, rt::TokioExecutor},
};
use http_body_util::Full;
use op_alloy_network::Optimism;
use std::sync::Arc;
use tower::ServiceBuilder;
use url::Url;

use kona_genesis::RollupConfig;
use kona_providers_alloy::OnlineBeaconClient;
use kona_rpc::{RpcBuilder, SupervisorRpcConfig};

/// The [`RollupNodeBuilder`] is used to construct a [`RollupNode`] service.
#[derive(Debug, Default)]
pub struct RollupNodeBuilder {
    /// The rollup configuration.
    config: RollupConfig,
    /// The L1 EL provider RPC URL.
    l1_provider_rpc_url: Option<Url>,
    /// The L1 beacon API URL.
    l1_beacon_api_url: Option<Url>,
    /// The L2 engine RPC URL.
    l2_engine_rpc_url: Option<Url>,
    /// The L2 EL provider RPC URL.
    l2_provider_rpc_url: Option<Url>,
    /// The JWT secret.
    jwt_secret: Option<JwtSecret>,
    /// The [`NetworkConfig`].
    p2p_config: Option<NetworkConfig>,
    /// An RPC Configuration.
    rpc_config: Option<RpcBuilder>,
    /// An RPC Configuration for the supervisor rpc.
    supervisor_rpc_config: SupervisorRpcConfig,
    /// An interval to load the runtime config.
    runtime_load_interval: Option<std::time::Duration>,
    /// The [`SequencerConfig`].
    sequencer_config: Option<SequencerConfig>,
    /// The mode to run the node in.
    mode: NodeMode,
    /// Whether to run the node in interop mode.
    interop_mode: InteropMode,
}

impl RollupNodeBuilder {
    /// Creates a new [`RollupNodeBuilder`] with the given [`RollupConfig`].
    pub fn new(config: RollupConfig) -> Self {
        Self { config, ..Self::default() }
    }

    /// Sets the interop mode on the [`RollupNodeBuilder`].
    pub fn with_interop_mode(self, interop_mode: InteropMode) -> Self {
        Self { interop_mode, ..self }
    }

    /// Sets the [`NodeMode`] on the [`RollupNodeBuilder`].
    pub fn with_mode(self, mode: NodeMode) -> Self {
        Self { mode, ..self }
    }

    /// Appends the [`SupervisorRpcConfig`] to the builder.
    pub fn with_supervisor_rpc_config(self, config: SupervisorRpcConfig) -> Self {
        Self { supervisor_rpc_config: config, ..self }
    }

    /// Appends an L1 EL provider RPC URL to the builder.
    pub fn with_l1_provider_rpc_url(self, l1_provider_rpc_url: Url) -> Self {
        Self { l1_provider_rpc_url: Some(l1_provider_rpc_url), ..self }
    }

    /// Appends an L1 beacon API URL to the builder.
    pub fn with_l1_beacon_api_url(self, l1_beacon_api_url: Url) -> Self {
        Self { l1_beacon_api_url: Some(l1_beacon_api_url), ..self }
    }

    /// Appends an L2 engine RPC URL to the builder.
    pub fn with_l2_engine_rpc_url(self, l2_engine_rpc_url: Url) -> Self {
        Self { l2_engine_rpc_url: Some(l2_engine_rpc_url), ..self }
    }

    /// Appends an L2 EL provider RPC URL to the builder.
    pub fn with_l2_provider_rpc_url(self, l2_provider_rpc_url: Url) -> Self {
        Self { l2_provider_rpc_url: Some(l2_provider_rpc_url), ..self }
    }

    /// Appends a JWT secret to the builder.
    pub fn with_jwt_secret(self, jwt_secret: JwtSecret) -> Self {
        Self { jwt_secret: Some(jwt_secret), ..self }
    }

    /// Appends the P2P [`NetworkConfig`] to the builder.
    pub fn with_p2p_config(self, config: NetworkConfig) -> Self {
        Self { p2p_config: Some(config), ..self }
    }

    /// Sets the [`RpcBuilder`] on the [`RollupNodeBuilder`].
    pub fn with_rpc_config(self, rpc_config: Option<RpcBuilder>) -> Self {
        Self { rpc_config, ..self }
    }

    /// Sets the runtime load interval on the [`RollupNodeBuilder`].
    pub fn with_runtime_load_interval(self, interval: std::time::Duration) -> Self {
        Self { runtime_load_interval: Some(interval), ..self }
    }

    /// Appends the [`SequencerConfig`] to the builder.
    pub fn with_sequencer_config(self, sequencer_config: SequencerConfig) -> Self {
        Self { sequencer_config: Some(sequencer_config), ..self }
    }

    /// Assembles the [`RollupNode`] service.
    ///
    /// By default, the supervisor RPC is disabled.
    /// To enable it, use the [`Self::with_supervisor_rpc_config`] method.
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
    pub fn build(self) -> RollupNode {
        let l1_rpc_url = self.l1_provider_rpc_url.expect("l1 provider rpc url not set");
        let l1_provider = RootProvider::new_http(l1_rpc_url.clone());
        let l1_beacon = OnlineBeaconClient::new_http(
            self.l1_beacon_api_url.expect("l1 beacon api url not set").to_string(),
        );

        let l2_rpc_url = self.l2_provider_rpc_url.expect("l2 provider rpc url not set");
        let jwt_secret = self.jwt_secret.expect("jwt secret not set");
        let hyper_client = Client::builder(TokioExecutor::new()).build_http::<Full<Bytes>>();

        let auth_layer = AuthLayer::new(jwt_secret);
        let service = ServiceBuilder::new().layer(auth_layer).service(hyper_client);

        let layer_transport = HyperClient::with_service(service);
        let http_hyper = Http::with_client(layer_transport, l2_rpc_url.clone());
        let rpc_client = RpcClient::new(http_hyper, false);
        let l2_provider = RootProvider::<Optimism>::new(rpc_client);

        let rollup_config = Arc::new(self.config);
        let engine_builder = EngineBuilder {
            config: Arc::clone(&rollup_config),
            l2_rpc_url,
            l1_rpc_url: l1_rpc_url.clone(),
            engine_url: self.l2_engine_rpc_url.expect("missing l2 engine rpc url"),
            jwt_secret,
            mode: self.mode,
        };

        let runtime_builder = self.runtime_load_interval.map(|load_interval| RuntimeState {
            loader: kona_sources::RuntimeLoader::new(l1_rpc_url, rollup_config.clone()),
            interval: load_interval,
        });

        let p2p_config = self.p2p_config.expect("P2P config not set");
        let sequencer_config = self.sequencer_config.unwrap_or_default();

        let interop_mode = match self.supervisor_rpc_config.is_disabled() {
            true => self.interop_mode,
            false => InteropMode::Indexed,
        };

        RollupNode {
            config: rollup_config,
            interop_mode,
            l1_provider,
            l1_beacon,
            l2_provider,
            engine_builder,
            rpc_builder: self.rpc_config,
            runtime_builder,
            p2p_config,
            sequencer_config,
            // By default, the supervisor rpc config is disabled.
            supervisor_rpc: self.supervisor_rpc_config,
        }
    }
}
