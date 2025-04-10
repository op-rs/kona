//! Tasks sent to the [Engine] for execution.
//!
//! [Engine]: crate::Engine

use super::{BuildTask, ConsolidateTask, ForkchoiceTask, InsertUnsafeTask};
use crate::EngineState;
use async_trait::async_trait;
use thiserror::Error;

/// Tasks that may be inserted into and executed by the [Engine].
///
/// [Engine]: crate::Engine
#[derive(Debug, Clone)]
pub enum EngineTask {
    /// Perform a `engine_forkchoiceUpdated` call with the current [EngineState]'s forkchoice,
    /// and no payload attributes.
    ForkchoiceUpdate(ForkchoiceTask),
    /// Inserts an unsafe payload into the execution engine.
    InsertUnsafe(InsertUnsafeTask),
    /// Builds a new block with the given attributes, and inserts it into the execution engine.
    BuildBlock(BuildTask),
    /// Performs consolidation on the engine state, reverting to payload attribute processing
    /// via the [`BuildTask`] if consolidation fails.
    Consolidate(ConsolidateTask),
}

impl EngineTask {
    /// Executes the task without consuming it.
    async fn execute_inner(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        match self.clone() {
            Self::ForkchoiceUpdate(task) => task.execute(state).await,
            Self::InsertUnsafe(task) => task.execute(state).await,
            Self::BuildBlock(task) => task.execute(state).await,
            Self::Consolidate(task) => task.execute(state).await,
        }
    }
}

#[async_trait]
impl EngineTaskExt for EngineTask {
    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError> {
        // Retry the task until it succeeds or a critical error occurs.
        while let Err(e) = self.execute_inner(state).await {
            match e {
                EngineTaskError::Temporary(e) => {
                    debug!(target: "engine", "{e}");
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
            }
        }

        Ok(())
    }
}

/// The interface for an engine task.
#[async_trait]
pub trait EngineTaskExt {
    /// Executes the task, taking a shared lock on the engine state and `self`.
    async fn execute(&self, state: &mut EngineState) -> Result<(), EngineTaskError>;
}

/// An error that may occur during an [EngineTask]'s execution.
#[derive(Error, Debug)]
pub enum EngineTaskError {
    /// A temporary error within the engine.
    #[error("Temporary engine task error: {0}")]
    Temporary(Box<dyn std::error::Error>),
    /// A critical error within the engine.
    #[error("Critical engine task error: {0}")]
    Critical(Box<dyn std::error::Error>),
    /// An error that requires a derivation pipeline reset.
    #[error("Derivation pipeline reset required: {0}")]
    Reset(Box<dyn std::error::Error>),
}
