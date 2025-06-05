//! Provider for derivation-related database operations.

use crate::{
    error::StorageError,
    models::{DerivedBlocks, SourceToDerivedBlockNumbers, StoredDerivedBlockPair, U64List},
};
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use reth_db_api::{
    cursor::DbCursorRO,
    transaction::{DbTx, DbTxMut},
};
use tracing::{error, warn};

/// Provides access to derivation storage operations within a transaction.
#[derive(Debug)]
pub(crate) struct DerivationProvider<'tx, TX> {
    tx: &'tx TX,
}

impl<'tx, TX> DerivationProvider<'tx, TX> {
    /// Creates a new [`DerivationProvider`] instance.
    pub(crate) const fn new(tx: &'tx TX) -> Self {
        Self { tx }
    }
}

impl<TX> DerivationProvider<'_, TX>
where
    TX: DbTx,
{
    /// Helper to get [`StoredDerivedBlockPair`] by block number.
    fn get_derived_block_pair_by_number(
        &self,
        derived_block_number: u64,
    ) -> Result<StoredDerivedBlockPair, StorageError> {
        let derived_block_pair_opt =
            self.tx.get::<DerivedBlocks>(derived_block_number).inspect_err(|err| {
                error!(
                  target: "supervisor_storage",
                  derived_block_number,
                  %err,
                  "Failed to get derived block pair"
                );
            })?;

        let derived_block_pair = derived_block_pair_opt.ok_or_else(|| {
            warn!(
              target: "supervisor_storage",
              derived_block_number,
              "Derived block not found"
            );
            StorageError::EntryNotFound("derived block not found".to_string())
        })?;

        Ok(derived_block_pair)
    }

    /// Helper to get [`StoredDerivedBlockPair`] by derived [`BlockNumHash`].
    /// This function checks if the derived block hash matches the expected hash.
    /// If there is a mismatch, it logs a warning and returns [`StorageError::EntryNotFound`] error.
    fn get_derived_block_pair(
        &self,
        derived_block_id: BlockNumHash,
    ) -> Result<StoredDerivedBlockPair, StorageError> {
        let derived_block_pair = self.get_derived_block_pair_by_number(derived_block_id.number)?;

        if derived_block_pair.derived.hash != derived_block_id.hash {
            warn!(
              target: "supervisor_storage",
              derived_block_number = derived_block_id.number,
              expected_hash = %derived_block_id.hash,
              actual_hash = %derived_block_pair.derived.hash,
              "Derived block hash mismatch"
            );
            return Err(StorageError::EntryNotFound(
                "derived block hash does not match".to_string(),
            ));
        }

        Ok(derived_block_pair)
    }

    /// Gets the source [`BlockInfo`] for the given derived [`BlockNumHash`].
    pub(crate) fn derived_to_source(
        &self,
        derived_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        let derived_block_pair: StoredDerivedBlockPair =
            self.get_derived_block_pair(derived_block_id)?;
        Ok(derived_block_pair.source.into())
    }

    fn derived_block_numbers_at_source(
        &self,
        source_block_number: u64,
    ) -> Result<U64List, StorageError> {
        let derived_block_numbers =
            self.tx.get::<SourceToDerivedBlockNumbers>(source_block_number).inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    source_block_number,
                    %err,
                    "Failed to get source to derived block numbers"
                );
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
    pub(crate) fn latest_derived_block_at_source(
        &self,
        source_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        let derived_block_numbers = self.derived_block_numbers_at_source(source_block_id.number)?;
        let derived_block_number = derived_block_numbers.last().ok_or_else(|| {
            // note:: this should not happen. the list should always have at least one element
            // todo:: make sure unwinding removes the entry properly
            error!(
              target: "supervisor_storage",
              source_block_number = source_block_id.number,
              "source to derived block numbers list is empty"
            );
            StorageError::EntryNotFound("no derived blocks found for source block".to_string())
        })?;

        let derived_block_pair = self.get_derived_block_pair_by_number(*derived_block_number)?;

        // Check if the source block hash matches the expected hash
        // This is necessary to ensure the integrity of the derived block.
        if derived_block_pair.source.hash != source_block_id.hash {
            warn!(
              target: "supervisor_storage",
              source_block_number = source_block_id.number,
              expected_hash = %source_block_id.hash,
              actual_hash = %derived_block_pair.source.hash,
              "Source block hash mismatch"
            );
            return Err(StorageError::EntryNotFound("source block hash does not match".to_string()));
        }

        Ok(derived_block_pair.derived.into())
    }

    /// Gets the latest [`DerivedRefPair`].
    pub(crate) fn latest_derived_block_pair(&self) -> Result<DerivedRefPair, StorageError> {
        let mut cursor = self.tx.cursor_read::<DerivedBlocks>().inspect_err(|err| {
            error!(target: "supervisor_storage",
              %err,
              "Failed to get cursor for DerivedBlocks"
            );
        })?;

        let result = cursor.last().inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                %err,
                "Failed to seek to last block"
            );
        })?;

        let (_, block) = result.ok_or_else(|| {
            error!(target: "supervisor_storage", "No blocks found in storage");
            StorageError::EntryNotFound("no blocks found".to_string())
        })?;
        Ok(block.into())
    }
}

impl<TX> DerivationProvider<'_, TX>
where
    TX: DbTxMut + DbTx,
{
    /// initialises the database with a derived block pair anchor.
    pub(crate) fn initialise(&self, anchor: DerivedRefPair) -> Result<(), StorageError> {
        match self.get_derived_block_pair_by_number(0) {
            Ok(pair)
                if pair.derived.hash == anchor.derived.hash &&
                    pair.source.hash == anchor.source.hash =>
            {
                // Anchor matches, nothing to do
                Ok(())
            }
            Ok(_) => Err(StorageError::InvalidAnchor),
            Err(StorageError::EntryNotFound(_)) => self.save_derived_block_pair_internal(anchor),
            Err(err) => Err(err),
        }
    }

    /// Saves a [`StoredDerivedBlockPair`] to [`DerivedBlocks`](`crate::models::DerivedBlocks`)
    /// table and [`U64List`] to [`SourceToDerivedBlockNumbers`](`SourceToDerivedBlockNumbers`)
    /// table in the database.
    pub(crate) fn save_derived_block_pair(
        &self,
        incoming_pair: DerivedRefPair,
    ) -> Result<(), StorageError> {
        // todo: use cursor to get the last block(performance improvement)
        let latest_block_pair = match self.latest_derived_block_pair() {
            Ok(pair) => pair,
            Err(StorageError::EntryNotFound(_)) => return Err(StorageError::DatabaseNotInitialised),
            Err(e) => return Err(e),
        };

        if !latest_block_pair.derived.is_parent_of(&incoming_pair.derived) {
            warn!(
              target: "supervisor_storage",
              latest_derived_block_pair = %latest_block_pair,
              incoming_derived_block_pair = %incoming_pair,
              "Latest stored derived block is not parent of the incoming derived block"
            );
            return Err(StorageError::DerivedBlockOutOfOrder);
        }
        // todo: analyze if we should check if the incoming derived block is the first block
        // or let service handle it

        self.save_derived_block_pair_internal(incoming_pair)
    }

    /// Internal function to save a derived block pair.
    /// This function does not perform any checks on the incoming pair,
    /// it assumes that the pair is valid and the latest derived block is its parent.
    fn save_derived_block_pair_internal(
        &self,
        incoming_pair: DerivedRefPair,
    ) -> Result<(), StorageError> {
        let mut derived_block_numbers =
            match self.derived_block_numbers_at_source(incoming_pair.source.number) {
                Ok(list) => list,
                Err(StorageError::EntryNotFound(_)) => U64List::default(),
                Err(err) => {
                    error!(
                      target: "supervisor_storage",
                      incoming_derived_block_pair = %incoming_pair,
                      %err,
                      "Failed to get derived block numbers for source block"
                    );
                    return Err(err);
                }
            };

        // Add the derived block number to the list
        derived_block_numbers.push(incoming_pair.derived.number);

        // Save the derived block pair to the database
        self.tx
            .put::<DerivedBlocks>(incoming_pair.derived.number, incoming_pair.clone().into())
            .inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    incoming_derived_block_pair = %incoming_pair,
                    %err,
                    "Failed to save derived block pair"
                );
            })?;

        // Save the derived block numbers to the database
        self.tx
            .put::<SourceToDerivedBlockNumbers>(incoming_pair.source.number, derived_block_numbers)
            .inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    incoming_derived_block_pair = %incoming_pair,
                    %err,
                    "Failed to save derived block numbers for source block"
                );
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Tables;
    use alloy_primitives::B256;
    use kona_interop::DerivedRefPair;
    use kona_protocol::BlockInfo;
    use reth_db::{
        Database, DatabaseEnv,
        mdbx::{DatabaseArguments, init_db_for},
    };
    use tempfile::TempDir;

    fn block_info(number: u64, parent_hash: B256, timestamp: u64) -> BlockInfo {
        BlockInfo { hash: B256::from([number as u8; 32]), number, parent_hash, timestamp }
    }

    const fn derived_pair(source: BlockInfo, derived: BlockInfo) -> DerivedRefPair {
        DerivedRefPair { source, derived }
    }

    fn genesis_block() -> BlockInfo {
        BlockInfo {
            hash: B256::from([0u8; 32]),
            number: 0,
            parent_hash: B256::ZERO,
            timestamp: 100,
        }
    }

    /// Sets up a new temp DB
    fn setup_db() -> DatabaseEnv {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        init_db_for::<_, Tables>(temp_dir.path(), DatabaseArguments::default())
            .expect("Failed to init database")
    }

    /// Helper to initialize database in a new transaction, committing if successful.
    fn initialize_db(db: &DatabaseEnv, pair: &DerivedRefPair) -> Result<(), StorageError> {
        let tx = db.tx_mut().expect("Could not get mutable tx");
        let provider = DerivationProvider::new(&tx);
        let res = provider.initialise(pair.clone());
        if res.is_ok() {
            tx.commit().expect("Failed to commit transaction");
        }
        res
    }

    /// Helper to insert a pair in a new transaction, committing if successful.
    fn insert_pair(db: &DatabaseEnv, pair: &DerivedRefPair) -> Result<(), StorageError> {
        let tx = db.tx_mut().expect("Could not get mutable tx");
        let provider = DerivationProvider::new(&tx);
        let res = provider.save_derived_block_pair(pair.clone());
        if res.is_ok() {
            tx.commit().expect("Failed to commit transaction");
        }
        res
    }

    #[test]
    fn initialise_inserts_anchor_if_not_exists() {
        let db = setup_db();

        let source = block_info(100, B256::from([100u8; 32]), 200);
        let derived = block_info(0, genesis_block().hash, 200);
        let anchor = derived_pair(source, derived);

        // Should succeed and insert the anchor
        assert!(initialize_db(&db, &anchor).is_ok());

        // Check that the anchor is present
        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);
        let stored = provider.get_derived_block_pair_by_number(0).expect("should exist");
        assert_eq!(stored.source.hash, anchor.source.hash);
        assert_eq!(stored.derived.hash, anchor.derived.hash);
    }

    #[test]
    fn initialise_is_idempotent_if_anchor_matches() {
        let db = setup_db();

        let source = block_info(100, B256::from([100u8; 32]), 200);
        let anchor = derived_pair(source, genesis_block());

        // First initialise
        assert!(initialize_db(&db, &anchor).is_ok());
        // Second initialise with the same anchor should succeed (idempotent)
        assert!(initialize_db(&db, &anchor).is_ok());
    }

    #[test]
    fn initialise_fails_if_anchor_mismatch() {
        let db = setup_db();

        let source = block_info(100, B256::from([100u8; 32]), 200);
        let anchor = derived_pair(source, genesis_block());

        // Insert the genesis
        assert!(initialize_db(&db, &anchor).is_ok());

        // Try to initialise with a different anchor (different hash)
        let wrong_derived = block_info(1, B256::from([42u8; 32]), 200);
        let wrong_anchor = derived_pair(source, wrong_derived);

        let result = initialize_db(&db, &wrong_anchor);
        assert!(matches!(result, Err(StorageError::InvalidAnchor)));
    }

    #[test]
    fn save_derived_block_pair_positive() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let derived2 = block_info(2, derived1.hash, 300);
        let pair2 = derived_pair(source1, derived2);
        assert!(insert_pair(&db, &pair2).is_ok());

        let source3 = block_info(200, B256::from([200u8; 32]), 400);
        let derived3 = block_info(3, derived2.hash, 400);
        let pair3 = derived_pair(source3, derived3);
        assert!(insert_pair(&db, &pair3).is_ok());
    }

    #[test]
    fn save_derived_block_pair_wrong_parent_should_fail() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let wrong_parent_hash = B256::from([99u8; 32]);
        let derived2 = block_info(2, wrong_parent_hash, 300);
        let pair2 = derived_pair(source1, derived2);
        let result = insert_pair(&db, &pair2);
        assert!(matches!(result, Err(StorageError::DerivedBlockOutOfOrder)));
    }

    #[test]
    fn save_derived_block_pair_gap_in_number_should_fail() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let derived2 = block_info(4, derived1.hash, 400); // should be 2, not 4
        let pair2 = derived_pair(source1, derived2);
        let result = insert_pair(&db, &pair2);
        assert!(matches!(result, Err(StorageError::DerivedBlockOutOfOrder)));
    }

    #[test]
    fn duplicate_derived_block_number_should_fail() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        // Try to insert the same derived block again
        let result = insert_pair(&db, &pair1);
        assert!(matches!(result, Err(StorageError::DerivedBlockOutOfOrder)));
    }

    #[test]
    fn non_monotonic_l2_number_should_fail() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let derived2 = block_info(2, derived1.hash, 300);
        let pair2 = derived_pair(source1, derived2);
        assert!(insert_pair(&db, &pair2).is_ok());

        // Try to insert a block with a lower number than the latest
        let derived_non_monotonic = block_info(1, derived2.hash, 400);
        let pair_non_monotonic = derived_pair(source1, derived_non_monotonic);
        let result = insert_pair(&db, &pair_non_monotonic);
        assert!(matches!(result, Err(StorageError::DerivedBlockOutOfOrder)));
    }

    #[test]
    fn test_latest_derived_block_at_source_returns_latest() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let derived2 = block_info(2, derived1.hash, 300);
        let pair2 = derived_pair(source1, derived2);
        assert!(insert_pair(&db, &pair2).is_ok());

        let source2 = block_info(105, B256::from([100u8; 32]), 300);
        let derived3 = block_info(3, derived2.hash, 400);
        let pair3 = derived_pair(source2, derived3);
        assert!(insert_pair(&db, &pair3).is_ok());

        // Now check latest_derived_block_at_source returns derived2 for source1
        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);
        let source_id1 = BlockNumHash { number: source1.number, hash: source1.hash };
        let latest = provider.latest_derived_block_at_source(source_id1).expect("should exist");
        assert_eq!(latest.number, derived2.number);
        assert_eq!(latest.hash, derived2.hash);

        // Now check latest_derived_block_at_source returns derived3 for source2
        let source_id2 = BlockNumHash { number: source2.number, hash: source2.hash };
        let latest = provider.latest_derived_block_at_source(source_id2).expect("should exist");
        assert_eq!(latest, derived3);
    }

    #[test]
    fn test_latest_derived_block_at_source_empty_list_returns_error() {
        let db = setup_db();

        // Use a source block that does not exist
        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);
        let source_id = BlockNumHash { number: 9999, hash: B256::from([99u8; 32]) };
        let result = provider.latest_derived_block_at_source(source_id);
        assert!(matches!(result, Err(StorageError::EntryNotFound(_))));
    }

    #[test]
    fn test_latest_derived_block_at_source_hash_mismatch_returns_error() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        // Use correct number but wrong hash
        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);
        let wrong_hash = B256::from([123u8; 32]);
        let source_id = BlockNumHash { number: source1.number, hash: wrong_hash };
        let result = provider.latest_derived_block_at_source(source_id);
        assert!(matches!(result, Err(StorageError::EntryNotFound(_))));
    }

    #[test]
    fn test_latest_derived_block_pair_returns_latest() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let derived2 = block_info(2, derived1.hash, 300);
        let pair2 = derived_pair(source1, derived2);
        assert!(insert_pair(&db, &pair2).is_ok());

        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);

        let latest = provider.latest_derived_block_pair().expect("should exist");
        assert_eq!(latest, pair2);
    }

    #[test]
    fn test_latest_derived_block_pair_empty_returns_error() {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        let db = init_db_for::<_, Tables>(temp_dir.path(), DatabaseArguments::default())
            .expect("Failed to init database");

        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);
        assert!(matches!(
            provider.latest_derived_block_pair(),
            Err(StorageError::EntryNotFound(_))
        ));
    }

    #[test]
    fn test_derived_to_source_returns_correct_source() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);

        let derived_block_id = BlockNumHash { number: derived1.number, hash: derived1.hash };
        let source = provider.derived_to_source(derived_block_id).expect("should exist");
        assert_eq!(source, source1);
    }

    #[test]
    fn test_derived_to_source_not_found_returns_error() {
        let db = setup_db();

        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);

        let derived_block_id = BlockNumHash { number: 9999, hash: B256::from([9u8; 32]) };
        let result = provider.derived_to_source(derived_block_id);
        assert!(matches!(result, Err(StorageError::EntryNotFound(_))));
    }
}
