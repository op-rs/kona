//! A task for synchronizing engine state from an external follow source.

use crate::{
    EngineClient, EngineState, EngineTaskExt, FollowTaskError, SynchronizeTask,
    state::EngineSyncStateUpdate,
};
use async_trait::async_trait;
use derive_more::Constructor;
use kona_genesis::RollupConfig;
use std::{sync::Arc, time::Instant};

/// The [`FollowTask`] synchronizes the engine state with externally provided safe and finalized
/// heads from a follow source (e.g., an external L2 Consensus Layer node).
///
/// This task is used in "follow mode" where the node trusts an external source for safe/finalized
/// heads instead of deriving them from L1 data. It updates the engine state by calling
/// [`SynchronizeTask`] internally.
///
/// ## Why this task exists
///
/// Unlike [`ConsolidateTask`] and [`FinalizeTask`], which operate on locally derived or finalized
/// data, [`FollowTask`] handles external state updates. By implementing the [`EngineTaskExt`]
/// trait, it can receive `&mut EngineState` as a parameter, allowing it to inline the
/// `SynchronizeTask::new().execute(state)` call just like other tasks.
///
/// This is the idiomatic pattern for tasks that need to update engine state: they are enqueued
/// as [`EngineTask`] variants and processed by the engine's task queue, rather than being called
/// directly from the actor's select! loop.
///
/// [`ConsolidateTask`]: crate::ConsolidateTask
/// [`FinalizeTask`]: crate::FinalizeTask
/// [`EngineTask`]: crate::EngineTask
#[derive(Debug, Clone, Constructor)]
pub struct FollowTask<EngineClient_: EngineClient> {
    /// The engine client.
    pub client: Arc<EngineClient_>,
    /// The rollup config.
    pub cfg: Arc<RollupConfig>,
    /// The sync state update containing external safe and finalized heads.
    pub update: EngineSyncStateUpdate,
}

#[async_trait]
impl<EngineClient_: EngineClient> EngineTaskExt for FollowTask<EngineClient_> {
    type Output = ();

    type Error = FollowTaskError;

    async fn execute(&self, state: &mut EngineState) -> Result<(), FollowTaskError> {
        let start = Instant::now();

        // Synchronize the engine state with the external update
        SynchronizeTask::new(self.client.clone(), self.cfg.clone(), self.update)
            .execute(state)
            .await?;

        let duration = start.elapsed();

        info!(
            target: "engine",
            unsafe_head = ?self.update.unsafe_head.as_ref().map(|h| h.block_info.number),
            safe_head = ?self.update.safe_head.as_ref().map(|h| h.block_info.number),
            finalized_head = ?self.update.finalized_head.as_ref().map(|h| h.block_info.number),
            ?duration,
            "Synchronized engine state with external follow source"
        );

        Ok(())
    }
}
