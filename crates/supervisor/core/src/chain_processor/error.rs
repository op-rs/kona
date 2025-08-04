use crate::{
    logindexer::LogIndexerError,
    syncnode::ManagedNodeCommand,
};
use kona_supervisor_storage::StorageError;
use thiserror::Error;
use tokio::sync::mpsc;

/// Errors that may occur while processing chains in the supervisor core.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ChainProcessorError {
    /// Represents an error that occurred while interacting with the storage layer.
    #[error(transparent)]
    StorageError(#[from] StorageError),

    /// Represents an error that occurred while indexing logs.
    #[error(transparent)]
    LogIndexerError(#[from] LogIndexerError),

    /// Represents an error that occurred while sending an event to the channel.
    #[error(transparent)]
    ChannelSendFailed(#[from] mpsc::error::SendError<ManagedNodeCommand>),
}
