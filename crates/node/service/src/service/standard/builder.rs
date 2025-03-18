//! The [RollupNodeBuilder].

use super::RollupNode;
use crate::NodeMode;
use alloy_provider::RootProvider;
use alloy_rpc_types_engine::JwtSecret;
use kona_engine::SyncConfig;
use kona_genesis::RollupConfig;
use kona_providers_alloy::OnlineBeaconClient;
use libp2p::identity::Keypair;
use std::{net::SocketAddr, sync::Arc};
use url::Url;

/// The [RollupNodeBuilder] is used to construct a [RollupNode] service.
#[derive(Debug, Default)]
pub struct RollupNodeBuilder {
    /// The rollup configuration.
    config: RollupConfig,
    /// The sync configuration.
    _sync_config: Option<SyncConfig>,
    /// The L1 EL provider RPC URL.
    l1_provider_rpc_url: Option<Url>,
    /// The L1 beacon API URL.
    l1_beacon_api_url: Option<Url>,
    /// The L2 engine RPC URL.
    _l2_engine_rpc_url: Option<Url>,
    /// The L2 EL provider RPC URL.
    l2_provider_rpc_url: Option<Url>,
    /// The JWT secret.
    _jwt_secret: Option<JwtSecret>,
    /// The gossip address.
    gossip_address: Option<SocketAddr>,
    /// The discovery address.
    discovery_address: Option<SocketAddr>,
    /// If p2p networking is entirely disabled.
    network_disabled: bool,
    /// The keypair.
    keypair: Option<Keypair>,
}

impl RollupNodeBuilder {
    /// Creates a new [RollupNodeBuilder] with the given [RollupConfig].
    pub fn new(config: RollupConfig) -> Self {
        Self { config, ..Self::default() }
    }

    /// Appends a [SyncConfig] to the builder.
    pub fn with_sync_config(self, sync_config: SyncConfig) -> Self {
        Self { _sync_config: Some(sync_config), ..self }
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
        Self { _l2_engine_rpc_url: Some(l2_engine_rpc_url), ..self }
    }

    /// Appends an L2 EL provider RPC URL to the builder.
    pub fn with_l2_provider_rpc_url(self, l2_provider_rpc_url: Url) -> Self {
        Self { l2_provider_rpc_url: Some(l2_provider_rpc_url), ..self }
    }

    /// Appends a JWT secret to the builder.
    pub fn with_jwt_secret(self, jwt_secret: JwtSecret) -> Self {
        Self { _jwt_secret: Some(jwt_secret), ..self }
    }

    /// Appends the gossip address to the builder.
    pub fn with_gossip_address(self, gossip_addr: SocketAddr) -> Self {
        Self { gossip_address: Some(gossip_addr), ..self }
    }

    /// Appends the discovery address to the builder.
    pub fn with_discovery_address(self, discovery_addr: SocketAddr) -> Self {
        Self { discovery_address: Some(discovery_addr), ..self }
    }

    /// Appends whether p2p networking is entirely disabled to the builder.
    pub fn with_network_disabled(self, network_disabled: bool) -> Self {
        Self { network_disabled, ..self }
    }

    /// Appends the keypair to the builder.
    pub fn with_keypair(self, keypair: Keypair) -> Self {
        Self { keypair: Some(keypair), ..self }
    }

    /// Assembles the [RollupNode] service.
    ///
    /// ## Panics
    ///
    /// Panics if:
    /// - The L1 provider RPC URL is not set.
    /// - The L1 beacon API URL is not set.
    /// - The L2 provider RPC URL is not set.
    pub fn build(self) -> RollupNode {
        let l1_provider =
            RootProvider::new_http(self.l1_provider_rpc_url.expect("l1 provider rpc url not set"));
        let l1_beacon = OnlineBeaconClient::new_http(
            self.l1_beacon_api_url.expect("l1 beacon api url not set").to_string(),
        );
        let l2_provider =
            RootProvider::new_http(self.l2_provider_rpc_url.expect("l2 provider rpc url not set"));

        RollupNode {
            mode: NodeMode::Validator, // TODO: Make dynamic
            config: Arc::new(self.config),
            l1_provider,
            l1_beacon,
            l2_provider,
            _l2_engine: (),
            network_disabled: self.network_disabled,
            keypair: self.keypair,
            discovery_address: self.discovery_address,
            gossip_address: self.gossip_address,
        }
    }
}
