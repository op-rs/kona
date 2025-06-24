//! Runtime Loading Actor

use async_trait::async_trait;
use kona_sources::{RuntimeConfig, RuntimeLoader, RuntimeLoaderError};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

use crate::{NodeActor, actors::ActorContext};

/// The communication context used by the runtime actor.
#[derive(Debug)]
pub struct RuntimeContext {
    runtime_config: mpsc::Sender<RuntimeConfig>,
    cancellation: CancellationToken,
}

impl ActorContext for RuntimeContext {
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
    /// The [`RuntimeLoader`].
    loader: RuntimeLoader,
    /// The interval at which to load the runtime.
    interval: Duration,
}

impl RuntimeActor {
    /// Constructs a new [`RuntimeActor`] from the given [`RuntimeLoader`].
    pub const fn new(
        loader: RuntimeLoader,
        interval: Duration,
        runtime_config: mpsc::Sender<RuntimeConfig>,
        cancellation: CancellationToken,
    ) -> (Self, RuntimeContext) {
        let actor = Self { loader, interval };
        let context = RuntimeContext { runtime_config, cancellation };
        (actor, context)
    }
}

/// The Runtime Launcher is a simple launcher for the [`RuntimeActor`].
#[derive(Debug, Clone)]
pub struct RuntimeLauncher {
    /// The [`RuntimeLoader`].
    loader: RuntimeLoader,
    /// The interval at which to load the runtime.
    interval: Option<Duration>,
    /// The channel to send the [`RuntimeConfig`] to the engine actor.
    tx: Option<mpsc::Sender<RuntimeConfig>>,
    /// The cancellation token.
    cancellation: Option<CancellationToken>,
}

impl RuntimeLauncher {
    /// Constructs a new [`RuntimeLoader`] from the given runtime loading interval.
    pub const fn new(loader: RuntimeLoader, interval: Option<Duration>) -> Self {
        Self { loader, interval, tx: None, cancellation: None }
    }

    /// Sets the runtime config tx channel.
    pub fn with_tx(self, tx: mpsc::Sender<RuntimeConfig>) -> Self {
        Self { tx: Some(tx), ..self }
    }

    /// Sets the [`CancellationToken`] on the [`RuntimeLauncher`].
    pub fn with_cancellation(self, cancellation: CancellationToken) -> Self {
        Self { cancellation: Some(cancellation), ..self }
    }

    /// Launches the [`RuntimeActor`].
    pub fn launch(self) -> Option<(RuntimeActor, RuntimeContext)> {
        let cancellation = self.cancellation?;
        let tx = self.tx?;
        if self.interval.is_some() {
            info!(target: "runtime", interval = ?self.interval, "Launched Runtime Actor");
        }
        self.interval.map(|i| RuntimeActor::new(self.loader, i, tx, cancellation))
    }
}

#[async_trait]
impl NodeActor for RuntimeActor {
    type Error = RuntimeLoaderError;
    type Context = RuntimeContext;

    async fn start(
        mut self,
        RuntimeContext { runtime_config, cancellation }: Self::Context,
    ) -> Result<(), Self::Error> {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    warn!(target: "runtime", "RuntimeActor received shutdown signal.");
                    return Ok(());
                }
                _ = interval.tick() => {
                    let config = self.loader.load_latest().await?;
                    debug!(target: "runtime", ?config, "Loaded latest runtime config");
                    if let Err(e) = runtime_config.send(config).await {
                        error!(target: "runtime", ?e, "Failed to send runtime config to the engine actor");
                    }
                }
            }
        }
    }
}
