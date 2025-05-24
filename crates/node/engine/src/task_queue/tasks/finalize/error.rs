//! Contains error types for the [crate::FinalizeTask].

use crate::EngineTaskError;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_protocol::FromBlockError;
use thiserror::Error;

/// An error that occurs when running the [crate::FinalizeTask].
#[derive(Debug, Error)]
pub enum FinalizeTaskError {
    /// The block is not safe, and therefore cannot be finalized.
    #[error("Attempted to finalize a block that is not yet safe")]
    BlockNotSafe,
    /// The block to finalize was not found.
    #[error("The block to finalize was not found: Number {0}")]
    BlockNotFound(u64),
    /// An error occurred while transforming the RPC block into [`L2BlockInfo`].
    ///
    /// [`L2BlockInfo`]: kona_protocol::L2BlockInfo
    #[error(transparent)]
    FromBlock(#[from] FromBlockError),
    /// A temporary RPC failure.
    #[error(transparent)]
    TransportError(#[from] RpcError<TransportErrorKind>),
}

impl From<FinalizeTaskError> for EngineTaskError {
    fn from(value: FinalizeTaskError) -> Self {
        match value {
            FinalizeTaskError::BlockNotSafe => Self::Critical(Box::new(value)),
            FinalizeTaskError::BlockNotFound(_) => Self::Critical(Box::new(value)),
            FinalizeTaskError::FromBlock(_) => Self::Critical(Box::new(value)),
            FinalizeTaskError::TransportError(_) => Self::Temporary(Box::new(value)),
        }
    }
}
