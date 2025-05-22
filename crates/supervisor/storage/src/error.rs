use reth_db::DatabaseError;
use thiserror::Error;

/// Errors that may occur while interacting with supervisor log storage.
///
/// This enum is used across all implementations of the Storge traits.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum StorageError {
    /// Represents a database error that occurred while interacting with storage.
    #[error("Database error")]
    Database(#[source] DatabaseError),

    /// The expected entry was not found in the database.
    #[error("Entry not found: {0}")]
    EntryNotFound(String),
}
