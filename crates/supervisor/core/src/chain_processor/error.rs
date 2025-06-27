use crate::{logindexer::LogIndexerError, syncnode::ManagedNodeError};
use kona_supervisor_storage::StorageError;
use thiserror::Error;

/// Errors that may occur while processing chains in the supervisor core.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChainProcessorError {
    /// Represents an error that occurred while interacting with the managed node.
    #[error(transparent)]
    ManagedNode(#[from] ManagedNodeError),

    /// Represents an error that occurred while interacting with the storage layer.
    #[error(transparent)]
    StorageError(#[from] StorageError),

    /// Represents an error that occurred while indexing logs.
    #[error(transparent)]
    LogIndexerError(#[from] LogIndexerError),
}
