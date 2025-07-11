//! Contains error types for the [crate::ForkchoiceTask].

use crate::{
    EngineTaskError, ForkchoiceTaskError, InsertTaskError,
    task_queue::tasks::task::EngineTaskErrorSeverity,
};
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
    /// The initial forkchoice update call to the engine api failed.
    #[error(transparent)]
    ForkchoiceUpdateFailed(#[from] ForkchoiceTaskError),
    /// Impossible to insert the payload into the engine.
    #[error(transparent)]
    PayloadInsertionFailed(#[from] InsertTaskError),
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

impl EngineTaskError for BuildTaskError {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::ForkchoiceUpdateFailed(inner) => inner.severity(),
            Self::PayloadInsertionFailed(inner) => inner.severity(),
            Self::NoForkchoiceUpdateNeeded => EngineTaskErrorSeverity::Temporary,
            Self::EngineSyncing => EngineTaskErrorSeverity::Temporary,
            Self::GetPayloadFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::NewPayloadFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::HoloceneInvalidFlush => EngineTaskErrorSeverity::Flush,
            Self::MissingPayloadId => EngineTaskErrorSeverity::Critical,
            Self::UnexpectedPayloadStatus(_) => EngineTaskErrorSeverity::Critical,
            Self::DepositOnlyPayloadReattemptFailed => EngineTaskErrorSeverity::Critical,
            Self::DepositOnlyPayloadFailed => EngineTaskErrorSeverity::Critical,
            Self::FromBlock(_) => EngineTaskErrorSeverity::Critical,
            Self::MpscSend(_) => EngineTaskErrorSeverity::Critical,
        }
    }
}
