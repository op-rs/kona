//! The [`Engine`] is a task queue that receives and executes [`EngineTask`]s.

use super::{EngineTaskError, EngineTaskExt};
use crate::{EngineState, EngineTask, EngineTaskType};
use kona_protocol::L2BlockInfo;
use std::collections::{HashMap, VecDeque};
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
