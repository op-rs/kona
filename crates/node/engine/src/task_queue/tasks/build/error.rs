//! Contains error types for the [crate::SynchronizeTask].

use crate::{
    EngineTaskError, InsertTaskError, SynchronizeTaskError,
    task_queue::tasks::task::EngineTaskErrorSeverity,
};
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_protocol::FromBlockError;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use thiserror::Error;
use tokio::sync::mpsc;

/// An error that occurs during payload building within the engine.
///
/// This error type is specific to the block building process and represents failures
/// that can occur during the automatic forkchoice update phase of [`BuildTask`].
/// Unlike [`BuildTaskError`], which handles higher-level build orchestration errors,
/// `EngineBuildError` focuses on low-level engine API communication failures.
///
/// ## Error Categories
///
/// - **State Validation**: Errors related to inconsistent chain state
/// - **Engine Communication**: RPC failures during forkchoice updates
/// - **Payload Validation**: Invalid payload status responses from the execution layer
///
/// [`BuildTask`]: crate::BuildTask
#[derive(Debug, Error)]
pub enum EngineBuildError {
    /// The finalized head is ahead of the unsafe head.
    #[error("Finalized head is ahead of unsafe head")]
    FinalizedAheadOfUnsafe(u64, u64),
    /// The forkchoice update call to the engine api failed.
    #[error("Failed to build payload attributes in the engine. Forkchoice RPC error: {0}")]
    AttributesInsertionFailed(#[from] RpcError<TransportErrorKind>),
    /// The inserted payload is invalid.
    #[error("The inserted payload is invalid: {0}")]
    InvalidPayload(String),
    /// The inserted payload status is unexpected.
    #[error("The inserted payload status is unexpected: {0}")]
    UnexpectedPayloadStatus(PayloadStatusEnum),
    /// The payload ID is missing.
    #[error("The inserted payload ID is missing")]
    MissingPayloadId,
    /// The engine is syncing.
    #[error("The engine is syncing")]
    EngineSyncing,
}

/// An error that occurs when running the [crate::SynchronizeTask].
#[derive(Debug, Error)]
pub enum BuildTaskError {
    /// An error occurred when building the payload attributes in the engine.
    #[error("An error occurred when building the payload attributes to the engine.")]
    EngineBuildError(EngineBuildError),
    /// The initial forkchoice update call to the engine api failed.
    #[error(transparent)]
    ForkchoiceUpdateFailed(#[from] SynchronizeTaskError),
    /// Impossible to insert the payload into the engine.
    #[error(transparent)]
    PayloadInsertionFailed(#[from] InsertTaskError),
    /// The get payload call to the engine api failed.
    #[error(transparent)]
    GetPayloadFailed(RpcError<TransportErrorKind>),
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
            Self::EngineBuildError(EngineBuildError::FinalizedAheadOfUnsafe(_, _)) => {
                EngineTaskErrorSeverity::Critical
            }
            Self::EngineBuildError(EngineBuildError::AttributesInsertionFailed(_)) => {
                EngineTaskErrorSeverity::Temporary
            }
            Self::EngineBuildError(EngineBuildError::InvalidPayload(_)) => {
                EngineTaskErrorSeverity::Temporary
            }
            Self::EngineBuildError(EngineBuildError::UnexpectedPayloadStatus(_)) => {
                EngineTaskErrorSeverity::Temporary
            }
            Self::EngineBuildError(EngineBuildError::MissingPayloadId) => {
                EngineTaskErrorSeverity::Temporary
            }
            Self::EngineBuildError(EngineBuildError::EngineSyncing) => {
                EngineTaskErrorSeverity::Temporary
            }
            Self::GetPayloadFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::HoloceneInvalidFlush => EngineTaskErrorSeverity::Flush,
            Self::DepositOnlyPayloadReattemptFailed => EngineTaskErrorSeverity::Critical,
            Self::DepositOnlyPayloadFailed => EngineTaskErrorSeverity::Critical,
            Self::FromBlock(_) => EngineTaskErrorSeverity::Critical,
            Self::MpscSend(_) => EngineTaskErrorSeverity::Critical,
        }
    }
}
