//! Contains the builder for the [`RollupNode`].

use crate::{EngineLauncher, NodeMode, RollupNode};
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

use kona_engine::SyncConfig;
use kona_genesis::RollupConfig;
use kona_p2p::Config;
use kona_providers_alloy::OnlineBeaconClient;
use kona_rpc::RpcConfig;

/// The [`RollupNodeBuilder`] is used to construct a [`RollupNode`] service.
#[derive(Debug, Default)]
pub struct RollupNodeBuilder {
    /// The rollup configuration.
    config: RollupConfig,
    /// The sync configuration.
    sync_config: Option<SyncConfig>,
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
    /// The [`Config`].
    p2p_config: Option<Config>,
    /// An RPC Configuration.
    rpc_config: Option<RpcConfig>,
    /// The mode to run the node in.
    mode: NodeMode,
    /// If p2p networking is entirely disabled.
    network_disabled: bool,
}

impl RollupNodeBuilder {
    /// Creates a new [`RollupNodeBuilder`] with the given [`RollupConfig`].
    pub fn new(config: RollupConfig) -> Self {
        Self { config, ..Self::default() }
    }

    /// Sets the mode on the [`RollupNodeBuilder`].
    pub fn with_mode(self, mode: NodeMode) -> Self {
        Self { mode, ..self }
    }

    /// Appends a [`SyncConfig`] to the builder.
    pub fn with_sync_config(self, sync_config: SyncConfig) -> Self {
        Self { sync_config: Some(sync_config), ..self }
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

    /// Appends the P2P [`Config`] to the builder.
    pub fn with_p2p_config(self, config: Config) -> Self {
        Self { p2p_config: Some(config), ..self }
    }

    /// Sets the [`RpcConfig`] on the [`RollupNodeBuilder`].
    pub fn with_rpc_config(self, rpc_config: RpcConfig) -> Self {
        Self { rpc_config: Some(rpc_config), ..self }
    }

    /// Appends whether p2p networking is entirely disabled to the builder.
    pub fn with_network_disabled(self, network_disabled: bool) -> Self {
        Self { network_disabled, ..self }
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
    /// - The sync config is not set.
    /// - The jwt secret is not set.
    pub fn build(self) -> RollupNode {
        let l1_provider =
            RootProvider::new_http(self.l1_provider_rpc_url.expect("l1 provider rpc url not set"));
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

        let rpc_launcher = self.rpc_config.map(|c| c.as_launcher()).unwrap_or_default();

        let config = Arc::new(self.config);
        let engine_launcher = EngineLauncher {
            config: Arc::clone(&config),
            sync: self.sync_config.expect("missing sync config"),
            l2_rpc_url,
            engine_url: self.l2_engine_rpc_url.expect("missing l2 engine rpc url"),
            jwt_secret,
        };

        RollupNode {
            mode: self.mode,
            config,
            l1_provider,
            l1_beacon,
            l2_provider,
            engine_launcher,
            rpc_launcher,
            p2p_config: self.p2p_config,
            network_disabled: self.network_disabled,
        }
    }
}
