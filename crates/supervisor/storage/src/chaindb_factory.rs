use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use alloy_primitives::ChainId;
use kona_protocol::BlockInfo;
use tracing::error;

use crate::{FinalizedL1Storage, chaindb::ChainDb, error::StorageError};

/// Factory for managing multiple chain databases.
/// This struct allows for the creation and retrieval of `ChainDb` instances
/// based on chain IDs, ensuring that each chain has its own database instance.
#[derive(Debug)]
pub struct ChainDbFactory {
    db_path: PathBuf,
    dbs: RwLock<HashMap<ChainId, Arc<ChainDb>>>,

    /// Finalized L1 block reference, used for tracking the finalized L1 block.
    /// In-memory only, not persisted.
    finalized_l1: RwLock<Option<BlockInfo>>,
}

impl ChainDbFactory {
    /// Create a new, empty factory.
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path, dbs: RwLock::new(HashMap::new()), finalized_l1: RwLock::new(None) }
    }

    /// Get or create a [`ChainDb`] for the given chain id.
    ///
    /// If the database does not exist, it will be created at the path `self.db_path/<chain_id>`.
    pub fn get_or_create_db(&self, chain_id: ChainId) -> Result<Arc<ChainDb>, StorageError> {
        {
            // Try to get it without locking for write
            let dbs = self.dbs.read().map_err(|err| {
                error!(target: "supervisor_storage", %err, "Failed to acquire read lock on databases");
                StorageError::LockPoisoned
            })?;
            if let Some(db) = dbs.get(&chain_id) {
                return Ok(db.clone());
            }
        }

        // Not found, create and insert
        let mut dbs = self.dbs.write().map_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to acquire write lock on databases");
            StorageError::LockPoisoned
        })?;
        // Double-check in case another thread inserted
        if let Some(db) = dbs.get(&chain_id) {
            return Ok(db.clone());
        }

        let chain_db_path = self.db_path.join(chain_id.to_string());
        let db = Arc::new(ChainDb::new(chain_db_path.as_path())?);
        dbs.insert(chain_id, db.clone());
        Ok(db)
    }

    /// Get a [`ChainDb`] for the given chain id, returning an error if it doesn't exist.
    ///
    /// # Returns
    /// * `Ok(Arc<ChainDb>)` if the database exists.
    /// * `Err(StorageError)` if the database does not exist.
    pub fn get_db(&self, chain_id: ChainId) -> Result<Arc<ChainDb>, StorageError> {
        let dbs = self.dbs.read().unwrap_or_else(|e| e.into_inner());
        dbs.get(&chain_id)
            .cloned()
            .ok_or_else(|| StorageError::EntryNotFound("chain not found".to_string()))
    }
}

impl FinalizedL1Storage for ChainDbFactory {
    fn get_finalized_l1(&self) -> Result<BlockInfo, StorageError> {
        let guard = self.finalized_l1.read().map_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to acquire read lock on finalized_l1");
            StorageError::LockPoisoned
        })?;
        guard.as_ref().cloned().ok_or(StorageError::FutureData)
    }

    fn update_finalized_l1(&self, block: BlockInfo) -> Result<(), StorageError> {
        let mut guard = self
            .finalized_l1
            .write()
            .map_err(|err| {
                error!(target: "supervisor_storage", %err, "Failed to acquire write lock on finalized_l1");
                StorageError::LockPoisoned
            })?;

        // Check if the new block number is greater than the current finalized block
        if let Some(ref current) = *guard {
            if block.number <= current.number {
                error!(target: "supervisor_storage",
                    current_block_number = current.number,
                    new_block_number = block.number,
                    "New finalized block number is not greater than current finalized block number",
                );
                return Err(StorageError::BlockOutOfOrder);
            }
        }
        *guard = Some(block);

        // update all chain databases finalized safety head refs
        let dbs = self.dbs.read().map_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to acquire read lock on databases");
            StorageError::LockPoisoned
        })?;
        for (chain_id, db) in dbs.iter() {
            if let Err(err) = db.update_finalized_head_ref(block) {
                error!(target: "supervisor_storage", chain_id = %chain_id, %err, "Failed to update finalized L1 in chain database");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_factory() -> (TempDir, ChainDbFactory) {
        let tmp = TempDir::new().expect("create temp dir");
        let factory = ChainDbFactory::new(tmp.path().to_path_buf());
        (tmp, factory)
    }

    #[test]
    fn test_get_or_create_db_creates_and_returns_db() {
        let (_tmp, factory) = temp_factory();
        let db = factory.get_or_create_db(1).expect("should create db");
        assert!(Arc::strong_count(&db) >= 1);
    }

    #[test]
    fn test_get_or_create_db_returns_same_instance() {
        let (_tmp, factory) = temp_factory();
        let db1 = factory.get_or_create_db(42).unwrap();
        let db2 = factory.get_or_create_db(42).unwrap();
        assert!(Arc::ptr_eq(&db1, &db2));
    }

    #[test]
    fn test_get_db_returns_error_if_not_exists() {
        let (_tmp, factory) = temp_factory();
        let err = factory.get_db(999).unwrap_err();
        assert!(matches!(err, StorageError::EntryNotFound(_)));
    }

    #[test]
    fn test_get_db_returns_existing_db() {
        let (_tmp, factory) = temp_factory();
        let db = factory.get_or_create_db(7).unwrap();
        let db2 = factory.get_db(7).unwrap();
        assert!(Arc::ptr_eq(&db, &db2));
    }

    #[test]
    fn test_db_path_is_unique_per_chain() {
        let (tmp, factory) = temp_factory();
        let db1 = factory.get_or_create_db(1).unwrap();
        let db2 = factory.get_or_create_db(2).unwrap();
        assert!(!Arc::ptr_eq(&db1, &db2));

        assert!(tmp.path().join("1").exists());
        assert!(tmp.path().join("2").exists());
    }

    #[test]
    fn test_get_finalized_l1_returns_error_when_none() {
        let (_tmp, factory) = temp_factory();
        let err = factory.get_finalized_l1().unwrap_err();
        assert!(matches!(err, StorageError::FutureData));
    }

    #[test]
    fn test_update_and_get_finalized_l1_success() {
        let (_tmp, factory) = temp_factory();
        let block1 = BlockInfo { number: 100, ..Default::default() };
        let block2 = BlockInfo { number: 200, ..Default::default() };

        // Set first finalized block
        factory.update_finalized_l1(block1).unwrap();
        assert_eq!(factory.get_finalized_l1().unwrap(), block1);

        // Update with higher block number
        factory.update_finalized_l1(block2).unwrap();
        assert_eq!(factory.get_finalized_l1().unwrap(), block2);
    }

    #[test]
    fn test_update_finalized_l1_with_lower_block_number_errors() {
        let (_tmp, factory) = temp_factory();
        let block1 = BlockInfo { number: 100, ..Default::default() };
        let block2 = BlockInfo { number: 50, ..Default::default() };

        factory.update_finalized_l1(block1).unwrap();
        let err = factory.update_finalized_l1(block2).unwrap_err();
        assert!(matches!(err, StorageError::BlockOutOfOrder));
    }

    #[test]
    fn test_update_finalized_l1_with_same_block_number_errors() {
        let (_tmp, factory) = temp_factory();
        let block1 = BlockInfo { number: 100, ..Default::default() };
        let block2 = BlockInfo { number: 100, ..Default::default() };

        factory.update_finalized_l1(block1).unwrap();
        let err = factory.update_finalized_l1(block2).unwrap_err();
        assert!(matches!(err, StorageError::BlockOutOfOrder));
    }
}
