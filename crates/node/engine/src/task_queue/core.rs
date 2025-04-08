//! The [Engine] is a task queue that receives and executes [EngineTask]s.

use super::{EngineTaskError, EngineTaskExt};
use crate::{EngineState, EngineTask};
use std::collections::VecDeque;

/// The [Engine] task queue.
///
/// Tasks are processed in FIFO order, providing synchronization and ordering guarantees
/// for the L2 execution layer and other actors. Because tasks are executed one at a time,
/// they are considered to be atomic operations over the [EngineState], and are given
/// exclusive access to the engine state during execution.
///
/// Tasks within the queue are also considered fallible. If they fail with a temporary error,
/// they are not popped from the queue and are retried on the next call to [Engine::drain].
#[derive(Debug)]
pub struct Engine {
    /// The state of the engine.
    state: EngineState,
    /// The task queue.
    tasks: VecDeque<EngineTask>,
}

impl Engine {
    /// Creates a new [Engine] with an empty task queue and the passed initial [EngineState].
    ///
    /// An initial [EngineTask::ForkchoiceUpdate] is added to the task queue to synchronize the
    /// engine with the forkchoice state of the [EngineState].
    pub const fn new(initial_state: EngineState) -> Self {
        Self { state: initial_state, tasks: VecDeque::new() }
    }

    /// Enqueues a new [EngineTask] for execution.
    pub fn enqueue(&mut self, task: EngineTask) {
        self.tasks.push_back(task);
    }

    /// Clears the task queue.
    pub fn clear(&mut self) {
        self.tasks.clear();
    }

    /// Attempts to drain the queue by executing all [EngineTask]s in-order. If any task returns an
    /// error along the way, it is not popped from the queue (in case it must be retried) and
    /// the error is returned.
    ///
    /// If an [EngineTaskError::Reset] is encountered, the remaining tasks in the queue are cleared.
    pub async fn drain(&mut self) -> Result<(), EngineTaskError> {
        while let Some(task) = self.tasks.front() {
            match task.execute(&mut self.state).await {
                Ok(_) => {
                    // Dequeue the task if it was successful.
                    self.tasks.pop_front();
                }
                Err(EngineTaskError::Reset(e)) => {
                    self.clear();
                    return Err(EngineTaskError::Reset(e));
                }
                e => return e,
            }
        }

        Ok(())
    }
}
