//! The Engine Actor

use alloy_rpc_types_engine::JwtSecret;
use kona_engine::SyncConfig;
use url::Url;

/// Configuration for the Engine Actor.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// The sync configuration.
    pub sync: SyncConfig,
    /// The engine rpc url.
    pub engine_url: Url,
    /// The engine jwt secret.
    pub jwt_secret: JwtSecret,
}
