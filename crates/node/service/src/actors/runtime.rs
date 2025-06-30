//! Runtime Loading Actor

use async_trait::async_trait;
use kona_sources::{RuntimeConfig, RuntimeLoader, RuntimeLoaderError};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

use crate::{NodeActor, actors::CancellableContext};

/// The communication context used by the runtime actor.
#[derive(Debug)]
pub struct RuntimeContext {
    /// Cancels the runtime actor.
    pub cancellation: CancellationToken,
    /// The channel to send the [`RuntimeConfig`] to the runtime actor.
    pub runtime_config_tx: mpsc::Sender<RuntimeConfig>,
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
}

impl RuntimeActor {
    /// Constructs a new [`RuntimeActor`] from the given [`RuntimeLoader`].
    pub const fn new(state: RuntimeState) -> ((), Self) {
        ((), Self { state })
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
        RuntimeContext { cancellation, runtime_config_tx }: Self::OutboundData,
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
                    if let Err(e) = runtime_config_tx.send(config).await {
                        error!(target: "runtime", ?e, "Failed to send runtime config to the engine actor");
                    }
                }
            }
        }
    }
}
