//! Provider for derivation-related database operations.

use crate::{
  error::StorageError, 
  models::{DerivedBlocks, StoredDerivedBlockPair, SourceToDerivedBlockNumbers, U64List},
};
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use reth_db_api::{
  cursor::{DbCursorRO, DbDupCursorRO, DbDupCursorRW},
  transaction::{DbTx, DbTxMut}
};
use tracing::{error, warn};

/// Provides access to derivation storage operations within a transaction.
#[derive(Debug)]
#[expect(dead_code)]
pub(crate) struct DerivationProvider<'tx, TX> {
    tx: &'tx TX,
}

impl<'tx, TX> DerivationProvider<'tx, TX> {
    /// Creates a new [`DerivationProvider`] instance.
    #[expect(dead_code)]
    pub(crate) const fn new(tx: &'tx TX) -> Self {
        Self { tx }
    }
}

impl<TX> DerivationProvider<'_, TX>
where
    TX: DbTx,
{
    /// Helper to get [`StoredDerivedBlockPair`] by block number.
    #[expect(dead_code)]
    fn get_derived_block_pair_by_number(
        &self,
        derived_block_number: u64,
    ) -> Result<StoredDerivedBlockPair, StorageError> {
        let derived_block_pair_opt =
            self.tx.get::<DerivedBlocks>(derived_block_number).map_err(|e| {
                error!(
                  target: "supervisor_storage",
                  derived_block_number = derived_block_number,
                  "Failed to get derived block pair: {e:?}"
                );
                StorageError::Database(e)
            })?;

        let derived_block_pair = derived_block_pair_opt.ok_or_else(|| {
            warn!(
              target: "supervisor_storage",
              derived_block_number = ?derived_block_number,
              "Derived block not found"
            );
            StorageError::EntryNotFound("derived block not found".to_string())
        })?;

        Ok(derived_block_pair)
    }

    /// Helper to get [`StoredDerivedBlockPair`] by derived [`BlockNumHash`].
    /// This function checks if the derived block hash matches the expected hash.
    /// If there is a mismatch, it logs a warning and returns [`StorageError::EntryNotFound`] error.
    #[expect(dead_code)]
    fn get_derived_block_pair(
        &self,
        derived_block_id: BlockNumHash,
    ) -> Result<StoredDerivedBlockPair, StorageError> {
        let derived_block_pair = self.get_derived_block_pair_by_number(derived_block_id.number)?;

        if derived_block_pair.derived.hash != derived_block_id.hash {
            warn!(
              target: "supervisor_storage",
              derived_block_id = ?derived_block_id,
              expected_hash = ?derived_block_id.hash,
              actual_hash = ?derived_block_pair.derived.hash,
              "Derived block hash mismatch"
            );
            return Err(StorageError::EntryNotFound(
                "derived block hash does not match".to_string(),
            ));
        }

        Ok(derived_block_pair)
    }

    /// Gets the source [`BlockInfo`] for the given derived [`BlockNumHash`].
    #[expect(dead_code)]
    pub(crate) fn derived_to_source(
        &self,
        derived_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        let derived_block_pair: StoredDerivedBlockPair =
            self.get_derived_block_pair(derived_block_id)?;
        Ok(derived_block_pair.source.into())
    }

    #[expect(dead_code)]
    fn derived_block_numbers_at_source(
        &self,
        source_block_number: u64,
    ) -> Result<U64List, StorageError> {
        let derived_block_numbers = self
            .tx
            .get::<SourceToDerivedBlockNumbers>(source_block_number)
            .map_err(|e| {
                error!(
                    target: "supervisor_storage",
                    source_block_number,
                    "Failed to get source to derived block numbers: {e:?}"
                );
                StorageError::Database(e)
            })?;
        
        let derived_block_numbers = derived_block_numbers.ok_or_else(|| {
            warn!(
              target: "supervisor_storage",
              source_block_number,
              "source block not found"
            );
            StorageError::EntryNotFound("source block not found".to_string())
        })?;
        Ok(derived_block_numbers)
    }

    /// Gets the latest derived [`BlockInfo`] from the given source [`BlockNumHash`].
    #[expect(dead_code)]
    pub(crate) fn latest_derived_block_at_source(
        &self,
        source_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        let derived_block_numbers = self.derived_block_numbers_at_source(source_block_id.number)?;
        let derived_block_number = derived_block_numbers.last().ok_or_else(|| {
          // note:: this should not happen. the list should always have at least one element
          // todo: add test cases in insert to make sure this is not possible
          error!(
            target: "supervisor_storage",
            source_block_id = ?source_block_id,
            "source to derived block numbers list is empty"
          );
          StorageError::EntryNotFound("No derived blocks found for source block".to_string())
        })?;

        let derived_block_pair = self.get_derived_block_pair_by_number(*derived_block_number)?;
        
        // Check if the source block hash matches the expected hash
        // This is necessary to ensure the integrity of the derived block.
        if derived_block_pair.source.hash != source_block_id.hash {
            warn!(
              target: "supervisor_storage",
              source_block_id = ?source_block_id,
              expected_hash = ?source_block_id.hash,
              actual_hash = ?derived_block_pair.source.hash,
              "Source block hash mismatch"
            );
            return Err(StorageError::EntryNotFound("source block hash does not match".to_string()));
        }

        Ok(derived_block_pair.derived.into())
    }
    
    /// Gets the latest [`DerivedRefPair`].
    #[expect(dead_code)]
    pub(crate) fn latest_derived_block(
        &self,
    ) -> Result<DerivedRefPair, StorageError> {
        let mut cursor = self.tx.cursor_read::<DerivedBlocks>().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to get cursor for DerivedBlocks: {e}");
            StorageError::Database(e)
        })?;

        let result = cursor.last().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to seek to last block: {e}");
            StorageError::Database(e)
        })?;

        let (_, block) = result.ok_or_else(|| {
            error!(target: "supervisor_storage", "No blocks found in storage");
            StorageError::EntryNotFound("No blocks found".to_string())
        })?;
        Ok(block.into())
    }
}

impl<TX> DerivationProvider<'_, TX>
where
    TX: DbTxMut + DbTx,
{
    /// Saves a [`StoredDerivedBlockPair`] to [`DerivedBlocks`](`crate::models::DerivedBlocks`) table
    /// and [`U64List`] to [`SourceToDerivedBlockNumbers`](`SourceToDerivedBlockNumbers`) table
    /// in the database.
    #[expect(dead_code)]
    pub(crate) fn save_derived_block_pair(
        &self,
        incoming_pair: DerivedRefPair,
    ) -> Result<(), StorageError> {
        let latest_block_pair = self.latest_derived_block()?;
        
        // Validate if the latest derived block is parent of the incoming derived block
        // todo: refactor this
        if latest_block_pair.derived.number + 1 != incoming_pair.derived.number && 
            latest_block_pair.derived.hash != incoming_pair.derived.parent_hash {
            warn!(
                target: "supervisor_storage",
                latest_derived_block_pair = ?latest_block_pair,
                incoming_derived_block_pair = ?incoming_pair,
                "Latest stored derived block is not parent of the incoming derived block"
            );
            return Err(StorageError::ConflictError(
                "Latest stored derived block is not parent of the incoming derived block".to_string(),
            ));
        }

        let mut derived_block_numbers = match self.derived_block_numbers_at_source(incoming_pair.source.number) {
            Ok(list) => list,
            Err(StorageError::EntryNotFound(_)) => U64List::default(),
            Err(e) => {
              error!(
                target: "supervisor_storage",
                 incoming_derived_block_pair = ?incoming_pair,
                "Failed to get derived block numbers for source block: {e:?}"
              );
              return Err(e);
            }
        };

        // Add the derived block number to the list
        derived_block_numbers.push(incoming_pair.derived.number);
        
        // Save the derived block pair to the database
        self.tx
            .put::<DerivedBlocks>(incoming_pair.derived.number, incoming_pair.clone().into())
            .map_err(|e| {
                error!(
                    target: "supervisor_storage",
                    incoming_derived_block_pair = ?incoming_pair,
                    "Failed to save derived block pair: {e:?}"
                );
                StorageError::Database(e)
            })?;
        
        // Save the derived block numbers to the database
        self.tx
            .put::<SourceToDerivedBlockNumbers>(incoming_pair.source.number, derived_block_numbers)
            .map_err(|e| {
                error!(
                    target: "supervisor_storage",
                    incoming_derived_block_pair = ?incoming_pair,
                    "Failed to save derived block numbers for source block: {e:?}"
                );
                StorageError::Database(e)
            })?;
        Ok(())
    }
}
