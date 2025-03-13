//! Contains the error types for the [InsertUnsafeTask].
//!
//! [InsertUnsafeTask]: crate::InsertUnsafeTask

use crate::EngineTaskError;
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_protocol::FromBlockError;

/// An error that occurs when running the [InsertUnsafeTask].
///
/// [InsertUnsafeTask]: crate::InsertUnsafeTask
#[derive(Debug, thiserror::Error)]
pub enum InsertUnsafeTaskError {
    /// Could not fetch the finalized L2 block.
    #[error("Could not fetch the finalized L2 block")]
    FinalizedBlockFetch,
    /// Failed to insert new payload.
    #[error("Failed to insert new payload: {0}")]
    InsertFailed(RpcError<TransportErrorKind>),
    /// Failed to update the forkchoice.
    #[error("Failed to update the forkchoice: {0}")]
    ForkchoiceUpdateFailed(RpcError<TransportErrorKind>),
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
            InsertUnsafeTaskError::FinalizedBlockFetch => Self::Temporary(Box::new(value)),
            InsertUnsafeTaskError::InsertFailed(_) => Self::Temporary(Box::new(value)),
            InsertUnsafeTaskError::ForkchoiceUpdateFailed(_) => Self::Temporary(Box::new(value)),
            InsertUnsafeTaskError::UnexpectedPayloadStatus(_) => Self::Temporary(Box::new(value)),
            InsertUnsafeTaskError::L2BlockInfoConstruction(_) => Self::Critical(Box::new(value)),
            InsertUnsafeTaskError::InconsistentForkchoiceState => Self::Reset(Box::new(value)),
        }
    }
}
