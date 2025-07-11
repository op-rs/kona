//! Contains error types for the [crate::ForkchoiceTask].

use crate::EngineTaskError;
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_protocol::FromBlockError;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use thiserror::Error;
use tokio::sync::mpsc;

/// An error that occurs when running the [crate::ForkchoiceTask].
#[derive(Debug, Error)]
pub enum BuildTaskError {
    /// The forkchoice update is not needed.
    #[error("No forkchoice update needed")]
    NoForkchoiceUpdateNeeded,
    /// The engine is syncing.
    #[error("Attempting to update forkchoice state while EL syncing")]
    EngineSyncing,
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
    /// A deposit-only payload failed to import.
    #[error("Deposit-only payload failed to import")]
    DepositOnlyPayloadFailed,
    /// Failed to re-atttempt payload import with deposit-only payload.
    #[error("Failed to re-attempt payload import with deposit-only payload")]
    DepositOnlyPayloadReattemptFailed,
    /// The payload is invalid, and the derivation pipeline must
    /// be flushed post-holocene.
    #[error("Invalid payload, must flush post-holocene")]
    HoloceneInvalidFlush,
    /// Failed to convert a [`OpExecutionPayload`] to a [`L2BlockInfo`].
    ///
    /// [`OpExecutionPayload`]: op_alloy_rpc_types_engine::OpExecutionPayload
    /// [`L2BlockInfo`]: kona_protocol::L2BlockInfo
    #[error(transparent)]
    FromBlock(#[from] FromBlockError),
    /// Error sending the built payload envelope.
    #[error(transparent)]
    MpscSend(#[from] mpsc::error::SendError<OpExecutionPayloadEnvelope>),
}

impl From<BuildTaskError> for EngineTaskError {
    fn from(value: BuildTaskError) -> Self {
        match value {
            BuildTaskError::NoForkchoiceUpdateNeeded => Self::Temporary(Box::new(value)),
            BuildTaskError::EngineSyncing => Self::Temporary(Box::new(value)),
            BuildTaskError::GetPayloadFailed(_) => Self::Temporary(Box::new(value)),
            BuildTaskError::NewPayloadFailed(_) => Self::Temporary(Box::new(value)),
            BuildTaskError::HoloceneInvalidFlush => Self::Flush(Box::new(value)),
            BuildTaskError::MissingPayloadId => Self::Critical(Box::new(value)),
            BuildTaskError::UnexpectedPayloadStatus(_) => Self::Critical(Box::new(value)),
            BuildTaskError::DepositOnlyPayloadReattemptFailed => Self::Critical(Box::new(value)),
            BuildTaskError::DepositOnlyPayloadFailed => Self::Critical(Box::new(value)),
            BuildTaskError::FromBlock(_) => Self::Critical(Box::new(value)),
            BuildTaskError::MpscSend(_) => Self::Critical(Box::new(value)),
        }
    }
}
