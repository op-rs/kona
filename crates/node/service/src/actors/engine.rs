//! The Engine Actor

use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use kona_engine::{EngineClient, EngineStateBuilder, SyncConfig};
use kona_genesis::RollupConfig;
use std::sync::Arc;
use url::Url;

use crate::NodeActor;

/// The [`EngineActor`] for the engine api sub-routine.
///
/// The engine actor is essentially just a wrapper over two things.
/// - [`kona_engine::EngineState`]
/// - The Engine API
///
/// The rollup node ne
#[derive(Debug)]
pub struct EngineActor {
    /// The [`SyncConfig`] for engine api tasks.
    pub sync: SyncConfig,
    /// The [`EngineStateBuilder`].
    pub builder: EngineStateBuilder,
}

impl From<EngineConfig> for EngineActor {
    fn from(cfg: EngineConfig) -> Self {
        Self { builder: cfg.state_builder(), sync: cfg.sync }
    }
}

/// Configuration for the Engine Actor.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// The rollup config.
    pub config: Arc<RollupConfig>,
    /// The [`SyncConfig`] for engine api tasks.
    pub sync: SyncConfig,
    /// The engine rpc url.
    pub engine_url: Url,
    /// The l2 rpc url.
    pub l2_rpc_url: Url,
    /// The engine jwt secret.
    pub jwt_secret: JwtSecret,
}

impl EngineConfig {
    /// Returns the [`EngineClient`] for the [`EngineConfig`].
    pub fn client(&self) -> EngineClient {
        EngineClient::new_http(
            self.engine_url.clone(),
            self.l2_rpc_url.clone(),
            self.config.clone(),
            self.jwt_secret,
        )
    }

    /// Returns an [`EngineStateBuilder`] for the [`EngineConfig`].
    pub fn state_builder(&self) -> EngineStateBuilder {
        EngineStateBuilder::new(self.client())
    }
}

#[async_trait]
impl NodeActor for EngineActor {
    type InboundEvent = ();
    type Error = ();

    async fn start(mut self) -> Result<(), Self::Error> {
        unimplemented!("NodeActor::start is unimplemented")
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        unimplemented!("NodeActor::process is unimplemented")
    }
}
