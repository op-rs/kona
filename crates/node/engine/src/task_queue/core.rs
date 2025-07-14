//! The [`Engine`] is a task queue that receives and executes [`EngineTask`]s.

use super::{EngineTaskError, EngineTaskExt};
use crate::{
    EngineClient, EngineState, EngineSyncStateUpdate, EngineTask, ForkchoiceTask, Metrics,
};
use alloy_provider::Provider;
use alloy_rpc_types_eth::Transaction;
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::{BlockInfo, L2BlockInfo, OpBlockConversionError, to_system_config};
use kona_sources::{SyncStartError, find_starting_forkchoice};
use op_alloy_consensus::OpTxEnvelope;
use std::{collections::BinaryHeap, sync::Arc};
use thiserror::Error;
use tokio::sync::watch::Sender;

/// The [`Engine`] task queue.
///
/// Tasks of a shared [`EngineTask`] variant are processed in FIFO order, providing synchronization
/// guarantees for the L2 execution layer and other actors. A priority queue, ordered by
/// [`EngineTask`]'s [`Ord`] implementation, is used to prioritize tasks executed by the
/// [`Engine::drain`] method.
///
///  Because tasks are executed one at a time, they are considered to be atomic operations over the
/// [`EngineState`], and are given exclusive access to the engine state during execution.
///
/// Tasks within the queue are also considered fallible. If they fail with a temporary error,
/// they are not popped from the queue, the error is returned, and they are retried on the
/// next call to [`Engine::drain`].
#[derive(Debug)]
pub struct Engine {
    /// The state of the engine.
    state: EngineState,
    /// A sender that can be used to notify the engine actor of state changes.
    state_sender: Sender<EngineState>,
    /// The task queue.
    tasks: BinaryHeap<EngineTask>,
}

impl Engine {
    /// Creates a new [`Engine`] with an empty task queue and the passed initial [`EngineState`].
    ///
    /// An initial [`EngineTask::ForkchoiceUpdate`] is added to the task queue to synchronize the
    /// engine with the forkchoice state of the [`EngineState`].
    pub fn new(initial_state: EngineState, state_sender: Sender<EngineState>) -> Self {
        Self { state: initial_state, state_sender, tasks: BinaryHeap::default() }
    }

    /// Returns a reference to the inner [`EngineState`].
    pub const fn state(&self) -> &EngineState {
        &self.state
    }

    /// Returns a receiver that can be used to listen to engine state updates.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<EngineState> {
        self.state_sender.subscribe()
    }

    /// Enqueues a new [`EngineTask`] for execution.
    pub fn enqueue(&mut self, task: EngineTask) {
        self.tasks.push(task);
    }

    /// Resets the engine by finding a plausible sync starting point via
    /// [`find_starting_forkchoice`]. The state will be updated to the starting point, and a
    /// forkchoice update will be enqueued in order to reorg the execution layer.
    pub async fn reset(
        &mut self,
        client: Arc<EngineClient>,
        config: Arc<RollupConfig>,
    ) -> Result<(L2BlockInfo, BlockInfo, SystemConfig), EngineResetError> {
        // Clear any outstanding tasks to prepare for the reset.
        self.clear();

        let start =
            find_starting_forkchoice(&config, client.l1_provider(), client.l2_provider()).await?;

        ForkchoiceTask::new(
            client.clone(),
            config.clone(),
            EngineSyncStateUpdate {
                unsafe_head: Some(start.un_safe),
                cross_unsafe_head: Some(start.un_safe),
                local_safe_head: Some(start.safe),
                safe_head: Some(start.safe),
                finalized_head: Some(start.finalized),
            },
            None,
        )
        .execute(&mut self.state)
        .await?;

        // Find the new safe head's L1 origin and SystemConfig.
        let origin_block = start
            .safe
            .l1_origin
            .number
            .saturating_sub(config.channel_timeout(start.safe.block_info.timestamp));
        let l1_origin_info: BlockInfo = client
            .l1_provider()
            .get_block(origin_block.into())
            .await
            .map_err(SyncStartError::RpcError)?
            .ok_or(SyncStartError::BlockNotFound(origin_block.into()))?
            .into_consensus()
            .into();
        let l2_safe_block = client
            .l2_provider()
            .get_block(start.safe.block_info.hash.into())
            .full()
            .await
            .map_err(SyncStartError::RpcError)?
            .ok_or(SyncStartError::BlockNotFound(origin_block.into()))?
            .into_consensus()
            .map_transactions(|t| <Transaction<OpTxEnvelope> as Clone>::clone(&t).into_inner());
        let system_config = to_system_config(&l2_safe_block, &config)?;

        kona_macros::inc!(counter, Metrics::ENGINE_RESET_COUNT);

        Ok((start.safe, l1_origin_info, system_config))
    }

    /// Clears the task queue.
    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    /// Attempts to drain the queue by executing all [`EngineTask`]s in-order. If any task returns
    /// an error along the way, it is not popped from the queue (in case it must be retried) and
    /// the error is returned.
    pub async fn drain(&mut self) -> Result<(), EngineTaskError> {
        // Drain tasks in order of priority, halting on errors for a retry to be attempted.
        while let Some(task) = self.tasks.peek() {
            // Execute the task
            task.execute(&mut self.state).await?;

            // Update the state and notify the engine actor.
            self.state_sender.send_replace(self.state);

            // Pop the task from the queue now that it's been executed.
            self.tasks.pop();
        }

        Ok(())
    }
}

/// An error occurred while attempting to reset the [`Engine`].
#[derive(Debug, Error)]
pub enum EngineResetError {
    /// An error that originated from within the engine task.
    #[error(transparent)]
    Task(#[from] EngineTaskError),
    /// An error occurred while traversing the L1 for the sync starting point.
    #[error(transparent)]
    SyncStart(#[from] SyncStartError),
    /// An error occurred while constructing the SystemConfig for the new safe head.
    #[error(transparent)]
    SystemConfigConversion(#[from] OpBlockConversionError),
}
