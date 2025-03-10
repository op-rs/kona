//! Contains error types for the [crate::ForkchoiceTask].

/// An error that occurs when running the [crate::ForkchoiceTask].
#[derive(Debug, thiserror::Error)]
pub enum ForkchoiceTaskError {
    /// The forkchoice update is not needed.
    #[error("No forkchoice update needed")]
    NoForkchoiceUpdateNeeded,
    /// The forkchoice state is invalid.
    #[error("Invalid forkchoice state: {0} < {1}")]
    InvalidForkchoiceState(u64, u64),
    /// The forkchoice response is invalid.
    #[error("Invalid forkchoice response")]
    InvalidForkchoiceResponse,
    /// The sync status response is invalid.
    #[error("Invalid sync status response")]
    InvalidSyncStatusResponse,
    /// A receive error occurred.
    #[error("Receive error")]
    ReceiveError,
    /// The forkchoice update call to the engine api failed.
    #[error("Forkchoice update engine api call failed")]
    ForkchoiceUpdateFailed,
}
