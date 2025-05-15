//! The [`Engine`] is a task queue that receives and executes [`EngineTask`]s.

use super::{EngineTaskError, EngineTaskExt};
use crate::{EngineClient, EngineState, EngineTask, EngineTaskType, ForkchoiceTask};
use alloy_provider::Provider;
use alloy_rpc_types_eth::Transaction;
use kona_genesis::{RollupConfig, SystemConfig};
use kona_protocol::{BlockInfo, L2BlockInfo, OpBlockConversionError, to_system_config};
use kona_sources::{SyncStartError, find_starting_forkchoice};
use op_alloy_consensus::OpTxEnvelope;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use thiserror::Error;
use tokio::sync::watch::Sender;

/// The [`Engine`] task queue.
///
/// Tasks are processed in FIFO order, providing synchronization and ordering guarantees
/// for the L2 execution layer and other actors. Because tasks are executed one at a time,
/// they are considered to be atomic operations over the [`EngineState`], and are given
/// exclusive access to the engine state during execution.
///
/// Tasks within the queue are also considered fallible. If they fail with a temporary error,
/// they are not popped from the queue and are retried on the next call to [`Engine::drain`].
#[derive(Debug)]
pub struct Engine {
    /// The state of the engine.
    state: EngineState,
    /// A sender that can be used to notify the engine actor of state changes.
    state_sender: Sender<EngineState>,
    /// The task queue.
    tasks: HashMap<EngineTaskType, VecDeque<EngineTask>>,
    /// The current task being executed.
    cursor: EngineTaskType,
}

impl Engine {
    /// Creates a new [`Engine`] with an empty task queue and the passed initial [`EngineState`].
    ///
    /// An initial [`EngineTask::ForkchoiceUpdate`] is added to the task queue to synchronize the
    /// engine with the forkchoice state of the [`EngineState`].
    pub fn new(initial_state: EngineState, state_sender: Sender<EngineState>) -> Self {
        Self {
            state: initial_state,
            tasks: HashMap::new(),
            cursor: EngineTaskType::ForkchoiceUpdate,
            state_sender,
        }
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
        self.tasks.entry(task.ty()).or_default().push_back(task);
    }

    /// Resets the engine by finding a plausible sync starting point via
    /// [`find_starting_forkchoice`]. The state will be updated to the starting point, and a
    /// forkchoice update will be enqueued in order to reorg the execution layer.
    pub async fn reset(
        &mut self,
        client: Arc<EngineClient>,
        config: &RollupConfig,
    ) -> Result<(L2BlockInfo, BlockInfo, SystemConfig), EngineResetError> {
        let start =
            find_starting_forkchoice(config, client.l1_provider(), client.l2_provider()).await?;

        self.state.set_unsafe_head(start.un_safe);
        self.state.set_cross_unsafe_head(start.un_safe);
        self.state.set_local_safe_head(start.safe);
        self.state.set_safe_head(start.safe);
        self.state.set_finalized_head(start.finalized);

        self.enqueue(EngineTask::ForkchoiceUpdate(ForkchoiceTask::new(client.clone())));

        let origin_block =
            start.safe.l1_origin.number - config.channel_timeout(start.safe.block_info.timestamp);
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
        let system_config = to_system_config(&l2_safe_block, config)?;

        Ok((start.safe, l1_origin_info, system_config))
    }

    /// Clears the task queue.
    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    /// Returns the next task type to be executed.
    pub fn next(&self) -> EngineTaskType {
        let mut ty = self.cursor;
        let task_len = self.tasks.len();
        for _ in 0..task_len {
            if !self.tasks.contains_key(&ty) {
                ty = ty.next();
            } else {
                break;
            }
        }
        ty
    }

    /// Attempts to drain the queue by executing all [`EngineTask`]s in-order. If any task returns
    /// an error along the way, it is not popped from the queue (in case it must be retried) and
    /// the error is returned.
    ///
    /// If an [`EngineTaskError::Reset`] is encountered, the remaining tasks in the queue are
    /// cleared.
    pub async fn drain(&mut self) -> Result<(), EngineTaskError> {
        loop {
            let ty = self.next();
            self.cursor = self.cursor.next();
            let Some(task) = self.tasks.get(&ty) else {
                return Ok(());
            };
            let Some(task) = task.front() else {
                return Ok(());
            };
            match task.execute(&mut self.state).await {
                Ok(_) => {}
                Err(EngineTaskError::Reset(e)) => {
                    self.clear();
                    return Err(EngineTaskError::Reset(e));
                }
                e => return e,
            }

            // Update the state and notify the engine actor.
            self.state_sender.send_replace(self.state);

            let ty = task.ty();
            if let Some(queue) = self.tasks.get_mut(&ty) {
                queue.pop_front();
            };
        }
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
