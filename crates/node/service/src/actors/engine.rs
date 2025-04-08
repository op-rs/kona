//! The Engine Actor

use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use kona_engine::{Engine, EngineClient, EngineStateBuilder, SyncConfig};
use kona_genesis::RollupConfig;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
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
    /// The [`Engine`].
    pub engine: Engine,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl EngineActor {
    /// Constructs a new [`EngineActor`] from the params.
    pub const fn new(sync: SyncConfig, engine: Engine, cancellation: CancellationToken) -> Self {
        Self { sync, engine, cancellation }
    }
}

/// An error thrown by the [`EngineLauncher`] at initialization.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum EngineLaunchError {
    /// An error occured when building the engine state.
    #[error("an error occured building the engine state")]
    EngineStateBuildFailed,
}

/// Configuration for the Engine Actor.
#[derive(Debug, Clone)]
pub struct EngineLauncher {
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

impl EngineLauncher {
    /// Launches the [`Engine`].
    pub async fn launch(self) -> Result<Engine, EngineLaunchError> {
        let state = self
            .state_builder()
            .build()
            .await
            .map_err(|_| EngineLaunchError::EngineStateBuildFailed)?;
        Ok(Engine::new(state))
    }

    /// Returns the [`EngineClient`].
    pub fn client(&self) -> EngineClient {
        EngineClient::new_http(
            self.engine_url.clone(),
            self.l2_rpc_url.clone(),
            self.config.clone(),
            self.jwt_secret,
        )
    }

    /// Returns an [`EngineStateBuilder`].
    pub fn state_builder(&self) -> EngineStateBuilder {
        EngineStateBuilder::new(self.client())
    }
}

#[async_trait]
impl NodeActor for EngineActor {
    type InboundEvent = ();
    type Error = EngineError;

    async fn start(mut self) -> Result<(), Self::Error> {
        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    info!(
                        target: "engine",
                        "Received shutdown signal. Exiting engine task."
                    );
                    return Ok(());
                }
                res = self.engine.drain() => {
                    if let Err(e) = res {
                        warn!(target: "engine", "Encountered error draining engine api tasks: {:?}", e);
                    }
                }
            }
        }
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        unimplemented!("NodeActor::process is unimplemented")
    }
}

/// An error from the [`EngineActor`].
#[derive(thiserror::Error, Debug)]
pub enum EngineError {}
