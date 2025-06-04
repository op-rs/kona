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

impl PartialEq for StorageError {
    fn eq(&self, other: &Self) -> bool {
        use StorageError::*;
        match (self, other) {
            (Database(a), Database(b)) => a == b,
            (DatabaseInit(a), DatabaseInit(b)) => format!("{}", a) == format!("{}", b),
            (EntryNotFound(a), EntryNotFound(b)) => a == b,
            (InvalidAnchor, InvalidAnchor) => true,
            (DatabaseNotInitialised, DatabaseNotInitialised) => true,
            (ConflictError(a), ConflictError(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for StorageError {}
