//! The [RollupNodeBuilder].

#![allow(unused)]

use super::RollupNode;
use alloy_rpc_types_engine::JwtSecret;
use kona_engine::SyncConfig;
use url::Url;

/// The [RollupNodeBuilder] is used to construct a [RollupNode] service.
#[derive(Debug, Default)]
pub struct RollupNodeBuilder {
    /// The sync configuration.
    sync_config: Option<SyncConfig>,
    /// The L1 EL provider RPC URL.
    l1_provider_rpc_url: Option<Url>,
    /// The L2 engine RPC URL.
    l2_engine_rpc_url: Option<Url>,
    /// The L2 EL provider RPC URL.
    l2_provider_rpc_url: Option<Url>,
    /// The JWT secret.
    jwt_secret: Option<JwtSecret>,
}

impl RollupNodeBuilder {
    /// Appends a [SyncConfig] to the builder.
    pub fn with_sync_config(self, sync_config: SyncConfig) -> Self {
        Self { sync_config: Some(sync_config), ..self }
    }

    /// Appends an L1 EL provider RPC URL to the builder.
    pub fn with_l1_provider_rpc_url(self, l1_provider_rpc_url: Url) -> Self {
        Self { l1_provider_rpc_url: Some(l1_provider_rpc_url), ..self }
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

    /// Assembles the [RollupNode] service.
    pub fn build(self) -> RollupNode {
        todo!()
    }
}
