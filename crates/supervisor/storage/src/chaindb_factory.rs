use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use alloy_primitives::ChainId;

use crate::{chaindb::ChainDb, error::StorageError};

/// Factory for managing multiple chain databases.
/// This struct allows for the creation and retrieval of `ChainDb` instances
/// based on chain IDs, ensuring that each chain has its own database instance.
#[derive(Debug)]
pub struct ChainDbFactory {
    db_path: PathBuf,
    dbs: RwLock<HashMap<ChainId, Arc<ChainDb>>>,
}

impl ChainDbFactory {
    /// Create a new, empty factory.
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path, dbs: RwLock::new(HashMap::new()) }
    }

    /// Get or create a [`ChainDb`] for the given chain id.
    ///
    /// If the database does not exist, it will be created at the path `self.db_path/<chain_id>`.
    pub fn get_or_create_db(&self, chain_id: ChainId) -> Result<Arc<ChainDb>, StorageError> {
        {
            // Try to get it without locking for write
            let dbs = self.dbs.write().unwrap_or_else(|e| e.into_inner());
            if let Some(db) = dbs.get(&chain_id) {
                return Ok(db.clone());
            }
        }

        // Not found, create and insert
        let mut dbs = self.dbs.write().unwrap_or_else(|e| e.into_inner());
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
}
