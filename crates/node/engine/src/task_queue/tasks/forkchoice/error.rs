//! Contains error types for the [crate::ForkchoiceTask].

use crate::{EngineTaskError, task_queue::tasks::task::EngineTaskErrorSeverity};
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use thiserror::Error;

/// An error that occurs when running the [crate::ForkchoiceTask].
#[derive(Debug, Error)]
pub enum ForkchoiceTaskError {
    /// The forkchoice update is not needed.
    #[error("No forkchoice update needed")]
    NoForkchoiceUpdateNeeded,
    /// The engine is syncing.
    #[error("Attempting to update forkchoice state while EL syncing")]
    EngineSyncing,
    /// The forkchoice update call to the engine api failed.
    #[error("Forkchoice update engine api call failed")]
    ForkchoiceUpdateFailed(RpcError<TransportErrorKind>),
    /// The finalized head is behind the unsafe head.
    #[error("Invalid forkchoice state: unsafe head {0} is ahead of finalized head {1}")]
    FinalizedAheadOfUnsafe(u64, u64),
    /// The forkchoice state is invalid.
    #[error("Invalid forkchoice state")]
    InvalidForkchoiceState,
    /// The payload status is invalid.
    #[error("Invalid payload status: {0}")]
    InvalidPayloadStatus(String),
    /// The payload status is unexpected.
    #[error("Unexpected payload status: {0}")]
    UnexpectedPayloadStatus(PayloadStatusEnum),
}

impl EngineTaskError for ForkchoiceTaskError {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::NoForkchoiceUpdateNeeded => EngineTaskErrorSeverity::Temporary,
            Self::EngineSyncing => EngineTaskErrorSeverity::Temporary,
            Self::ForkchoiceUpdateFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::FinalizedAheadOfUnsafe(_, _) => EngineTaskErrorSeverity::Critical,
            Self::UnexpectedPayloadStatus(_) => EngineTaskErrorSeverity::Critical,
            Self::InvalidForkchoiceState => EngineTaskErrorSeverity::Reset,
            Self::InvalidPayloadStatus(_) => EngineTaskErrorSeverity::Reset,
        }
    }
}
