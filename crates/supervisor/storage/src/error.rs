use thiserror::Error;

/// A dynamic error type for encapsulating low-level database/backend errors.
///
/// This type is used as the source for most [`StorageError`] variants to allow
/// flexibility in wrapping reth database errors uniformly.
pub type SourceError = Box<dyn std::error::Error + Send + Sync>;

/// Errors that may occur while interacting with supervisor log storage.
///
/// This enum is used across all implementations of the Storge traits.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Failed to initialize the underlying database environment or schema.
    #[error("Database initialization failed")]
    DatabaseInit(#[source] SourceError),

    /// Failed to read from the database.
    #[error("Database read failed")]
    DatabaseRead(#[source] SourceError),

    /// Failed to write to the database.
    #[error("Database write failed")]
    DatabaseWrite(#[source] SourceError),

    /// Failed to create or begin a read or write transaction.
    #[error("Transaction initialization failed")]
    TransactionInit(#[source] SourceError),

    /// Failed to commit a transaction.
    #[error("Transaction commit failed")]
    TransactionCommit(#[source] SourceError),

    /// Failed to initialize a cursor or dup-sorted cursor.
    #[error("Cursor initialization failed")]
    CursorInit(#[source] SourceError),

    /// The expected entry was not found in the database.
    #[error("Entry not found: {0}")]
    EntryNotFound(String),
}
