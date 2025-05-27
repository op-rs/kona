use reth_db::DatabaseError;
use thiserror::Error;

/// Errors that may occur while interacting with supervisor log storage.
///
/// This enum is used across all implementations of the Storge traits.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Represents a database error that occurred while interacting with storage.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// Represents an error that occurred while initializing the database.
    #[error(transparent)]
    DatabaseInit(#[from] eyre::Report),

    /// The expected entry was not found in the database.
    #[error("entry not found: {0}")]
    EntryNotFound(String),

    /// Represents a conflict occurred while attempting to write to the database.
    #[error("conflict error: {0}")]
    ConflictError(String),
}
