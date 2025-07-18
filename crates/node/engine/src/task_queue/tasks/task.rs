//! Tasks sent to the [`Engine`] for execution.
//!
//! [`Engine`]: crate::Engine

use super::{BuildTask, ConsolidateTask, FinalizeTask, ForkchoiceTask, InsertTask};
use crate::{
    BuildTaskError, ConsolidateTaskError, EngineState, FinalizeTaskError, ForkchoiceTaskError,
    InsertTaskError,
};
use async_trait::async_trait;
use std::cmp::Ordering;
use thiserror::Error;

/// The severity of an engine task error.
///
/// This is used to determine how to handle the error when draining the engine task queue.
#[derive(Debug, PartialEq, Eq)]
pub enum EngineTaskErrorSeverity {
    /// The error is temporary and the task is retried.
    Temporary,
    /// The error is critical and is propagated to the engine actor.
    Critical,
    /// The error indicates that the engine should be reset.
    Reset,
    /// The error indicates that the engine should be flushed.
    Flush,
}

/// The interface for an engine task error.
///
/// An engine task error should have an associated severity level to specify how to handle the error
/// when draining the engine task queue.
pub trait EngineTaskError {
    /// The severity of the error.
    fn severity(&self) -> EngineTaskErrorSeverity;
}

/// The interface for an engine task.
#[async_trait]
pub trait EngineTaskExt {
    /// The output type of the task.
    type Output;

    /// The error type of the task.
    type Error: EngineTaskError;

    /// Executes the task, taking a shared lock on the engine state and `self`.
    async fn execute(&self, state: &mut EngineState) -> Result<Self::Output, Self::Error>;
}

/// An error that may occur during an [`EngineTask`]'s execution.
#[derive(Error, Debug)]
pub enum EngineTaskErrors {
    /// An error that occurred while updating the forkchoice state.
    #[error(transparent)]
    Forkchoice(#[from] ForkchoiceTaskError),
    /// An error that occurred while inserting a block into the engine.
    #[error(transparent)]
    Insert(#[from] InsertTaskError),
    /// An error that occurred while building a block.
    #[error(transparent)]
    Build(#[from] BuildTaskError),
    /// An error that occurred while consolidating the engine state.
    #[error(transparent)]
    Consolidate(#[from] ConsolidateTaskError),
    /// An error that occurred while finalizing an L2 block.
    #[error(transparent)]
    Finalize(#[from] FinalizeTaskError),
}

impl EngineTaskError for EngineTaskErrors {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::Forkchoice(inner) => inner.severity(),
            Self::Insert(inner) => inner.severity(),
            Self::Build(inner) => inner.severity(),
            Self::Consolidate(inner) => inner.severity(),
            Self::Finalize(inner) => inner.severity(),
        }
    }
}

/// Tasks that may be inserted into and executed by the [`Engine`].
///
/// [`Engine`]: crate::Engine
#[derive(Debug, Clone)]
pub enum EngineTask {
    /// Perform a `engine_forkchoiceUpdated` call with the current [`EngineState`]'s forkchoice,
    /// and no payload attributes.
    ForkchoiceUpdate(ForkchoiceTask),
    /// Inserts a payload into the execution engine.
    Insert(InsertTask),
    /// Builds a new block with the given attributes, and inserts it into the execution engine.
    Build(BuildTask),
    /// Performs consolidation on the engine state, reverting to payload attribute processing
    /// via the [`BuildTask`] if consolidation fails.
    Consolidate(ConsolidateTask),
    /// Finalizes an L2 block
    Finalize(FinalizeTask),
}

impl EngineTask {
    /// Executes the task without consuming it.
    async fn execute_inner(&self, state: &mut EngineState) -> Result<(), EngineTaskErrors> {
        match self.clone() {
            Self::ForkchoiceUpdate(task) => task.execute(state).await.map(|_| ())?,
            Self::Insert(task) => task.execute(state).await?,
            Self::Build(task) => task.execute(state).await?,
            Self::Consolidate(task) => task.execute(state).await?,
            Self::Finalize(task) => task.execute(state).await?,
        };

        Ok(())
    }
}

impl PartialEq for EngineTask {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::ForkchoiceUpdate(_), Self::ForkchoiceUpdate(_)) |
                (Self::Insert(_), Self::Insert(_)) |
                (Self::Build(_), Self::Build(_)) |
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
            (Self::Insert(_), Self::Insert(_)) => Ordering::Equal,
            (Self::Consolidate(_), Self::Consolidate(_)) => Ordering::Equal,
            (Self::Build(_), Self::Build(_)) => Ordering::Equal,
            (Self::ForkchoiceUpdate(_), Self::ForkchoiceUpdate(_)) => Ordering::Equal,
            (Self::Finalize(_), Self::Finalize(_)) => Ordering::Equal,

            // Individual ForkchoiceUpdate tasks are the highest priority
            (Self::ForkchoiceUpdate(_), _) => Ordering::Greater,
            (_, Self::ForkchoiceUpdate(_)) => Ordering::Less,

            // BuildBlock tasks are prioritized over InsertUnsafe and Consolidate tasks
            (Self::Build(_), _) => Ordering::Greater,
            (_, Self::Build(_)) => Ordering::Less,

            // InsertUnsafe tasks are prioritized over Consolidate and Finalize tasks
            (Self::Insert(_), _) => Ordering::Greater,
            (_, Self::Insert(_)) => Ordering::Less,

            // Consolidate tasks are prioritized over Finalize tasks
            (Self::Consolidate(_), _) => Ordering::Greater,
            (_, Self::Consolidate(_)) => Ordering::Less,
        }
    }
}

#[async_trait]
impl EngineTaskExt for EngineTask {
    type Output = ();

    type Error = EngineTaskErrors;

    async fn execute(&self, state: &mut EngineState) -> Result<(), Self::Error> {
        // Retry the task until it succeeds or a critical error occurs.
        while let Err(e) = self.execute_inner(state).await {
            match e.severity() {
                EngineTaskErrorSeverity::Temporary => {
                    trace!(target: "engine", "{e}");
                    continue;
                }
                EngineTaskErrorSeverity::Critical => {
                    error!(target: "engine", "{e}");
                    return Err(e);
                }
                EngineTaskErrorSeverity::Reset => {
                    warn!(target: "engine", "Engine requested derivation reset");
                    return Err(e);
                }
                EngineTaskErrorSeverity::Flush => {
                    warn!(target: "engine", "Engine requested derivation flush");
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}
