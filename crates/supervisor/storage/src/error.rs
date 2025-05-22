use reth_db::DatabaseError;
use thiserror::Error;

/// Errors that may occur while interacting with supervisor log storage.
///
/// This enum is used across all implementations of the Storge traits.
#[derive(Debug, Error)]
pub enum StorageError {
    /// DatabaseError
    #[error("Database error")]
    Database(#[source] DatabaseError),

    /// The expected entry was not found in the database.
    #[error("Entry not found: {0}")]
    EntryNotFound(String),

    /// Represents a conflict occurred while attempting to write to the database.
    #[error("Conflict error: {0}")]
    ConflictError(String),
}
