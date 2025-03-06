//! Contains the error types for the [crate::InsertTask].

/// An error that occurs when running the [crate::InsertTask].
#[derive(Debug, thiserror::Error)]
pub enum InsertTaskError {
    /// An invalid sync status response.
    #[error("invalid sync status response")]
    InvalidSyncStatusResponse,
    /// Failed to receive a message from the external actor.
    #[error("failed to receive message from external actor")]
    ReceiveFailed,
    /// Received an invalid message response from the external actor.
    #[error("received invalid message response from external actor")]
    InvalidMessageResponse,
}
