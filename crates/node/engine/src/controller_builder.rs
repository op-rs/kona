//! An [EngineController] builder.

use anyhow::{Result, bail};
use kona_genesis::RollupConfig;

use crate::{EngineClient, EngineController, EngineState, SyncStatus};

/// A builder for the [EngineController].
#[derive(Debug, Clone)]
pub struct ControllerBuilder {
    /// The engine client.
    client: EngineClient,
    /// The engine state.
    state: Option<EngineState>,
    /// The rollup config.
    config: Option<RollupConfig>,
    /// The sync status.
    sync: Option<SyncStatus>,
}

impl ControllerBuilder {
    /// Instantiates a new [ControllerBuilder] from the provided [EngineClient].
    pub const fn new(client: EngineClient) -> Self {
        Self { client, state: None, config: None, sync: None }
    }

    /// Sets the [RollupConfig] for the [EngineController].
    pub const fn with_config(mut self, config: RollupConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Sets the [SyncStatus] for the [EngineController].
    pub const fn with_sync(mut self, sync: SyncStatus) -> Self {
        self.sync = Some(sync);
        self
    }

    /// Builds the [EngineController].
    pub async fn build(self) -> Result<EngineController> {
        let state = if let Some(s) = self.state {
            s
        } else {
            EngineState::builder(self.client.clone()).build().await?
        };

        let config = if let Some(c) = self.config {
            c
        } else {
            bail!("The rollup config is required to build the EngineController");
        };

        let sync_status = if let Some(s) = self.sync {
            s
        } else {
            bail!("Sync status is required to build the EngineController");
        };

        Ok(EngineController {
            client: self.client,
            sync_status,
            state,
            blocktime: config.block_time,
            ecotone_timestamp: config.hardforks.ecotone_time,
            canyon_timestamp: config.hardforks.canyon_time,
        })
    }
}
