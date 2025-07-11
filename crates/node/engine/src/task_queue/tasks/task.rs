//! Tasks sent to the [`Engine`] for execution.
//!
//! [`Engine`]: crate::Engine

use super::{BuildTask, ConsolidateTask, FinalizeTask, ForkchoiceTask, InsertUnsafeTask};
use crate::EngineState;
use async_trait::async_trait;
use std::cmp::Ordering;
use thiserror::Error;

/// Tasks that may be inserted into and executed by the [`Engine`].
///
/// [`Engine`]: crate::Engine
#[derive(Debug, Clone)]
pub enum EngineTask {
    /// Perform a `engine_forkchoiceUpdated` call with the current [`EngineState`]'s forkchoice,
    /// and no payload attributes.
    ForkchoiceUpdate(ForkchoiceTask),
    /// Inserts an unsafe payload into the execution engine.
    InsertUnsafe(InsertUnsafeTask),
    /// Builds a new block with the given attributes, and inserts it into the execution engine.
    BuildBlock(BuildTask),
    /// Performs consolidation on the engine state, reverting to payload attribute processing
    /// via the [`BuildTask`] if consolidation fails.
    Consolidate(ConsolidateTask),
    /// Finalizes an L2 block
    Finalize(FinalizeTask),
}

impl EngineTask {
    /// Executes the task without consuming it.
    async fn execute_inner(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        match self.clone() {
            Self::ForkchoiceUpdate(task) => task.execute(state).await.map(|_| ()),
            Self::InsertUnsafe(task) => task.execute(state).await,
            Self::BuildBlock(task) => task.execute(state).await,
            Self::Consolidate(task) => task.execute(state).await,
            Self::Finalize(task) => task.execute(state).await,
        }
    }
}

impl PartialEq for EngineTask {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::ForkchoiceUpdate(_), Self::ForkchoiceUpdate(_)) |
                (Self::InsertUnsafe(_), Self::InsertUnsafe(_)) |
                (Self::BuildBlock(_), Self::BuildBlock(_)) |
                (Self::Consolidate(_), Self::Consolidate(_)) |
                (Self::Finalize(_), Self::Finalize(_))
        )
    }
}

impl Eq for EngineTask {}

impl PartialOrd for EngineTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EngineTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Order (descending): ForkchoiceUpdate -> BuildBlock -> InsertUnsafe -> Consolidate
        //
        // https://specs.optimism.io/protocol/derivation.html#forkchoice-synchronization
        //
        // - Outstanding FCUs are processed before anything else.
        // - Block building jobs are prioritized above InsertUnsafe and Consolidate tasks, to give
        //   priority to the sequencer.
        // - InsertUnsafe tasks are prioritized over Consolidate tasks, to ensure that unsafe block
        //   gossip is imported promptly.
        // - Consolidate tasks are the lowest priority, as they are only used for advancing the safe
        //   chain via derivation.
        match (self, other) {
            // Same variant cases
            (Self::InsertUnsafe(_), Self::InsertUnsafe(_)) => Ordering::Equal,
            (Self::Consolidate(_), Self::Consolidate(_)) => Ordering::Equal,
            (Self::BuildBlock(_), Self::BuildBlock(_)) => Ordering::Equal,
            (Self::ForkchoiceUpdate(_), Self::ForkchoiceUpdate(_)) => Ordering::Equal,
            (Self::Finalize(_), Self::Finalize(_)) => Ordering::Equal,

            // Individual ForkchoiceUpdate tasks are the highest priority
            (Self::ForkchoiceUpdate(_), _) => Ordering::Greater,
            (_, Self::ForkchoiceUpdate(_)) => Ordering::Less,

            // BuildBlock tasks are prioritized over InsertUnsafe and Consolidate tasks
            (Self::BuildBlock(_), _) => Ordering::Greater,
            (_, Self::BuildBlock(_)) => Ordering::Less,

            // InsertUnsafe tasks are prioritized over Consolidate and Finalize tasks
            (Self::InsertUnsafe(_), _) => Ordering::Greater,
            (_, Self::InsertUnsafe(_)) => Ordering::Less,

            // Consolidate tasks are prioritized over Finalize tasks
            (Self::Consolidate(_), _) => Ordering::Greater,
            (_, Self::Consolidate(_)) => Ordering::Less,
        }
    }
}

#[async_trait]
impl EngineTaskExt for EngineTask {
    type Output = ();

    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        // Retry the task until it succeeds or a critical error occurs.
        while let Err(e) = self.execute_inner(state).await {
            match e {
                EngineTaskError::Temporary(e) => {
                    trace!(target: "engine", "{e}");
                    continue;
                }
                EngineTaskError::Critical(e) => {
                    error!(target: "engine", "{e}");
                    return Err(EngineTaskError::Critical(e));
                }
                EngineTaskError::Reset(e) => {
                    warn!(target: "engine", "Engine requested derivation reset");
                    return Err(EngineTaskError::Reset(e));
                }
                EngineTaskError::Flush(e) => {
                    warn!(target: "engine", "Engine requested derivation flush");
                    return Err(EngineTaskError::Flush(e));
                }
            }
        }

        Ok(())
    }
}

/// The interface for an engine task.
#[async_trait]
pub trait EngineTaskExt {
    /// The output type of the task.
    type Output;

    /// Executes the task, taking a shared lock on the engine state and `self`.
    async fn execute(&self, state: &mut EngineState) -> Result<Self::Output, EngineTaskError>;
}

/// An error that may occur during an [`EngineTask`]'s execution.
#[derive(Error, Debug)]
pub enum EngineTaskError {
    /// A temporary error within the engine.
    #[error("Temporary engine task error: {0}")]
    Temporary(Box<dyn std::error::Error + Send + Sync>),
    /// A critical error within the engine.
    #[error("Critical engine task error: {0}")]
    Critical(Box<dyn std::error::Error + Send + Sync>),
    /// An error that requires a derivation pipeline reset.
    #[error("Derivation pipeline reset required: {0}")]
    Reset(Box<dyn std::error::Error + Send + Sync>),
    /// An error that requires the derivation pipeline to be flushed.
    #[error("Derivation pipeline flush required: {0}")]
    Flush(Box<dyn std::error::Error + Send + Sync>),
}
