//! The orchestrator for kona's consensus components.

use crate::engine::EngineClient;
use alloy_rpc_types_engine::{ForkchoiceState, JwtSecret};
use kona_genesis::RollupConfig;
use std::sync::Arc;
use url::Url;

/// The node orchestrator.
#[derive(Debug, Clone)]
pub struct Pilot {
    /// The engine client.
    engine: EngineClient,
}

impl Pilot {
    /// Constructs a new [`Pilot`].
    pub fn new(
        l2_engine_rpc: Url,
        l2_provider_rpc: Url,
        cfg: Arc<RollupConfig>,
        secret: JwtSecret,
    ) -> Self {
        let engine = EngineClient::new_http(l2_engine_rpc, l2_provider_rpc, cfg, secret);
        Self { engine }
    }

    /// Starts the sync service.
    pub async fn sync(&self) -> anyhow::Result<()> {
        self.engine.try_forkchoice_update(ForkchoiceState::default(), None).await?;
        println!("Updated fork choice state");
        // TODO: sync
        Ok(())
    }
}
