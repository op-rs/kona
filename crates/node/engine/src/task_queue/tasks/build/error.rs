//! Contains error types for the [crate::ForkchoiceTask].

use crate::EngineTaskError;
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use thiserror::Error;

/// An error that occurs when running the [crate::ForkchoiceTask].
#[derive(Debug, Error)]
pub enum BuildTaskError {
    /// The forkchoice update is not needed.
    #[error("No forkchoice update needed")]
    NoForkchoiceUpdateNeeded,
    /// The engine is syncing.
    #[error("Attempting to update forkchoice state while EL syncing")]
    EngineSyncing,
    /// The forkchoice update call to the engine api failed.
    #[error(transparent)]
    ForkchoiceUpdateFailed(RpcError<TransportErrorKind>),
    /// Missing payload ID.
    #[error("Missing payload ID")]
    MissingPayloadId,
    /// Unexpected payload status
    #[error("Unexpected payload status: {0}")]
    UnexpectedPayloadStatus(PayloadStatusEnum),
    /// The get payload call to the engine api failed.
    #[error(transparent)]
    GetPayloadFailed(RpcError<TransportErrorKind>),
    /// The new payload call to the engine api failed.
    #[error(transparent)]
    NewPayloadFailed(RpcError<TransportErrorKind>),
    /// The forkchoice state is invalid.
    #[error("Invalid forkchoice state")]
    InvalidForkchoiceState,
    /// The finalized head is behind the unsafe head.
    #[error("Invalid forkchoice state: unsafe head {0} is ahead of finalized head {1}")]
    FinalizedAheadOfUnsafe(u64, u64),
    /// A deposit-only payload failed to import.
    #[error("Deposit-only payload failed to import")]
    DepositOnlyPayloadFailed,
}

impl From<BuildTaskError> for EngineTaskError {
    fn from(value: BuildTaskError) -> Self {
        match value {
            BuildTaskError::NoForkchoiceUpdateNeeded => Self::Temporary(Box::new(value)),
            BuildTaskError::EngineSyncing => Self::Temporary(Box::new(value)),
            BuildTaskError::ForkchoiceUpdateFailed(_) => Self::Temporary(Box::new(value)),
            BuildTaskError::MissingPayloadId => Self::Temporary(Box::new(value)),
            BuildTaskError::UnexpectedPayloadStatus(_) => Self::Temporary(Box::new(value)),
            BuildTaskError::GetPayloadFailed(_) => Self::Temporary(Box::new(value)),
            BuildTaskError::NewPayloadFailed(_) => Self::Temporary(Box::new(value)),
            BuildTaskError::InvalidForkchoiceState => Self::Reset(Box::new(value)),
            BuildTaskError::FinalizedAheadOfUnsafe(_, _) => Self::Critical(Box::new(value)),
            BuildTaskError::DepositOnlyPayloadFailed => Self::Critical(Box::new(value)),
        }
    }
}
