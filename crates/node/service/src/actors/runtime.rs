//! Runtime Loading Actor

use async_trait::async_trait;
use kona_engine::EngineClient;
use kona_sources::{RuntimeConfig, RuntimeLoader, RuntimeLoaderError};
use op_alloy_provider::ext::engine::OpEngineApi;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

use crate::{NodeActor, actors::CancellableContext};

/// The communication context used by the runtime actor.
#[derive(Debug)]
pub struct RuntimeContext {
    /// Cancels the runtime actor.
    pub cancellation: CancellationToken,
}

impl CancellableContext for RuntimeContext {
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}

/// The Runtime Actor.
///
/// The runtime actor is responsible for loading the runtime config
/// using the [`RuntimeLoader`].
#[derive(Debug)]
pub struct RuntimeActor {
    state: RuntimeState,
}

/// The state of the runtime actor.
#[derive(Debug, Clone)]
pub struct RuntimeState {
    /// The [`RuntimeLoader`].
    pub loader: RuntimeLoader,
    /// The interval at which to load the runtime.
    pub interval: Duration,
    /// The client to send the updated runtime config to the EL engine.
    pub client: Arc<EngineClient>,
}

impl RuntimeActor {
    /// Constructs a new [`RuntimeActor`] from the given [`RuntimeLoader`].
    pub const fn new(state: RuntimeState) -> ((), Self) {
        ((), Self { state })
    }

    fn runtime_config_update(&mut self, config: RuntimeConfig) {
        let client = self.state.client.clone();
        tokio::task::spawn(async move {
            debug!(target: "engine", config = ?config, "Received runtime config");
            let recommended = config.recommended_protocol_version;
            let required = config.required_protocol_version;
            match client.signal_superchain_v1(recommended, required).await {
                Ok(v) => info!(target: "engine", ?v, "[SUPERCHAIN::SIGNAL]"),
                Err(e) => {
                    // Since the `engine_signalSuperchainV1` endpoint is OPTIONAL,
                    // a warning is logged instead of an error.
                    warn!(target: "engine", ?e, "Failed to send superchain signal (OPTIONAL)");
                }
            }
        });
    }
}

#[async_trait]
impl NodeActor for RuntimeActor {
    type Error = RuntimeLoaderError;
    type OutboundData = RuntimeContext;
    type InboundData = ();
    type Builder = RuntimeState;

    fn build(state: Self::Builder) -> (Self::InboundData, Self) {
        Self::new(state)
    }

    async fn start(
        mut self,
        RuntimeContext { cancellation }: Self::OutboundData,
    ) -> Result<(), Self::Error> {
        let mut interval = tokio::time::interval(self.state.interval);

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    warn!(target: "runtime", "RuntimeActor received shutdown signal.");
                    return Ok(());
                }
                _ = interval.tick() => {
                    let config = self.state.loader.load_latest().await?;
                    debug!(target: "runtime", ?config, "Loaded latest runtime config");
                    self.runtime_config_update(config);
                }
            }
        }
    }
}
