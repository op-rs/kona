use reth_db::DatabaseError;
use thiserror::Error;

/// Errors that may occur while interacting with supervisor log storage.
///
/// This enum is used across all implementations of the Storage traits.
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

    /// Represents an error that occurred while writing to log database.
    #[error("latest stored block is not parent of the incoming block")]
    BlockOutOfOrder,

    /// Represents an error that occurred while writing to derived block database.
    #[error("latest stored derived block is not parent of the incoming derived block")]
    DerivedBlockOutOfOrder,
}
