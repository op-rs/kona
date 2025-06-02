use crate::syncnode::ManagedNodeError;
use thiserror::Error;

/// Errors that may occur while processing chains in the supervisor core.
#[derive(Debug, Error)]
pub enum ChainProcessorError {
    /// Represents an error that occurred while interacting with the managed node.
    #[error(transparent)]
    ManagedNode(#[from] ManagedNodeError),

    /// Represents chain mismatch that occurred while adding a node to the chain processor.
    #[error("chain id mismatch")]
    InvalidChainId,
}
