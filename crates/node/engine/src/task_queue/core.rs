//! The [`Engine`] is a task queue that receives and executes [`EngineTask`]s.

use super::{EngineTaskError, EngineTaskExt, ForkchoiceTask};
use crate::{EngineClient, EngineState, EngineTask, EngineTaskType};
use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use kona_sources::find_starting_forkchoice;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
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

    /// Enqueues a new [`EngineTask`] for execution.
    pub fn enqueue(&mut self, task: EngineTask) {
        self.tasks.entry(task.ty()).or_default().push_back(task);
    }

    /// Resets the engine state.
    pub async fn reset(
        &mut self,
        client: Arc<EngineClient>,
        config: Arc<RollupConfig>,
    ) -> Result<(), EngineTaskError> {
        let (mut l1_provider, mut l2_provider) = client.alloy_providers();
        let forkchoice = find_starting_forkchoice(&config, &mut l1_provider, &mut l2_provider)
            .await
            .map_err(|e| EngineTaskError::Critical(Box::new(e)))?;

        self.state.set_finalized_head(forkchoice.finalized);
        self.state.set_safe_head(forkchoice.safe);
        self.state.set_unsafe_head(forkchoice.un_safe);
        // If the cross unsafe head is not set, set it to the safe head.
        self.state.set_cross_unsafe_head(self.state.safe_head);
        // If the local safe head is not set, set it to the safe head.
        self.state.set_local_safe_head(self.state.safe_head);

        self.state.forkchoice_update_needed = true;

        debug!(target: "engine", unsafe = ?self.state.unsafe_head(), safe = ?self.state.safe_head(), finalized = ?self.state.finalized_head(),
         "Resetted engine state. Sending FCU");

        self.enqueue(EngineTask::ForkchoiceUpdate(ForkchoiceTask::new(client)));

        Ok(())
    }

    /// Returns the L2 Safe Head [`L2BlockInfo`] from the state.
    pub const fn safe_head(&self) -> L2BlockInfo {
        self.state.safe_head()
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

    /// Returns a receiver that can be used to listen to engine state updates.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<EngineState> {
        self.state_sender.subscribe()
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
            let ty = task.ty();

            match task.execute(&mut self.state).await {
                Ok(_) => {}
                Err(EngineTaskError::Reset(e)) => {
                    // The engine actor should trigger a reset by calling [`Engine::reset`].
                    return Err(EngineTaskError::Reset(e));
                }
                e => return e,
            }

            // Update the state and notify the engine actor.
            self.state_sender.send_replace(self.state);

            if let Some(queue) = self.tasks.get_mut(&ty) {
                queue.pop_front();
            };
        }
    }
}
