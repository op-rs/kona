use alloy_primitives::ChainId;
use derive_more::Constructor;
use kona_interop::DerivedRefPair;
use kona_supervisor_storage::{LogStorageReader, StorageError, StorageRewinder};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info, warn};

/// Initiates supervisor-level rewinds based on chain events or storage conflicts.
///
/// This coordinates per-chain rewind logic using the underlying [`StorageRewinder`] implementation,
/// and encapsulates the context in which rewinds should occur.
///
/// It is used in response to:
/// - Local derivation conflicts (failure updating supervisor state)
/// - L1-originated reorgs affecting derived state
#[derive(Debug, Constructor)]
pub struct ChainRewinder<DB> {
    chain_id: ChainId,
    db: Arc<DB>,
}

#[expect(dead_code)] // todo: to be removed in the follow up PR
impl<DB> ChainRewinder<DB>
where
    DB: StorageRewinder + LogStorageReader,
{
    /// Handles a local reorg by rewinding supervisor state from the conflicting derived pair.
    ///
    /// This is triggered when an update to supervisor storage fails due to an
    /// integrity violation (e.g., mismatched on storing local safe block hash).
    pub fn handle_local_reorg(&self, derived_pair: &DerivedRefPair) -> Result<(), StorageError> {
        // get the same block from log storage
        let conflicting_block =
            self.db.get_block(derived_pair.derived.number).inspect_err(|err| {
                error!(
                    target: "rewinder",
                    chain = %self.chain_id,
                    block_number = derived_pair.derived.number,
                    %err,
                    "Error retrieving conflicting block for reorg"
                );
            })?;

        // cross-check whether the block is conflicting
        if conflicting_block == derived_pair.derived {
            return Ok(())
        }

        // rewind the log storage to remove all the blocks till the conflicting one
        self.db.rewind_log_storage(&conflicting_block.id()).inspect_err(|err| {
            error!(
                target: "rewinder",
                chain = %self.chain_id,
                block_number = derived_pair.derived.number,
                %err,
                "Error rewinding the log storage"
            );
        })?;

        // todo: sync the log storage - to prevent a reset
        // todo: save the derived_pair - now it should succeed

        info!(
            target: "rewinder",
            chain = self.chain_id,
            "Rewind successful after local derivation conflict"
        );

        Ok(())
    }

    /// Handles a rewind due to an L1 reorg.
    ///
    /// This method is expected to revert supervisor state based on the L1 reorg by finding the new
    /// valid state and removing any derived data that is no longer valid due to upstream
    /// reorganization.
    fn handle_l1_reorg(&self) -> Result<(), StorageError> {
        warn!(
            target: "rewinder",
            chain = self.chain_id,
            "L1 reorg handling is not yet implemented. Skipping rewind."
        );

        Ok(())
    }
}

/// Error type for the [`ChainRewinder`].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ChainRewinderError {
    /// Failed on storage operations
    #[error(transparent)]
    StorageError(#[from] StorageError),
}
