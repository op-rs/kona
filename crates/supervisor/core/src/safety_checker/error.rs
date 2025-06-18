use alloy_primitives::ChainId;
use kona_supervisor_storage::StorageError;
use thiserror::Error;

/// Errors returned when validating cross-chain message dependencies.
#[derive(Debug, Error)]
pub enum CrossSafetyError {
    /// Indicates a failure while accessing storage during dependency checking.
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    /// The block that a message depends on does not meet the required safety level.
    #[error(
        "dependency on block {block_number} (chain {chain_id}) does not meet required safety level"
    )]
    DependencyNotSafe {
        /// The ID of the chain containing the unsafe dependency.
        chain_id: ChainId,
        /// The block number of the dependency that failed the safety check
        block_number: u64,
    },
}
