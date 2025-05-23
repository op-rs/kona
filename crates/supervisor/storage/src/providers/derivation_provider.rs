//! Provider for derivation-related database operations.

use crate::{
    error::StorageError,
    models::{DerivedBlocks, SourceToDerivedBlockNumbers, StoredDerivedBlockPair, U64List},
};
use alloy_eips::eip1898::BlockNumHash;
use alloy_primitives::B256;
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
    fn get_derived_block_pair_by_number(
        &self,
        derived_block_number: u64,
    ) -> Result<StoredDerivedBlockPair, StorageError> {
        let derived_block_pair_opt =
            self.tx.get::<DerivedBlocks>(derived_block_number).inspect_err(|err| {
                error!(
                  target: "supervisor_storage",
                  derived_block_number,
                  ?err,
                  "Failed to get derived block pair"
                );
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

    fn derived_block_numbers_at_source(
        &self,
        source_block_number: u64,
    ) -> Result<U64List, StorageError> {
        let derived_block_numbers =
            self.tx.get::<SourceToDerivedBlockNumbers>(source_block_number).inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    source_block_number,
                    ?err,
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
    pub(crate) fn latest_derived_block(&self) -> Result<DerivedRefPair, StorageError> {
        let mut cursor = self.tx.cursor_read::<DerivedBlocks>().inspect_err(|err| {
            error!(target: "supervisor_storage",
              ?err,
              "Failed to get cursor for DerivedBlocks"
            );
        })?;

        let result = cursor.last().inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                ?err,
                "Failed to seek to last block"
            );
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
    /// Helper to save genesis [`StoredDerivedBlockPair`] to
    /// [`DerivedBlocks`](`crate::models::DerivedBlocks`) table in the database.
    fn save_genesis_derived_block_pair(
        &self,
        genesis_ref_pair: DerivedRefPair,
    ) -> Result<(), StorageError> {
        // validate that the genesis block pair info is correct
        // todo: refactor
        if genesis_ref_pair.derived.number != 0 &&
            genesis_ref_pair.derived.parent_hash != B256::ZERO &&
            genesis_ref_pair.source.number != 0 &&
            genesis_ref_pair.source.hash != B256::ZERO
        {
            warn!(
                target: "supervisor_storage",
                genesis_block_pair = ?genesis_ref_pair,
                "Genesis block info is not correct"
            );
            return Err(StorageError::EntryNotFound(
                "Genesis block info is not correct".to_string(),
            ));
        }

        self.tx.put::<DerivedBlocks>(0, genesis_ref_pair.clone().into()).inspect_err(|err| {
            error!(
                target: "supervisor_storage", 
                genesis_block_pair = ?genesis_ref_pair,
                ?err,
                "Failed to save genesis derived block pair");
        })?;
        Ok(())
    }

    /// Saves a [`StoredDerivedBlockPair`] to [`DerivedBlocks`](`crate::models::DerivedBlocks`)
    /// table and [`U64List`] to [`SourceToDerivedBlockNumbers`](`SourceToDerivedBlockNumbers`)
    /// table in the database.
    #[expect(dead_code)]
    pub(crate) fn save_derived_block_pair(
        &self,
        incoming_pair: DerivedRefPair,
    ) -> Result<(), StorageError> {
        let latest_block_pair = match self.latest_derived_block() {
            Ok(pair) => Some(pair),
            Err(StorageError::EntryNotFound(_)) => None,
            Err(e) => return Err(e),
        };

        if latest_block_pair.is_none() {
            // handle genesis block pair
            // todo: check if we need to expose separate function for handling genesis block
            return self.save_genesis_derived_block_pair(incoming_pair);
        }

        let latest_block_pair = latest_block_pair.unwrap();
        // Validate if the latest derived block is parent of the incoming derived block
        // todo: refactor
        if latest_block_pair.derived.number + 1 != incoming_pair.derived.number ||
            latest_block_pair.derived.hash != incoming_pair.derived.parent_hash
        {
            warn!(
                target: "supervisor_storage",
                latest_derived_block_pair = ?latest_block_pair,
                incoming_derived_block_pair = ?incoming_pair,
                "Latest stored derived block is not parent of the incoming derived block"
            );
            return Err(StorageError::ConflictError(
                "Latest stored derived block is not parent of the incoming derived block"
                    .to_string(),
            ));
        }

        let mut derived_block_numbers =
            match self.derived_block_numbers_at_source(incoming_pair.source.number) {
                Ok(list) => list,
                Err(StorageError::EntryNotFound(_)) => U64List::default(),
                Err(err) => {
                    error!(
                      target: "supervisor_storage",
                      incoming_derived_block_pair = ?incoming_pair,
                      ?err,
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
                    incoming_derived_block_pair = ?incoming_pair,
                    ?err,
                    "Failed to save derived block pair"
                );
            })?;

        // Save the derived block numbers to the database
        self.tx
            .put::<SourceToDerivedBlockNumbers>(incoming_pair.source.number, derived_block_numbers)
            .inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    incoming_derived_block_pair = ?incoming_pair,
                    ?err,
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
    use alloy_primitives::{B256, U256};
    use kona_interop::DerivedRefPair;
    use kona_protocol::BlockInfo;
    use reth_db::{
        Database, DatabaseEnv,
        mdbx::{DatabaseArguments, init_db_for},
    };
    use tempfile::TempDir;

    fn block_info(number: u64, parent_hash: B256, timestamp: u64) -> BlockInfo {
        BlockInfo { hash: B256::from(U256::from(number)), number, parent_hash, timestamp }
    }

    const fn derived_pair(source: BlockInfo, derived: BlockInfo) -> DerivedRefPair {
        DerivedRefPair { source, derived }
    }

    fn genesis_pair() -> DerivedRefPair {
        let source =
            BlockInfo { hash: B256::ZERO, number: 0, parent_hash: B256::ZERO, timestamp: 0 };
        let derived = block_info(0, B256::ZERO, 100);
        derived_pair(source, derived)
    }

    /// Sets up a new temp DB and inserts the genesis block.
    fn setup_db_and_genesis() -> (TempDir, DatabaseEnv, DerivedRefPair) {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        let db = init_db_for::<_, Tables>(temp_dir.path(), DatabaseArguments::default())
            .expect("Failed to init database");
        let genesis = genesis_pair();
        insert_pair(&db, &genesis).expect("Failed to insert genesis");
        (temp_dir, db, genesis)
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
    fn save_derived_block_pair_positive() {
        let (_temp_dir, db, genesis) = setup_db_and_genesis();

        let source1 = block_info(100, B256::from(U256::from(100)), 200);
        let derived1 = block_info(1, genesis.derived.hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(insert_pair(&db, &pair1).is_ok());

        let derived2 = block_info(2, derived1.hash, 300);
        let pair2 = derived_pair(source1, derived2);
        assert!(insert_pair(&db, &pair2).is_ok());

        let source3 = block_info(200, B256::from(U256::from(200)), 400);
        let derived3 = block_info(3, derived2.hash, 400);
        let pair3 = derived_pair(source3, derived3);
        assert!(insert_pair(&db, &pair3).is_ok());
    }

    #[test]
    fn save_derived_block_pair_wrong_parent_should_fail() {
        let (_temp_dir, db, genesis) = setup_db_and_genesis();

        let source1 = block_info(100, B256::from(U256::from(100)), 200);
        let derived1 = block_info(1, genesis.derived.hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(insert_pair(&db, &pair1).is_ok());

        let wrong_parent_hash = B256::from(U256::from(9995412));
        let derived2 = block_info(2, wrong_parent_hash, 300);
        let pair2 = derived_pair(source1, derived2);
        let result = insert_pair(&db, &pair2);
        assert!(result.is_err());
        if let Err(StorageError::ConflictError(msg)) = result {
            assert!(msg.contains("Latest stored derived block is not parent"));
        } else {
            panic!("Expected ConflictError");
        }
    }

    #[test]
    fn save_derived_block_pair_gap_in_number_should_fail() {
        let (_temp_dir, db, genesis) = setup_db_and_genesis();

        let source1 = block_info(100, B256::from(U256::from(100)), 200);
        let derived1 = block_info(1, genesis.derived.hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(insert_pair(&db, &pair1).is_ok());

        let derived2 = block_info(4, derived1.hash, 400); // should be 2, not 4
        let pair2 = derived_pair(source1, derived2);
        let result = insert_pair(&db, &pair2);
        assert!(result.is_err());
        if let Err(StorageError::ConflictError(msg)) = result {
            assert!(msg.contains("Latest stored derived block is not parent"));
        } else {
            panic!("Expected ConflictError");
        }
    }

    #[test]
    fn duplicate_derived_block_number_should_fail() {
        let (_temp_dir, db, genesis) = setup_db_and_genesis();

        let source1 = block_info(100, B256::from(U256::from(100)), 200);
        let derived1 = block_info(1, genesis.derived.hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(insert_pair(&db, &pair1).is_ok());

        // Try to insert the same derived block again
        let result = insert_pair(&db, &pair1);
        assert!(result.is_err());
        if let Err(StorageError::ConflictError(msg)) = result {
            assert!(msg.contains("Latest stored derived block is not parent"));
        } else {
            panic!("Expected ConflictError");
        }
    }

    #[test]
    fn non_monotonic_l2_number_should_fail() {
        let (_temp_dir, db, genesis) = setup_db_and_genesis();

        let source1 = block_info(100, B256::from(U256::from(100)), 200);
        let derived1 = block_info(1, genesis.derived.hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(insert_pair(&db, &pair1).is_ok());

        let derived2 = block_info(2, derived1.hash, 300);
        let pair2 = derived_pair(source1, derived2);
        assert!(insert_pair(&db, &pair2).is_ok());

        // Try to insert a block with a lower number than the latest
        let derived_non_monotonic = block_info(1, derived2.hash, 400);
        let pair_non_monotonic = derived_pair(source1, derived_non_monotonic);
        let result = insert_pair(&db, &pair_non_monotonic);
        assert!(result.is_err());
        if let Err(StorageError::ConflictError(msg)) = result {
            assert!(msg.contains("Latest stored derived block is not parent"));
        } else {
            panic!("Expected ConflictError");
        }
    }

    #[test]
    fn reinserting_genesis_block_should_fail() {
        let (_temp_dir, db, genesis) = setup_db_and_genesis();

        // Try to insert the same genesis block again
        let result = insert_pair(&db, &genesis);
        assert!(result.is_err());
        if let Err(StorageError::ConflictError(msg)) = result {
            assert!(msg.contains("Latest stored derived block is not parent"));
        } else {
            panic!("Expected ConflictError");
        }
    }
}
