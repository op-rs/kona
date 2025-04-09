//! The Engine Actor

use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use kona_engine::{
    ConsolidateTask, Engine, EngineClient, EngineStateBuilder, EngineTask, SyncConfig,
};
use kona_genesis::RollupConfig;
use kona_rpc::OpAttributesWithParent;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::NodeActor;

/// The [`EngineActor`] for the engine api sub-routine.
///
/// The engine actor is essentially just a wrapper over two things.
/// - [`kona_engine::EngineState`]
/// - The Engine API
#[derive(Debug)]
pub struct EngineActor {
    /// The [`RollupConfig`] used to build tasks.
    pub config: Arc<RollupConfig>,
    /// The [`SyncConfig`] for engine api tasks.
    pub sync: SyncConfig,
    /// An [`EngineClient`] used for creating engine tasks.
    pub client: Arc<EngineClient>,
    /// The [`Engine`].
    pub engine: Engine,
    /// A channel to receive [`OpAttributesWithParent`] from the derivation actor.
    attributes_rx: UnboundedReceiver<OpAttributesWithParent>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl EngineActor {
    /// Constructs a new [`EngineActor`] from the params.
    pub fn new(
        config: Arc<RollupConfig>,
        sync: SyncConfig,
        client: EngineClient,
        engine: Engine,
        attributes_rx: UnboundedReceiver<OpAttributesWithParent>,
        cancellation: CancellationToken,
    ) -> Self {
        Self { config, sync, client: Arc::new(client), engine, attributes_rx, cancellation }
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
    /// The [`RollupConfig`].
    pub config: Arc<RollupConfig>,
    /// The [`SyncConfig`] for engine tasks.
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
                attributes = self.attributes_rx.recv() => {
                    let Some(attributes) = attributes else {
                        debug!(target: "engine", "Received `None` attributes from receiver");
                        continue;
                    };
                    let task = ConsolidateTask::new(
                        Arc::clone(&self.client),
                        Arc::clone(&self.config),
                        attributes,
                        true,
                    );
                    let task = EngineTask::Consolidate(task);
                    self.engine.enqueue(task);
                }
            }
        }
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        unimplemented!("EngineActor::process is unimplemented")
    }
}

/// An error from the [`EngineActor`].
#[derive(thiserror::Error, Debug)]
pub enum EngineError {}
