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
    runtime_config: mpsc::Sender<RuntimeConfig>,
}

/// The state of the runtime actor.
#[derive(Debug, Clone)]
pub struct RuntimeState {
    /// The [`RuntimeLoader`].
    pub loader: RuntimeLoader,
    /// The interval at which to load the runtime.
    pub interval: Duration,
}

/// The outbound data for the runtime actor.
#[derive(Debug)]
pub struct RuntimeOutboundData {
    /// The channel to send the [`RuntimeConfig`] to the engine actor.
    pub runtime_config: mpsc::Receiver<RuntimeConfig>,
}

impl RuntimeActor {
    /// Constructs a new [`RuntimeActor`] from the given [`RuntimeLoader`].
    pub fn new(state: RuntimeState) -> (RuntimeOutboundData, Self) {
        let (runtime_config_tx, runtime_config_rx) = mpsc::channel(1024);
        let outbound_data = RuntimeOutboundData { runtime_config: runtime_config_rx };
        let actor = Self { state, runtime_config: runtime_config_tx };
        (outbound_data, actor)
    }
}

#[async_trait]
impl NodeActor for RuntimeActor {
    type Error = RuntimeLoaderError;
    type InboundData = RuntimeContext;
    type OutboundData = RuntimeOutboundData;
    type State = RuntimeState;

    fn build(state: Self::State) -> (Self::OutboundData, Self) {
        Self::new(state)
    }

    async fn start(
        mut self,
        RuntimeContext { cancellation }: Self::InboundData,
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
                    if let Err(e) = self.runtime_config.send(config).await {
                        error!(target: "runtime", ?e, "Failed to send runtime config to the engine actor");
                    }
                }
            }
        }
    }
}
