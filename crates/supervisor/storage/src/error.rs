use reth_db::DatabaseError;
use thiserror::Error;

/// Errors that may occur while interacting with supervisor log storage.
///
/// This enum is used across all implementations of the Storge traits.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum StorageError {
    /// Represents a database error that occurred while interacting with storage.
    #[error(transparent)]
    Database(#[from] DatabaseError),

    /// Represents an error that occurred while initializing the database.
    #[error("database init error: {0}")]
    DatabaseInit(String),

    /// The expected entry was not found in the database.
    #[error("entry not found: {0}")]
    EntryNotFound(String),

    /// Represents an error that occurred while initializing the database with an anchor.
    #[error("invalid anchor")]
    InvalidAnchor,

    /// Represents an error that occurred when database is not initialized.
    #[error("database not initialized")]
    DatabaseNotInitialised,

    /// Represents a conflict occurred while attempting to write to the database.
    #[error("conflict error: {0}")]
    ConflictError(String),
}
