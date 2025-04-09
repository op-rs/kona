//! The [Engine] is a task queue that receives and executes [EngineTask]s.

use super::{EngineTaskError, EngineTaskExt};
use crate::{EngineState, EngineTask};
use kona_rpc::OpAttributesWithParent;
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

    /// Returns if consolidation is needed.
    ///
    /// [Consolidation] is only performed by a rollup node when the unsafe head
    /// is ahead of the safe head. When the two are equal, consolidation isn't
    /// required and the [`crate::BuildTask`] can be used to build the block.
    ///
    /// [Consolidation]: https://specs.optimism.io/protocol/derivation.html#l1-consolidation-payload-attributes-matching
    pub fn needs_consolidation(&self) -> bool {
        self.state.safe_head() != self.state.unsafe_head()
    }

    /// Consolidates the oldest unsafe head (the unsafe head immediately _after_ the safe head).
    ///
    /// Will only consolidate if [`Self::needs_consolidation`] returns true. That is,
    /// if the unsafe head is ahead of the safe head (aka they're not equal).
    pub fn consolidate(
        &mut self,
        _attributes: &OpAttributesWithParent,
    ) -> Result<(), ConsolidationError> {
        if self.needs_consolidation() {
            debug!(target: "engine", "Performing consolidation");
            // TODO: consolidate
            // see: https://specs.optimism.io/protocol/derivation.html#l1-consolidation-payload-attributes-matching
        } else {
            debug!(target: "engine", "Skipping consolidation. Safe head [{}] == Unsafe Head [{}]", self.state.safe_head().block_info.number, self.state.unsafe_head().block_info.number);
        }
        Ok(())
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

/// An error occured during consolidation.
#[derive(thiserror::Error, Debug, Clone)]
pub enum ConsolidationError {}
