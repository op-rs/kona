//! Contains error types for the [crate::SynchronizeTask].

use crate::{EngineTaskError, InsertTaskError, task_queue::tasks::task::EngineTaskErrorSeverity};
use alloy_transport::{RpcError, TransportErrorKind};
use kona_protocol::FromBlockError;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use thiserror::Error;
use tokio::sync::mpsc;

/// An error that occurs when running the [crate::SealTask].
#[derive(Debug, Error)]
pub enum SealTaskError {
    /// Impossible to insert the payload into the engine.
    #[error(transparent)]
    PayloadInsertionFailed(#[from] Box<InsertTaskError>),
    /// The get payload call to the engine api failed.
    #[error(transparent)]
    GetPayloadFailed(RpcError<TransportErrorKind>),
    /// A deposit-only payload failed to import.
    #[error("Deposit-only payload failed to import")]
    DepositOnlyPayloadFailed,
    /// Failed to re-attempt payload import with deposit-only payload.
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
    MpscSend(
        #[from] Box<mpsc::error::SendError<Result<OpExecutionPayloadEnvelope, SealTaskError>>>,
    ),
    /// The clock went backwards.
    #[error("The clock went backwards")]
    ClockWentBackwards,
}

impl EngineTaskError for SealTaskError {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::PayloadInsertionFailed(inner) => inner.severity(),
            Self::GetPayloadFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::HoloceneInvalidFlush => EngineTaskErrorSeverity::Flush,
            Self::DepositOnlyPayloadReattemptFailed => EngineTaskErrorSeverity::Critical,
            Self::DepositOnlyPayloadFailed => EngineTaskErrorSeverity::Critical,
            Self::FromBlock(_) => EngineTaskErrorSeverity::Critical,
            Self::MpscSend(_) => EngineTaskErrorSeverity::Critical,
            Self::ClockWentBackwards => EngineTaskErrorSeverity::Critical,
        }
    }
}
