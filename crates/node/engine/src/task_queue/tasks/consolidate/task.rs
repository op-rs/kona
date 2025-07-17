//! A task to consolidate the engine state.

use crate::{
    BuildTask, ConsolidateTaskError, EngineClient, EngineState, EngineTaskExt, ForkchoiceTask,
    Metrics, state::EngineSyncStateUpdate,
};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OpAttributesWithParent};
use std::{sync::Arc, time::Instant};

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

    /// Executes a new [`BuildTask`].
    /// This is used when the [`ConsolidateTask`] fails to consolidate the engine state.
    async fn execute_build_task(
        &self,
        state: &mut EngineState,
    ) -> Result<(), ConsolidateTaskError> {
        let build_task = BuildTask::new(
            self.client.clone(),
            self.cfg.clone(),
            self.attributes.clone(),
            self.is_attributes_derived,
            None,
        );
        Ok(build_task.execute(state).await?)
    }

    /// Attempts consolidation on the engine state.
    pub async fn consolidate(&self, state: &mut EngineState) -> Result<(), ConsolidateTaskError> {
        let global_start = Instant::now();

        // Fetch the unsafe l2 block after the attributes parent.
        let block_num = self.attributes.block_number();
        let fetch_start = Instant::now();
        let block = match self.client.l2_block_by_label(block_num.into()).await {
            Ok(Some(block)) => block,
            Ok(None) => {
                warn!(target: "engine", "Received `None` block for {}", block_num);
                return Err(ConsolidateTaskError::MissingUnsafeL2Block(block_num));
            }
            Err(_) => {
                warn!(target: "engine", "Failed to fetch unsafe l2 block for consolidation");
                return Err(ConsolidateTaskError::FailedToFetchUnsafeL2Block);
            }
        };
        let block_fetch_duration = fetch_start.elapsed();

        // Attempt to consolidate the unsafe head.
        // If this is successful, the forkchoice change synchronizes.
        // Otherwise, the attributes need to be processed.
        let block_hash = block.header.hash;
        if crate::AttributesMatch::check(&self.cfg, &self.attributes, &block).is_match() {
            trace!(
                target: "engine",
                attributes = ?self.attributes,
                block_hash = %block_hash,
                "Consolidating engine state",
            );

            match L2BlockInfo::from_block_and_genesis(&block.into_consensus(), &self.cfg.genesis) {
                // Only issue a forkchoice update if the attributes are the last in the span
                // batch. This is an optimization to avoid sending a FCU
                // call for every block in the span batch.
                Ok(block_info) if !self.attributes.is_last_in_span => {
                    let total_duration = global_start.elapsed();

                    info!(
                        target: "engine",
                        hash = %block_info.block_info.hash,
                        number = block_info.block_info.number,
                        ?total_duration,
                        ?block_fetch_duration,
                        "Updated safe head via L1 consolidation"
                    );

                    // Apply a transient update to the safe head.
                    state.sync_state = state.sync_state.apply_update(EngineSyncStateUpdate {
                        safe_head: Some(block_info),
                        local_safe_head: Some(block_info),
                        ..Default::default()
                    });

                    return Ok(());
                }
                Ok(block_info) => {
                    let fcu_start = Instant::now();

                    ForkchoiceTask::new(
                        Arc::clone(&self.client),
                        self.cfg.clone(),
                        EngineSyncStateUpdate {
                            safe_head: Some(block_info),
                            local_safe_head: Some(block_info),
                            ..Default::default()
                        },
                        None,
                    )
                    .execute(state)
                    .await
                    .map_err(|e| {
                        warn!(target: "engine", ?e, "Consolidation failed");
                        e
                    })?;

                    let fcu_duration = fcu_start.elapsed();

                    let total_duration = global_start.elapsed();

                    // Update metrics.
                    kona_macros::inc!(
                        counter,
                        Metrics::ENGINE_TASK_COUNT,
                        Metrics::CONSOLIDATE_TASK_LABEL
                    );

                    info!(
                        target: "engine",
                        hash = %block_info.block_info.hash,
                        number = block_info.block_info.number,
                        ?total_duration,
                        ?block_fetch_duration,
                        fcu_duration = ?fcu_duration,
                        "Updated safe head via L1 consolidation"
                    );

                    return Ok(());
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
            "Attributes mismatch! Executing build task to initiate reorg",
        );
        self.execute_build_task(state).await
    }
}

#[async_trait]
impl EngineTaskExt for ConsolidateTask {
    type Output = ();

    type Error = ConsolidateTaskError;

    async fn execute(&self, state: &mut EngineState) -> Result<(), ConsolidateTaskError> {
        // Skip to building the payload attributes if consolidation is not needed.
        if state.sync_state.safe_head().block_info.number <
            state.sync_state.unsafe_head().block_info.number
        {
            self.consolidate(state).await
        } else {
            self.execute_build_task(state).await
        }
    }
}
