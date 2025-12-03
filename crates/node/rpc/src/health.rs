use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use rollup_boost::Health;
use tokio::sync::oneshot;

use crate::jsonrpsee::HealthzApiServer;

/// The rollup boost health status.
///
/// This type is the same as [`Health`], but it implements `serde::Serialize`
/// and `serde::Deserialize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum RollupBoostHealth {
    /// Rollup boost is healthy.
    Healthy,
    /// Rollup boost is partially healthy.
    PartialContent,
    /// Rollup boost service is unavailable.
    ServiceUnavailable,
}

impl From<Health> for RollupBoostHealth {
    fn from(health: Health) -> Self {
        match health {
            Health::Healthy => Self::Healthy,
            Health::PartialContent => Self::PartialContent,
            Health::ServiceUnavailable => Self::ServiceUnavailable,
        }
    }
}

/// A healthcheck response for the RPC server.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HealthzResponse {
    /// The application version.
    pub version: String,
}

/// A query to get the health of the rollup boost server.
#[derive(Debug)]
pub struct RollupBoostHealthQuery {
    /// The sender to send the rollup boost health to.
    pub sender: oneshot::Sender<RollupBoostHealth>,
}

/// The healthz rpc server.
#[derive(Debug, Default)]
pub struct HealthzRpc;

impl HealthzRpc {
    /// Constructs a new [`HealthzRpc`].
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl HealthzApiServer for HealthzRpc {
    async fn healthz(&self) -> RpcResult<HealthzResponse> {
        Ok(HealthzResponse { version: env!("CARGO_PKG_VERSION").to_string() })
    }
}
