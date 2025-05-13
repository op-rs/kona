//! Error type for the [`crate::EngineStateBuilder`].

use crate::{EngineTask, client::EngineClientError};
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

/// An error that occurs in the [`crate::EngineStateBuilder`].
#[derive(Error, Debug)]
pub enum EngineStateBuilderError {
    /// An error thrown by the [`crate::EngineClient`].
    #[error("Engine Client request failed: {0}")]
    EngineClientError(#[from] EngineClientError),
    /// Missing unsafe head when building the [`crate::EngineState`].
    #[error("The unsafe head is required to build the EngineState")]
    MissingUnsafeHead,
    /// Missing the finalized head when building the [`crate::EngineState`].
    #[error("The finalized head is required to build the EngineState")]
    MissingFinalizedHead,
    /// Missing the safe head when building the [`crate::EngineState`].
    #[error("The safe head is required to build the EngineState")]
    MissingSafeHead,
    /// Failed to send initial engine tasks.
    #[error("Failed to send initial engine tasks: {0}")]
    InitialTasksError(#[from] SendError<EngineTask>),
}
