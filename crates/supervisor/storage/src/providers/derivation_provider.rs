//! Provider for derivation-related database operations.

use crate::{error::StorageError, models, models::StoredDerivedBlockPair};
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use reth_db_api::transaction::{DbTx, DbTxMut};
use tracing::{error, warn};

/// Provides access to derivation storage operations within a transaction.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct DerivationProvider<'tx, TX> {
    tx: &'tx TX,
}

impl<'tx, TX> DerivationProvider<'tx, TX> {
    /// Creates a new [`DerivationProvider`] instance.
    #[allow(dead_code)]
    pub(crate) const fn new(tx: &'tx TX) -> Self {
        Self { tx }
    }
}

impl<TX> DerivationProvider<'_, TX>
where
    TX: DbTx,
{
    /// Helper to get [`StoredDerivedBlockPair`] by block number.
    #[allow(dead_code)]
    fn get_derived_block_pair_by_number(
        &self,
        derived_block_number: u64,
    ) -> Result<StoredDerivedBlockPair, StorageError> {
        let derived_block_pair_opt =
            self.tx.get::<models::DerivedBlocks>(derived_block_number).map_err(|e| {
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
    /// If there is a mismatch, it logs a warning and returns an error.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub(crate) fn derived_to_source(
        &self,
        derived_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        let derived_block_pair: StoredDerivedBlockPair =
            self.get_derived_block_pair(derived_block_id)?;
        Ok(derived_block_pair.source.into())
    }

    /// Gets the latest derived [`BlockInfo`] from the given source [`BlockNumHash`].
    #[allow(dead_code)]
    pub(crate) fn latest_derived_block_at_source(
        &self,
        source_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        let latest_derived_block_number = self
            .tx
            .get::<models::SourceToDerivedBlockNumbers>(source_block_id.number)
            .map_err(|e| {
                error!(
                    target: "supervisor_storage",
                    source_block_id = ?source_block_id,
                    "Failed to get source to derived block numbers: {e:?}"
                );
                StorageError::Database(e)
            })?
            .and_then(|list| list.last().copied());

        let derived_block_number = latest_derived_block_number.ok_or_else(|| {
            warn!(
              target: "supervisor_storage",
              source_block_id = ?source_block_id,
              "source block not found"
            );
            StorageError::EntryNotFound("source block not found".to_string())
        })?;

        let derived_block_pair = self.get_derived_block_pair_by_number(derived_block_number)?;
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
}

impl<TX> DerivationProvider<'_, TX>
where
    TX: DbTxMut + DbTx,
{
    /// Saves a [`StoredDerivedBlockPair`] to [`DerivedBlocks`](`crate::models::DerivedBlocks`)
    /// and [`U64List`] to
    /// [`SourceToDerivedBlockNumbers`](`crate::models::SourceToDerivedBlockNumbers`)
    /// to the database.
    #[allow(dead_code)]
    pub(crate) fn save_derived_block_pair(
        &self,
        _pair: DerivedRefPair,
    ) -> Result<(), StorageError> {
        // todo: implement it
        Ok(())
    }
}
