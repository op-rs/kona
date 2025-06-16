//! Runtime Loading Actor

use async_trait::async_trait;
use kona_sources::{RuntimeConfig, RuntimeLoader, RuntimeLoaderError};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::NodeActor;

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
    /// A channel to send the loaded runtime config to the engine actor.
    runtime_config_tx: mpsc::Sender<RuntimeConfig>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl RuntimeActor {
    /// Constructs a new [`RuntimeActor`] from the given [`RuntimeLoader`].
    pub const fn new(
        loader: RuntimeLoader,
        interval: Duration,
        runtime_config_tx: mpsc::Sender<RuntimeConfig>,
        cancellation: CancellationToken,
    ) -> Self {
        Self { loader, interval, runtime_config_tx, cancellation }
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
    pub fn launch(self) -> Option<RuntimeActor> {
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
    type InboundEvent = ();
    type Error = RuntimeLoaderError;

    async fn start(mut self) -> Result<(), Self::Error> {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    warn!(target: "runtime", "RuntimeActor received shutdown signal.");
                    return Ok(());
                }
                _ = interval.tick() => {
                    let config = self.loader.load_latest().await?;
                    debug!(target: "runtime", ?config, "Loaded latest runtime config");
                    if let Err(e) = self.runtime_config_tx.send(config).await {
                        error!(target: "runtime", ?e, "Failed to send runtime config to the engine actor");
                    }
                }
            }
        }
    }

    async fn process(&mut self, e: Self::InboundEvent) -> Result<(), Self::Error> {
        trace!(target: "runtime", ?e, "Runtime Actor received unexpected inbound event. Ignoring.");
        Ok(())
    }
}
