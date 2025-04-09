//! Contains error types for the [`crate::ConsolidateTask`].

use crate::EngineTaskError;
use thiserror::Error;

/// An error that occurs when running the [`crate::ConsolidateTask`].
#[derive(Debug, Error)]
pub enum ConsolidateTaskError {
    /// The unsafe L2 block is missing.
    #[error("Unsafe L2 block is missing {0}")]
    MissingUnsafeL2Block(u64),
    /// Failed to fetch the unsafe L2 block.
    #[error("Failed to fetch the unsafe L2 block")]
    FailedToFetchUnsafeL2Block,
}

impl From<ConsolidateTaskError> for EngineTaskError {
    fn from(value: ConsolidateTaskError) -> Self {
        match value {
            ConsolidateTaskError::MissingUnsafeL2Block(_) => Self::Temporary(Box::new(value)),
            ConsolidateTaskError::FailedToFetchUnsafeL2Block => Self::Temporary(Box::new(value)),
        }
    }
}
