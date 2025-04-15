//! The Engine Actor

use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use kona_engine::{
    ConsolidateTask, Engine, EngineClient, EngineStateBuilder, EngineStateBuilderError, EngineTask,
    InsertUnsafeTask, SyncConfig,
};
use kona_genesis::RollupConfig;
use kona_rpc::OpAttributesWithParent;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
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
    pub sync: Arc<SyncConfig>,
    /// An [`EngineClient`] used for creating engine tasks.
    pub client: Arc<EngineClient>,
    /// The [`Engine`].
    pub engine: Engine,
    /// A channel to receive [`OpAttributesWithParent`] from the derivation actor.
    attributes_rx: UnboundedReceiver<OpAttributesWithParent>,
    /// A channel to receive [`OpNetworkPayloadEnvelope`] from the network actor.
    unsafe_block_rx: UnboundedReceiver<OpNetworkPayloadEnvelope>,
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
        unsafe_block_rx: UnboundedReceiver<OpNetworkPayloadEnvelope>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            config,
            sync: Arc::new(sync),
            client: Arc::new(client),
            engine,
            attributes_rx,
            unsafe_block_rx,
            cancellation,
        }
    }
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
    pub async fn launch(self) -> Result<Engine, EngineStateBuilderError> {
        let state = self.state_builder().build().await?;
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
        EngineStateBuilder::new(self.client(), self.config.genesis)
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
                    warn!(target: "engine", "EngineActor received shutdown signal.");
                    return Ok(());
                }
                res = self.engine.drain() => {
                    if let Err(e) = res {
                        warn!(target: "engine", "Encountered error draining engine api tasks: {:?}", e);
                    }
                }
                attributes = self.attributes_rx.recv() => {
                    let Some(attributes) = attributes else {
                        error!(target: "engine", "Attributes receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    let task = ConsolidateTask::new(
                        Arc::clone(&self.client),
                        Arc::clone(&self.config),
                        attributes,
                        true,
                    );
                    let task = EngineTask::Consolidate(task);
                    self.engine.enqueue(task);
                    debug!(target: "engine", "Enqueued attributes consolidation task.");
                }
                unsafe_block = self.unsafe_block_rx.recv() => {
                    let Some(envelope) = unsafe_block else {
                        error!(target: "engine", "Unsafe block receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    let task = InsertUnsafeTask::new(
                        Arc::clone(&self.client),
                        Arc::clone(&self.sync),
                        Arc::clone(&self.config),
                        envelope,
                    );
                    let task = EngineTask::InsertUnsafe(task);
                    self.engine.enqueue(task);
                    debug!(target: "engine", "Enqueued unsafe block task.");
                }
            }
        }
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// An error from the [`EngineActor`].
#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    /// Closed channel error.
    #[error("closed channel error")]
    ChannelClosed,
}
