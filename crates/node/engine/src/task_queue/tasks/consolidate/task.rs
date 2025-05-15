//! A task to consolidate the engine state.

use crate::{
    BuildTask, ConsolidateTaskError, EngineClient, EngineState, EngineTaskError, EngineTaskExt,
    ForkchoiceTask, Metrics,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OpAttributesWithParent};
use std::sync::Arc;

/// The [`ConsolidateTask`] attempts to consolidate the engine state
/// using the specified payload attributes and the oldest unsafe head.
///
/// If consolidation fails, payload attributes processing is attempted using the [`BuildTask`].
#[derive(Debug, Clone)]
pub struct ConsolidateTask {
    /// The engine client.
    pub client: Arc<EngineClient>,
    /// The [`RollupConfig`].
    pub cfg: Arc<RollupConfig>,
    /// The [`OpAttributesWithParent`] to instruct the execution layer to build.
    pub attributes: OpAttributesWithParent,
    /// Whether or not the payload was derived, or created by the sequencer.
    pub is_attributes_derived: bool,
}

impl ConsolidateTask {
    /// Creates a new [`ConsolidateTask`].
    pub const fn new(
        client: Arc<EngineClient>,
        config: Arc<RollupConfig>,
        attributes: OpAttributesWithParent,
        is_attributes_derived: bool,
    ) -> Self {
        Self { client, cfg: config, attributes, is_attributes_derived }
    }

    /// Executes the [`ForkchoiceTask`] if the attributes match the block.
    async fn execute_forkchoice_task(
        &self,
        state: &mut EngineState,
    ) -> Result<(), EngineTaskError> {
        let task = ForkchoiceTask::new(Arc::clone(&self.client));
        task.execute(state).await
    }

    /// Executes a new [`BuildTask`].
    /// This is used when the [`ConsolidateTask`] fails to consolidate the engine state.
    async fn execute_build_task(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        let build_task = BuildTask::new(
            self.client.clone(),
            self.cfg.clone(),
            self.attributes.clone(),
            self.is_attributes_derived,
        );
        build_task.execute(state).await
    }

    /// Attempts consolidation on the engine state.
    pub async fn consolidate(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        // Fetch the unsafe l2 block after the attributes parent.
        let block_num = self.attributes.parent.block_info.number + 1;
        let block = match self.client.l2_block_by_label(block_num.into()).await {
            Ok(Some(block)) => block,
            Ok(None) => {
                warn!(target: "engine", "Received `None` block for {}", block_num);
                return Err(ConsolidateTaskError::MissingUnsafeL2Block(block_num).into());
            }
            Err(_) => {
                warn!(target: "engine", "Failed to fetch unsafe l2 block for consolidation");
                return Err(ConsolidateTaskError::FailedToFetchUnsafeL2Block.into());
            }
        };

        // Attempt to consolidate the unsafe head.
        // If this is successful, the forkchoice change synchronizes.
        // Otherwise, the attributes need to be processed.
        let block_hash = block.header.hash;
        if crate::AttributesMatch::check(&self.cfg, &self.attributes, &block).is_match() {
            debug!(
                target: "engine",
                attributes = ?self.attributes,
                block_hash = %block.header.hash,
                "Consolidating engine state",
            );
            match L2BlockInfo::from_block_and_genesis(&block.into_consensus(), &self.cfg.genesis) {
                Ok(block_info) => {
                    debug!(target: "engine", ?block_info, "Promoted safe head");
                    state.set_safe_head(block_info);
                    match self.execute_forkchoice_task(state).await {
                        Ok(()) => {
                            debug!(target: "engine", "Consolidation successful");

                            // Update metrics.
                            kona_macros::inc!(
                                counter,
                                Metrics::ENGINE_TASK_COUNT,
                                Metrics::CONSOLIDATE_TASK_LABEL
                            );

                            return Ok(());
                        }
                        Err(e) => {
                            warn!(target: "engine", ?e, "Consolidation failed");
                            return Err(e);
                        }
                    }
                }
                Err(e) => {
                    // Continue on to build the block since we failed to construct the block info.
                    warn!(target: "engine", ?e, "Failed to construct L2BlockInfo, proceeding to build task");
                }
            }
        }

        // Otherwise, the attributes need to be processed.
        debug!(
            target: "engine",
            attributes = ?self.attributes,
            block_hash = %block_hash,
            "No consolidation needed executing build task",
        );
        self.execute_build_task(state).await
    }
}

#[async_trait]
impl EngineTaskExt for ConsolidateTask {
    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        // Skip to processing the payload attributes if consolidation is not needed.
        match state.needs_consolidation() {
            true => self.consolidate(state).await,
            false => self.execute_build_task(state).await,
        }
    }
}
