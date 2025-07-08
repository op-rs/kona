//! Provider for derivation-related database operations.
use crate::{
    error::StorageError,
    models::{
        BlockTraversal, DerivedBlocks, SourceBlockTraversal, StoredDerivedBlockPair, U64List,
    },
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
    pub(crate) fn get_derived_block_pair(
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

    /// Gets the [`SourceBlockTraversal`] for the given source block number.
    ///
    /// # Arguments
    ///
    /// * `source_block_number` - The source block number.
    ///
    /// Returns the [`SourceBlockTraversal`] for the given source block number.
    fn get_block_traversal(
        &self,
        source_block_number: u64,
    ) -> Result<SourceBlockTraversal, StorageError> {
        let block_traversal =
            self.tx.get::<BlockTraversal>(source_block_number).inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    source_block_number,
                    %err,
                    "Failed to get block traversal info for source block"
                );
            })?;

        block_traversal.ok_or_else(|| {
            warn!(
              target: "supervisor_storage",
              source_block_number,
              "source block not found"
            );

            // todo: replace with a more specific error
            StorageError::EntryNotFound("source block not found".to_string())
        })
    }

    /// Gets the latest derived [`BlockInfo`] at the given source [`BlockNumHash`].
    /// This does NOT mean to get the derived block that is "derived from" the source block.
    /// It could happen that a source block has no derived blocks, in which case the latest derived
    /// block is from one of the previous source blocks.
    ///
    /// Returns the latest derived block pair.
    pub(crate) fn latest_derived_block_at_source(
        &self,
        source_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        let mut block_traversal = self.get_block_traversal(source_block_id.number)?;

        if block_traversal.source.hash != source_block_id.hash {
            warn!(
                target: "supervisor_storage",
                source_block_hash = %source_block_id.hash,
                "Source block hash mismatch"
            );
            return Err(StorageError::EntryNotFound("source block hash mismatch".to_string()));
        }

        while block_traversal.derived_block_numbers.is_empty() {
            let prev_block_traversal =
                self.get_block_traversal(block_traversal.source.number - 1)?;
            block_traversal = prev_block_traversal;
        }

        let derived_block_number =
            block_traversal.derived_block_numbers.last().ok_or_else(|| {
                StorageError::EntryNotFound("no derived blocks for this source block".to_string())
            })?;

        let derived_block_pair = self.get_derived_block_pair_by_number(*derived_block_number)?;
        Ok(derived_block_pair.derived.into())
    }

    /// Gets the latest derivation state [`DerivedRefPair`], which includes the latest source block
    /// and the latest derived block.
    ///
    /// # Returns
    /// A [`DerivedRefPair`] containing the latest source block and latest derived block.
    pub(crate) fn latest_derivation_state(&self) -> Result<DerivedRefPair, StorageError> {
        let mut cursor = self.tx.cursor_read::<DerivedBlocks>().inspect_err(|err| {
            error!(
                target: "supervisor_storage",
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
            error!(
                target: "supervisor_storage",
                "No blocks found in storage"
            );
            StorageError::EntryNotFound("no blocks found".to_string())
        })?;

        let latest_source_block = self.latest_source_block().inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                %err,
                "Failed to get latest source block"
            );
        })?;

        Ok(DerivedRefPair { source: latest_source_block, derived: block.derived.into() })
    }

    /// Gets the latest [`SourceBlockTraversal`].
    ///
    /// # Returns
    /// The latest [`SourceBlockTraversal`] in the database.
    fn latest_source_block_traversal(&self) -> Result<SourceBlockTraversal, StorageError> {
        let mut cursor = self.tx.cursor_read::<BlockTraversal>()?;
        let result = cursor.last()?;

        let (_, block_traversal) = result.ok_or_else(|| StorageError::DatabaseNotInitialised)?;
        Ok(block_traversal)
    }

    /// Gets the source block for the given source block number.
    fn get_source_block(&self, source_block_number: u64) -> Result<BlockInfo, StorageError> {
        let block_traversal = self.get_block_traversal(source_block_number)?;
        Ok(block_traversal.source.into())
    }

    /// Gets the latest source block, even if it has no derived blocks.
    pub(crate) fn latest_source_block(&self) -> Result<BlockInfo, StorageError> {
        let block = self.latest_source_block_traversal().inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                %err,
                "Failed to get latest source block traversal"
            );
        })?;

        Ok(block.source.into())
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
            Err(StorageError::EntryNotFound(_)) => {
                self.save_source_block_internal(anchor.source)?;
                self.save_derived_block_pair_internal(anchor)?;
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    /// Saves a [`StoredDerivedBlockPair`] to [`DerivedBlocks`](`crate::models::DerivedBlocks`)
    /// table and [`SourceBlockTraversal`] to [`BlockTraversal`](`crate::models::BlockTraversal`)
    /// table in the database.
    pub(crate) fn save_derived_block_pair(
        &self,
        incoming_pair: DerivedRefPair,
    ) -> Result<(), StorageError> {
        // todo: use cursor to get the last block(performance improvement)
        let latest_derivation_state = match self.latest_derivation_state() {
            Ok(pair) => pair,
            Err(StorageError::EntryNotFound(_)) => return Err(StorageError::DatabaseNotInitialised),
            Err(e) => return Err(e),
        };

        // If the incoming derived block is not newer than the latest stored derived block,
        // we do not save it, check if it is consistent with the saved state.
        // If it is not consistent, we return an error.
        if latest_derivation_state.derived.number >= incoming_pair.derived.number {
            let stored_pair = self
                .get_derived_block_pair_by_number(incoming_pair.derived.number)
                .inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    incoming_derived_block_pair = %incoming_pair,
                    %err,
                    "Failed to get derived block pair"
                );
            })?;

            if incoming_pair == stored_pair.into() {
                return Ok(());
            } else {
                error!(
                    target: "supervisor_storage",
                    latest_derived_block_pair = %latest_derivation_state,
                    incoming_derived_block_pair = %incoming_pair,
                    "Incoming derived block is not consistent with the latest stored derived block"
                );
                return Err(StorageError::ConflictError(
                    "incoming derived block is not consistent with the stored derived block"
                        .to_string(),
                ));
            }
        }

        // Latest source block must be same as the incoming source block
        if latest_derivation_state.source != incoming_pair.source {
            warn!(
                target: "supervisor_storage",
                latest_source_block = %latest_derivation_state.source,
                incoming_source = %incoming_pair.source,
                "Latest source block does not match the incoming derived block source"
            );
            return Err(StorageError::BlockOutOfOrder);
        }

        if !latest_derivation_state.derived.is_parent_of(&incoming_pair.derived) {
            warn!(
              target: "supervisor_storage",
              latest_derived_block_pair = %latest_derivation_state,
              incoming_derived_block_pair = %incoming_pair,
              "Latest stored derived block is not parent of the incoming derived block"
            );
            return Err(StorageError::DerivedBlockOutOfOrder);
        }

        self.save_derived_block_pair_internal(incoming_pair)
    }

    /// Internal function to save a derived block pair.
    /// This function does not perform checks on the incoming derived pair,
    /// it assumes that the pair is valid and the latest derived block is its parent.
    fn save_derived_block_pair_internal(
        &self,
        incoming_pair: DerivedRefPair,
    ) -> Result<(), StorageError> {
        // the derived block must be derived from the latest source block
        let mut block_traversal = self.latest_source_block_traversal().inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                incoming_derived_block_pair = %incoming_pair,
                %err,
                "Failed to get latest source block traversal"
            );
        })?;

        let latest_source_block = block_traversal.clone().source.into();
        if incoming_pair.source != latest_source_block {
            warn!(
                target: "supervisor_storage",
                latest_source_block = %latest_source_block,
                incoming_source = %incoming_pair.source,
                "Latest source block does not match the incoming derived block source"
            );
            return Err(StorageError::BlockOutOfOrder);
        }

        // Add the derived block number to the list
        block_traversal.derived_block_numbers.push(incoming_pair.derived.number);

        // Save the derived block pair to the database
        self.tx
            .put::<DerivedBlocks>(incoming_pair.derived.number, incoming_pair.into())
            .inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    incoming_derived_block_pair = %incoming_pair,
                    %err,
                    "Failed to save derived block pair"
                );
            })?;

        // Save the SourceBlockTraversal to the database
        self.tx.put::<BlockTraversal>(incoming_pair.source.number, block_traversal).inspect_err(
            |err| {
                error!(
                    target: "supervisor_storage",
                    incoming_derived_block_pair = %incoming_pair,
                    %err,
                    "Failed to save derived block numbers for source block"
                );
            },
        )?;

        Ok(())
    }

    /// Saves a source block to the database.
    /// If the source block already exists, it does nothing.
    /// If the source block does not exist, it creates a new [`SourceBlockTraversal`] and saves it
    /// to the database.
    pub(crate) fn save_source_block(&self, incoming_source: BlockInfo) -> Result<(), StorageError> {
        let latest_source_block = match self.latest_source_block() {
            Ok(latest_source_block) => latest_source_block,
            Err(StorageError::EntryNotFound(_)) => return Err(StorageError::DatabaseNotInitialised),
            Err(err) => return Err(err),
        };

        // idempotent check: if the source block already exists, do nothing
        if latest_source_block == incoming_source {
            return Ok(());
        }

        // If the incoming source block is not newer than the latest source block,
        // we do not save it, check if it is consistent with the saved state.
        // If it is not consistent, we return an error.
        if latest_source_block.number > incoming_source.number {
            let source_block =
                self.get_source_block(incoming_source.number).inspect_err(|err| {
                    error!(
                        target: "supervisor_storage",
                        incoming_source = %incoming_source,
                        %err,
                        "Failed to get source block"
                    );
                })?;

            if source_block == incoming_source {
                return Ok(());
            } else {
                error!(
                    target: "supervisor_storage",
                    latest_source_block = %latest_source_block,
                    incoming_source = %incoming_source,
                    "Incoming source block is not consistent with the latest source block"
                );
                return Err(StorageError::ConflictError(
                    "incoming source block is not consistent with the stored source block"
                        .to_string(),
                ));
            }
        }

        if !latest_source_block.is_parent_of(&incoming_source) {
            error!(
                target: "supervisor_storage",
                latest_source_block = %latest_source_block,
                incoming_source = %incoming_source,
                "Stored latest source block is not parent of the incoming source block"
            );
            return Err(StorageError::BlockOutOfOrder);
        }

        self.save_source_block_internal(incoming_source)?;
        Ok(())
    }

    fn save_source_block_internal(&self, incoming_source: BlockInfo) -> Result<(), StorageError> {
        let block_traversal = SourceBlockTraversal {
            source: incoming_source.into(),
            derived_block_numbers: U64List::default(),
        };

        self.tx.put::<BlockTraversal>(incoming_source.number, block_traversal).inspect_err(
            |err| {
                error!(target: "supervisor_storage", %err, "Failed to save block traversal");
            },
        )?;

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
        let res = provider.initialise(*pair);
        if res.is_ok() {
            tx.commit().expect("Failed to commit transaction");
        }
        res
    }

    /// Helper to insert a pair in a new transaction, committing if successful.
    fn insert_pair(db: &DatabaseEnv, pair: &DerivedRefPair) -> Result<(), StorageError> {
        let tx = db.tx_mut().expect("Could not get mutable tx");
        let provider = DerivationProvider::new(&tx);
        let res = provider.save_derived_block_pair(*pair);
        if res.is_ok() {
            tx.commit().expect("Failed to commit transaction");
        }
        res
    }

    /// Helper to insert a source block in a new transaction, committing if successful.
    fn insert_source_block(db: &DatabaseEnv, source: &BlockInfo) -> Result<(), StorageError> {
        let tx = db.tx_mut().expect("Could not get mutable tx");
        let provider = DerivationProvider::new(&tx);
        let res = provider.save_source_block(*source);
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

        let source3 = block_info(101, source1.hash, 400);
        let derived3 = block_info(3, derived2.hash, 400);
        let pair3 = derived_pair(source3, derived3);
        assert!(insert_source_block(&db, &source3).is_ok());
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
    fn duplicate_derived_block_number_should_pass() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        // Try to insert the same derived block again
        let result = insert_pair(&db, &pair1);
        assert!(result.is_ok(), "Should allow inserting the same derived block again");
    }

    #[test]
    fn save_old_block_should_pass() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let derived2 = block_info(2, derived1.hash, 300);
        let pair2 = derived_pair(source1, derived2);
        assert!(insert_pair(&db, &pair2).is_ok());

        // Try to insert a block with a lower number than the latest
        let result = insert_pair(&db, &pair1);
        assert!(result.is_ok(), "Should allow inserting an old derived block");
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
        assert!(matches!(result, Err(StorageError::ConflictError(_))));
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

        let source2 = block_info(101, B256::from([100u8; 32]), 300);
        let derived3 = block_info(3, derived2.hash, 400);
        let pair3 = derived_pair(source2, derived3);
        assert!(insert_source_block(&db, &source2).is_ok());
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
    fn test_latest_derivation_state() {
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

        let latest = provider.latest_derivation_state().expect("should exist");
        assert_eq!(latest, pair2);
    }

    #[test]
    fn test_latest_derivation_state_empty_source() {
        let db = setup_db();

        let source1 = block_info(100, B256::from([100u8; 32]), 200);
        let derived1 = block_info(1, genesis_block().hash, 200);
        let pair1 = derived_pair(source1, derived1);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source2 = block_info(101, source1.hash, 300);
        let derived2 = block_info(2, derived1.hash, 300);
        let pair2 = derived_pair(source2, derived2);
        assert!(insert_source_block(&db, &source2).is_ok());
        assert!(insert_pair(&db, &pair2).is_ok());

        let source3 = block_info(102, source2.hash, 400);
        assert!(insert_source_block(&db, &source3).is_ok());
        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);

        let latest = provider.latest_derivation_state().expect("should exist");
        let expected_derivation_state = DerivedRefPair { source: source3, derived: derived2 };
        assert_eq!(latest, expected_derivation_state);
    }

    #[test]
    fn test_latest_derivation_state_empty_returns_error() {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        let db = init_db_for::<_, Tables>(temp_dir.path(), DatabaseArguments::default())
            .expect("Failed to init database");

        let tx = db.tx().expect("Could not get tx");
        let provider = DerivationProvider::new(&tx);
        assert!(matches!(provider.latest_derivation_state(), Err(StorageError::EntryNotFound(_))));
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

    #[test]
    fn save_source_block_positive() {
        let db = setup_db();

        let derived0 = block_info(10, B256::from([10u8; 32]), 200);
        let pair1 = derived_pair(genesis_block(), derived0);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source1 = block_info(1, genesis_block().hash, 200);
        assert!(insert_source_block(&db, &source1).is_ok());
    }

    #[test]
    fn save_source_block_idempotent_should_pass() {
        let db = setup_db();

        let derived0 = block_info(10, B256::from([10u8; 32]), 200);
        let pair1 = derived_pair(genesis_block(), derived0);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source1 = block_info(1, genesis_block().hash, 200);
        assert!(insert_source_block(&db, &source1).is_ok());
        // Try saving the same block again
        assert!(insert_source_block(&db, &source1).is_ok());
    }

    #[test]
    fn save_source_invalid_parent_should_fail() {
        let db = setup_db();

        let source0 = block_info(10, B256::from([10u8; 32]), 200);
        let derived0 = genesis_block();
        let pair1 = derived_pair(source0, derived0);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source1 = block_info(11, B256::from([1u8; 32]), 200);
        let result = insert_source_block(&db, &source1);
        assert!(
            matches!(result, Err(StorageError::BlockOutOfOrder)),
            "Should fail with BlockOutOfOrder error"
        );
    }

    #[test]
    fn save_source_block_lower_number_should_pass() {
        let db = setup_db();

        let source0 = block_info(10, B256::from([10u8; 32]), 200);
        let derived0 = genesis_block();
        let pair1 = derived_pair(source0, derived0);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source1 = block_info(11, source0.hash, 400);
        assert!(insert_source_block(&db, &source1).is_ok());

        // Try to save a block with a lower number
        let result = insert_source_block(&db, &source0);
        assert!(result.is_ok(), "Should allow saving a old source block");
    }

    #[test]
    fn save_inconsistent_source_block_lower_number_should_fail() {
        let db = setup_db();

        let source0 = block_info(10, B256::from([10u8; 32]), 200);
        let derived0 = genesis_block();
        let pair1 = derived_pair(source0, derived0);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source1 = block_info(11, source0.hash, 400);
        assert!(insert_source_block(&db, &source1).is_ok());

        let old_source = block_info(source0.number, B256::from([1u8; 32]), 400);
        // Try to save a block with a lower number
        let result = insert_source_block(&db, &old_source);
        assert!(matches!(result, Err(StorageError::ConflictError(_))));
    }

    #[test]
    fn save_source_block_gap_number_should_fail() {
        let db = setup_db();

        let derived0 = block_info(10, B256::from([10u8; 32]), 200);
        let pair1 = derived_pair(genesis_block(), derived0);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source1 = block_info(2, genesis_block().hash, 400);
        // Try to skip a block
        let result = insert_source_block(&db, &source1);
        assert!(matches!(result, Err(StorageError::BlockOutOfOrder)));
    }

    #[test]
    fn save_source_block_higher_number_should_succeed() {
        let db = setup_db();

        let derived0 = block_info(10, B256::from([10u8; 32]), 200);
        let pair1 = derived_pair(genesis_block(), derived0);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source1 = block_info(1, genesis_block().hash, 200);
        let source2 = block_info(2, source1.hash, 400);
        assert!(insert_source_block(&db, &source1).is_ok());
        assert!(insert_source_block(&db, &source2).is_ok());
    }

    #[test]
    fn save_source_block_traversal_updates_existing_traversal_positive() {
        let db = setup_db();

        let derived0 = block_info(10, B256::from([10u8; 32]), 200);
        let pair1 = derived_pair(genesis_block(), derived0);
        assert!(initialize_db(&db, &pair1).is_ok());

        let source1 = block_info(1, genesis_block().hash, 200);
        assert!(insert_source_block(&db, &source1).is_ok());

        let derived1 = block_info(100, derived0.hash, 200);

        let tx = db.tx_mut().expect("Could not get mutable tx");
        let provider = DerivationProvider::new(&tx);
        let mut block_traversal =
            provider.get_block_traversal(source1.number).expect("should exist");
        block_traversal.derived_block_numbers.push(derived1.number);
        assert!(tx.put::<BlockTraversal>(source1.number, block_traversal).is_ok());
    }
}
