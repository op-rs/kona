use alloy_primitives::ChainId;
use kona_supervisor_storage::StorageError;
use op_alloy_consensus::interop::SafetyLevel;
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

    /// No candidate block is currently available for promotion.
    #[error("no candidate block found to promote")]
    NoBlockToPromote,

    /// The requested safety level is not supported for promotion.
    #[error("promotion to level {0} is not supported")]
    UnsupportedTargetLevel(SafetyLevel),
}
