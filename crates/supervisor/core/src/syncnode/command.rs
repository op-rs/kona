use alloy_eips::BlockNumHash;
use kona_supervisor_types::BlockSeal;

/// Commands for managing a node in the supervisor.
#[derive(Debug, PartialEq, Eq)]
pub enum ManagedNodeCommand {
    /// Update the finalized block in the managed node.
    UpdateFinalized {
        /// Block ID of the finalized block.
        block_id: BlockNumHash,
    },

    /// Update the cross-unsafe block in the managed node.
    UpdateCrossUnsafe {
        /// Block ID of the cross-unsafe block.
        block_id: BlockNumHash,
    },

    /// Update the cross-safe block in the managed node.
    UpdateCrossSafe {
        /// Source block ID for the cross-safe update.
        source_block_id: BlockNumHash,
        /// Derived block ID for the cross-safe update.
        derived_block_id: BlockNumHash,
    },

    /// Reset the managed node.
    Reset {},

    /// Invalidate a block in the managed node.
    InvalidateBlock {
        /// [`BlockSeal`] of the block to invalidate.
        seal: BlockSeal,
    },
}
