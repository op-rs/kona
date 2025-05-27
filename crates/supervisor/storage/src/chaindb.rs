//! Main database access structure and transaction contexts.

use crate::{
    error::StorageError,
    providers::{DerivationProvider, LogProvider},
    traits::{DerivationStorage, LogStorage},
};
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_types::Log;
use reth_db::{
    DatabaseEnv,
    mdbx::{DatabaseArguments, init_db_for},
};
use reth_db_api::database::Database;
use std::path::Path;

/// Manages the database environment for a single chain.
/// Provides transactional access to data via providers.
#[derive(Debug)]
pub struct ChainDb {
    env: DatabaseEnv,
}

impl ChainDb {
    /// Creates or opens a database environment at the given path.
    pub fn new(path: &Path) -> Result<Self, StorageError> {
        let env = init_db_for::<_, crate::models::Tables>(path, DatabaseArguments::default())?;
        Ok(Self { env })
    }
}

impl DerivationStorage for ChainDb {
    fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| DerivationProvider::new(tx).derived_to_source(derived_block_id))?
    }

    fn latest_derived_block_at_source(
        &self,
        source_block_id: BlockNumHash,
    ) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| {
            DerivationProvider::new(tx).latest_derived_block_at_source(source_block_id)
        })?
    }

    fn latest_derived_block_pair(&self) -> Result<DerivedRefPair, StorageError> {
        self.env.view(|tx| DerivationProvider::new(tx).latest_derived_block_pair())?
    }

    fn save_derived_block_pair(&self, incoming_pair: DerivedRefPair) -> Result<(), StorageError> {
        self.env
            .update(|ctx| DerivationProvider::new(ctx).save_derived_block_pair(incoming_pair))?
    }
}

impl LogStorage for ChainDb {
    fn get_latest_block(&self) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| LogProvider::new(tx).get_latest_block())?
    }

    fn get_block_by_log(&self, block_number: u64, log: &Log) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| LogProvider::new(tx).get_block_by_log(block_number, log))?
    }

    fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, StorageError> {
        self.env.view(|tx| LogProvider::new(tx).get_logs(block_number))?
    }

    fn store_block_logs(&self, block: &BlockInfo, logs: Vec<Log>) -> Result<(), StorageError> {
        self.env.update(|ctx| LogProvider::new(ctx).store_block_logs(block, logs))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use kona_supervisor_types::Log;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_open_db() {
        let tmp_dir = TempDir::new().expect("create temp dir");
        let db_path = tmp_dir.path().join("chaindb");
        let db = ChainDb::new(&db_path);
        assert!(db.is_ok(), "Should create or open database");
    }

    #[test]
    fn test_log_storage() {
        let tmp_dir = TempDir::new().expect("create temp dir");
        let db_path = tmp_dir.path().join("chaindb_logs");
        let db = ChainDb::new(&db_path).expect("create db");

        // Create dummy block and logs
        let block = BlockInfo::default();
        let log1 = Log { index: 0, hash: B256::from([0u8; 32]), executing_message: None };
        let log2 = Log { index: 1, hash: B256::from([1u8; 32]), executing_message: None };
        let logs = vec![log1, log2];

        // Store logs
        db.store_block_logs(&block, logs.clone()).expect("store logs");

        // Retrieve logs
        let retrieved_logs = db.get_logs(block.number).expect("get logs");
        assert_eq!(retrieved_logs.len(), 2);
        assert_eq!(retrieved_logs, logs, "First log should match stored log");

        let latest_block = db.get_latest_block().expect("latest block");
        assert_eq!(latest_block, block, "Latest block should match stored block");

        let block_by_log = db.get_block_by_log(block.number, &logs[1]).expect("get block by log");
        assert_eq!(block_by_log, block, "Block by log should match stored block");
    }

    #[test]
    fn test_derivation_storage() {
        let tmp_dir = TempDir::new().expect("create temp dir");
        let db_path = tmp_dir.path().join("chaindb_derivation");
        let db = ChainDb::new(&db_path).expect("create db");

        // Create dummy derived block pair
        let derived_pair = DerivedRefPair {
            source: BlockInfo {
                hash: B256::from([0u8; 32]),
                number: 100,
                parent_hash: B256::from([1u8; 32]),
                timestamp: 0,
            },
            derived: BlockInfo {
                hash: B256::from([2u8; 32]),
                number: 1,
                parent_hash: B256::from([3u8; 32]),
                timestamp: 0,
            },
        };

        // Save derived block pair
        db.save_derived_block_pair(derived_pair.clone()).expect("save derived pair");

        // Retrieve latest derived block pair
        let latest_pair = db.latest_derived_block_pair().expect("get latest derived pair");
        assert_eq!(latest_pair, derived_pair, "Latest derived pair should match saved pair");

        // Retrieve derived to source mapping
        let derived_block_id =
            BlockNumHash::new(derived_pair.derived.number, derived_pair.derived.hash);
        let source_block = db.derived_to_source(derived_block_id).expect("get derived to source");
        assert_eq!(
            source_block, derived_pair.source,
            "Source block should match derived pair source"
        );

        // Retrieve latest derived block at source
        let source_block_id =
            BlockNumHash::new(derived_pair.source.number, derived_pair.source.hash);
        let latest_derived = db
            .latest_derived_block_at_source(source_block_id)
            .expect("get latest derived at source");
        assert_eq!(
            latest_derived, derived_pair.derived,
            "Latest derived block at source should match derived pair derived"
        );
    }
}
