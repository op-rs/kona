//! A task for finalizing an L2 block.

use crate::{
    EngineClient, EngineState, EngineTaskError, EngineTaskExt, FinalizeTaskError, ForkchoiceTask,
    Metrics,
};
use alloy_provider::Provider;
use async_trait::async_trait;
use kona_protocol::L2BlockInfo;
use std::{sync::Arc, time::Instant};

/// The [`FinalizeTask`] fetches the [`L2BlockInfo`] at `block_number`, updates the [`EngineState`],
/// and dispatches a forkchoice update to finalize the block.
#[derive(Debug, Clone)]
pub struct FinalizeTask {
    /// The engine client.
    pub client: Arc<EngineClient>,
    /// The number of the L2 block to finalize.
    pub block_number: u64,
}

impl FinalizeTask {
    /// Creates a new [`ForkchoiceTask`].
    pub const fn new(client: Arc<EngineClient>, block_number: u64) -> Self {
        Self { client, block_number }
    }
}

#[async_trait]
impl EngineTaskExt for FinalizeTask {
    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        // Sanity check that the block that is being finalized is at least safe.
        if state.safe_head().block_info.number < self.block_number {
            return Err(FinalizeTaskError::BlockNotSafe.into());
        }

        let block_fetch_start = Instant::now();
        let block = self
            .client
            .l2_provider()
            .get_block(self.block_number.into())
            .full()
            .await
            .map_err(FinalizeTaskError::TransportError)?
            .ok_or(FinalizeTaskError::BlockNotFound(self.block_number))?
            .into_consensus();
        let block_info = L2BlockInfo::from_block_and_genesis(&block, &self.client.cfg().genesis)
            .map_err(FinalizeTaskError::FromBlock)?;
        let block_fetch_duration = block_fetch_start.elapsed();

        // Update the finalized block in the engine state.
        state.set_finalized_head(block_info);

        // Dispatch a forkchoice update.
        let fcu_start = Instant::now();
        ForkchoiceTask::new(self.client.clone()).execute(state).await?;
        let fcu_duration = fcu_start.elapsed();

        // Update metrics.
        kona_macros::inc!(counter, Metrics::ENGINE_TASK_COUNT, Metrics::FINALIZE_TASK_LABEL);

        info!(
            target: "engine",
            hash = %block_info.block_info.hash,
            number = block_info.block_info.number,
            ?block_fetch_duration,
            ?fcu_duration,
            "Updated finalized head"
        );

        Ok(())
    }
}
