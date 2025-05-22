use thiserror::Error;

/// A dynamic error type for encapsulating low-level database/backend errors.
///
/// This type is used as the source for most [`StorageError`] variants to allow
/// flexibility in wrapping reth database errors uniformly.
pub(crate) type SourceError = Box<dyn std::error::Error + Send + Sync>;

/// Errors that may occur while interacting with supervisor log storage.
///
/// This enum is used across all implementations of the Storge traits.
#[allow(dead_code)]
#[derive(Debug, Error)]
pub(crate) enum StorageError {
    /// DatabaseError
    #[error("Database error")]
    Database(#[source] DatabaseError),

    /// The expected entry was not found in the database.
    #[error("Entry not found: {0}")]
    EntryNotFound(String),
}
