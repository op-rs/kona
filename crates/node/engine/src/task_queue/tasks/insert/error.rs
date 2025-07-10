//! Contains the error types for the [InsertUnsafeTask].
//!
//! [InsertUnsafeTask]: crate::InsertUnsafeTask

use crate::EngineTaskError;
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_protocol::FromBlockError;
use op_alloy_rpc_types_engine::OpPayloadError;

/// An error that occurs when running the [InsertUnsafeTask].
///
/// [InsertUnsafeTask]: crate::InsertUnsafeTask
#[derive(Debug, thiserror::Error)]
pub enum InsertUnsafeTaskError {
    /// Error converting a payload into a block.
    #[error(transparent)]
    FromBlockError(#[from] OpPayloadError),
    /// Failed to insert new payload.
    #[error("Failed to insert new payload: {0}")]
    InsertFailed(RpcError<TransportErrorKind>),
    /// Unexpected payload status
    #[error("Unexpected payload status: {0}")]
    UnexpectedPayloadStatus(PayloadStatusEnum),
    /// Inconsistent forchoice state.
    #[error("Inconsistent forkchoice state; Pipeline reset required")]
    InconsistentForkchoiceState,
    /// Error converting the payload + chain genesis into an L2 block info.
    #[error(transparent)]
    L2BlockInfoConstruction(#[from] FromBlockError),
}

impl From<InsertUnsafeTaskError> for EngineTaskError {
    fn from(value: InsertUnsafeTaskError) -> Self {
        match value {
            InsertUnsafeTaskError::FromBlockError(_) => Self::Critical(Box::new(value)),
            InsertUnsafeTaskError::InsertFailed(_) => Self::Temporary(Box::new(value)),
            InsertUnsafeTaskError::UnexpectedPayloadStatus(_) => Self::Critical(Box::new(value)),
            InsertUnsafeTaskError::L2BlockInfoConstruction(_) => Self::Critical(Box::new(value)),
            InsertUnsafeTaskError::InconsistentForkchoiceState => Self::Reset(Box::new(value)),
        }
    }
}
