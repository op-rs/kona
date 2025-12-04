//! Contains error types for the [crate::SynchronizeTask].

use crate::{
    EngineTaskError, InvalidEngineSyncStateError, task_queue::tasks::task::EngineTaskErrorSeverity,
};
use alloy_rpc_types_engine::PayloadStatusEnum;
use alloy_transport::{RpcError, TransportErrorKind};
use thiserror::Error;

/// An error that occurs when running the [crate::SynchronizeTask].
#[derive(Debug, Error)]
pub enum SynchronizeTaskError {
    /// The forkchoice update call to the engine api failed.
    #[error("Forkchoice update engine api call failed due to an RPC error: {0}")]
    ForkchoiceUpdateFailed(RpcError<TransportErrorKind>),
    /// Invalid sync state configuration.
    #[error(transparent)]
    InvalidSyncState(#[from] Box<InvalidEngineSyncStateError>),
    /// The forkchoice state is invalid.
    #[error("Invalid forkchoice state")]
    InvalidForkchoiceState,
    /// The payload status is unexpected.
    #[error("Unexpected payload status: {0}")]
    UnexpectedPayloadStatus(PayloadStatusEnum),
}

impl EngineTaskError for SynchronizeTaskError {
    fn severity(&self) -> EngineTaskErrorSeverity {
        match self {
            Self::InvalidSyncState(_) => EngineTaskErrorSeverity::Critical,
            Self::ForkchoiceUpdateFailed(_) => EngineTaskErrorSeverity::Temporary,
            Self::UnexpectedPayloadStatus(_) => EngineTaskErrorSeverity::Temporary,
            Self::InvalidForkchoiceState => EngineTaskErrorSeverity::Reset,
        }
    }
}
