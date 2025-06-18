//! Main database access structure and transaction contexts.

use crate::{
    error::StorageError,
    providers::{DerivationProvider, LogProvider, SafetyHeadRefProvider},
    traits::{
        DerivationStorageReader, DerivationStorageWriter, HeadRefStorageReader,
        HeadRefStorageWriter, LogStorageReader, LogStorageWriter,
    },
};
use alloy_eips::eip1898::BlockNumHash;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_types::{Log, SuperHead};
use op_alloy_consensus::interop::SafetyLevel;
use reth_db::{
    DatabaseEnv,
    mdbx::{DatabaseArguments, init_db_for},
};
use reth_db_api::database::Database;
use std::{path::Path, sync::RwLock};
use tracing::{error, warn};

/// Manages the database environment for a single chain.
/// Provides transactional access to data via providers.
#[derive(Debug)]
pub struct ChainDb {
    env: DatabaseEnv,
    /// Current L1 block reference, used for tracking the latest L1 block processed.
    /// In-memory only, not persisted.
    current_l1: RwLock<Option<BlockInfo>>,
}

impl ChainDb {
    /// Creates or opens a database environment at the given path.
    pub fn new(path: &Path) -> Result<Self, StorageError> {
        let env = init_db_for::<_, crate::models::Tables>(path, DatabaseArguments::default())?;
        Ok(Self { env, current_l1: RwLock::new(None) })
    }

    /// initialises the database with a given anchor derived block pair.
    pub fn initialise(&self, anchor: DerivedRefPair) -> Result<(), StorageError> {
        self.env.update(|tx| {
            DerivationProvider::new(tx).initialise(anchor)?;
            LogProvider::new(tx).initialise(anchor.derived)?;

            let sp = SafetyHeadRefProvider::new(tx);
            // todo: cross check if we can consider following safety head ref update
            sp.update_safety_head_ref(SafetyLevel::LocalUnsafe, &anchor.derived)?;
            sp.update_safety_head_ref(SafetyLevel::CrossUnsafe, &anchor.derived)?;
            sp.update_safety_head_ref(SafetyLevel::LocalSafe, &anchor.derived)?;
            sp.update_safety_head_ref(SafetyLevel::CrossSafe, &anchor.derived)
        })?
    }

    pub(super) fn update_finalized_head_ref(
        &self,
        l1_finalized: BlockInfo,
    ) -> Result<(), StorageError> {
        self.env.update(|tx| {
            let sp = SafetyHeadRefProvider::new(tx);
            let safe = sp.get_safety_head_ref(SafetyLevel::CrossSafe)?;

            let dp = DerivationProvider::new(tx);
            let safe_block_pair = dp.get_derived_block_pair(safe.id())?;

            if l1_finalized.number >= safe_block_pair.source.number {
                // this could happen during initial sync
                warn!(
                    target: "supervisor_storage",
                    l1_finilized_block_number = l1_finalized.number,
                    safe_source_block_number = safe_block_pair.source.number,
                    "L1 finalized block is greater than safe block",
                );
                return sp.update_safety_head_ref(SafetyLevel::Finalized, &safe);
            }

            let latest_derived = dp.latest_derived_block_at_source(l1_finalized.id())?;
            sp.update_safety_head_ref(SafetyLevel::Finalized, &latest_derived)
        })?
    }
}

impl DerivationStorageReader for ChainDb {
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
}

impl DerivationStorageWriter for ChainDb {
    // Todo: better name save_derived_block_pair
    fn save_derived_block_pair(&self, incoming_pair: DerivedRefPair) -> Result<(), StorageError> {
        self.env.update(|ctx| {
            let derived_block = incoming_pair.derived;
            let block =
                LogProvider::new(ctx).get_block(derived_block.number).map_err(|err| match err {
                    StorageError::EntryNotFound(_) => StorageError::ConflictError(
                        "conflict between unsafe block and derived block".to_string(),
                    ),
                    other => other, // propagate other errors as-is
                })?;

            if block != derived_block {
                return Err(StorageError::ConflictError(
                    "conflict between unsafe block and derived block".to_string(),
                ));
            }
            DerivationProvider::new(ctx).save_derived_block_pair(incoming_pair)?;
            SafetyHeadRefProvider::new(ctx)
                .update_safety_head_ref(SafetyLevel::LocalSafe, &incoming_pair.derived)
        })?
    }
}

impl LogStorageReader for ChainDb {
    fn get_latest_block(&self) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| LogProvider::new(tx).get_latest_block())?
    }

    fn get_block(&self, block_number: u64) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| LogProvider::new(tx).get_block(block_number))?
    }

    fn get_log(&self, block_number: u64, log_index: u32) -> Result<Log, StorageError> {
        self.env.view(|tx| LogProvider::new(tx).get_log(block_number, log_index))?
    }

    fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, StorageError> {
        self.env.view(|tx| LogProvider::new(tx).get_logs(block_number))?
    }
}

impl LogStorageWriter for ChainDb {
    fn store_block_logs(&self, block: &BlockInfo, logs: Vec<Log>) -> Result<(), StorageError> {
        self.env.update(|ctx| {
            LogProvider::new(ctx).store_block_logs(block, logs)?;

            SafetyHeadRefProvider::new(ctx).update_safety_head_ref(SafetyLevel::LocalUnsafe, block)
        })?
    }
}

impl HeadRefStorageReader for ChainDb {
    fn get_current_l1(&self) -> Result<BlockInfo, StorageError> {
        let guard = self.current_l1.read().map_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to acquire read lock on current_l1");
            StorageError::LockPoisoned
        })?;
        guard.as_ref().cloned().ok_or(StorageError::FutureData)
    }

    fn get_safety_head_ref(&self, safety_level: SafetyLevel) -> Result<BlockInfo, StorageError> {
        self.env.view(|tx| SafetyHeadRefProvider::new(tx).get_safety_head_ref(safety_level))?
    }

    /// Fetches all safety heads and current L1 state
    fn get_super_head(&self) -> Result<SuperHead, StorageError> {
        let l1_source = self.get_current_l1()?;

        self.env.view(|tx| {
            let sp = SafetyHeadRefProvider::new(tx);
            let local_unsafe = sp.get_safety_head_ref(SafetyLevel::LocalUnsafe)?;
            let cross_unsafe = sp.get_safety_head_ref(SafetyLevel::CrossUnsafe)?;
            let local_safe = sp.get_safety_head_ref(SafetyLevel::LocalSafe)?;
            let cross_safe = sp.get_safety_head_ref(SafetyLevel::CrossSafe)?;
            let finalized = sp.get_safety_head_ref(SafetyLevel::Finalized)?;

            Ok(SuperHead {
                l1_source,
                local_unsafe,
                cross_unsafe,
                local_safe,
                cross_safe,
                finalized,
            })
        })?
    }
}

impl HeadRefStorageWriter for ChainDb {
    fn update_current_l1(&self, block: BlockInfo) -> Result<(), StorageError> {
        let mut guard = self
            .current_l1
            .write()
            .map_err(|err| {
                error!(target: "supervisor_storage", %err, "Failed to acquire write lock on current_l1" );
                StorageError::LockPoisoned
            })?;

        // Check if the new block number is greater than the current L1 block
        if let Some(ref current) = *guard {
            if block.number <= current.number {
                error!(target: "supervisor_storage",
                    current_block_number = current.number,
                    new_block_number = block.number,
                    "New L1 block number is not greater than current L1 block number",
                );
                return Err(StorageError::BlockOutOfOrder);
            }
        }
        *guard = Some(block);
        Ok(())
    }

    fn update_safety_head_ref(
        &self,
        safety_level: SafetyLevel,
        block: &BlockInfo,
    ) -> Result<(), StorageError> {
        self.env.update(|ctx| {
            SafetyHeadRefProvider::new(ctx).update_safety_head_ref(safety_level, block)
        })?
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

        let anchor = DerivedRefPair {
            source: BlockInfo {
                hash: B256::from([0u8; 32]),
                number: 100,
                parent_hash: B256::from([1u8; 32]),
                timestamp: 0,
            },
            derived: BlockInfo {
                hash: B256::from([2u8; 32]),
                number: 0,
                parent_hash: B256::from([3u8; 32]),
                timestamp: 0,
            },
        };

        db.initialise(anchor).expect("initialise db");

        let block = BlockInfo {
            hash: B256::from([4u8; 32]),
            number: 1,
            parent_hash: anchor.derived.hash,
            timestamp: 0,
        };
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

        let log = db.get_log(block.number, 1).expect("get block by log");
        assert_eq!(log, logs[1], "Block by log should match stored block");
    }

    #[test]
    fn test_derivation_storage() {
        let tmp_dir = TempDir::new().expect("create temp dir");
        let db_path = tmp_dir.path().join("chaindb_derivation");
        let db = ChainDb::new(&db_path).expect("create db");

        let anchor = DerivedRefPair {
            source: BlockInfo {
                hash: B256::from([0u8; 32]),
                number: 100,
                parent_hash: B256::from([1u8; 32]),
                timestamp: 0,
            },
            derived: BlockInfo {
                hash: B256::from([2u8; 32]),
                number: 0,
                parent_hash: B256::from([3u8; 32]),
                timestamp: 0,
            },
        };

        // Create dummy derived block pair
        let derived_pair = DerivedRefPair {
            source: BlockInfo {
                hash: B256::from([4u8; 32]),
                number: 101,
                parent_hash: B256::from([5u8; 32]),
                timestamp: 0,
            },
            derived: BlockInfo {
                hash: B256::from([6u8; 32]),
                number: 1,
                parent_hash: anchor.derived.hash,
                timestamp: 0,
            },
        };

        // Initialise the database with the anchor derived block pair
        db.initialise(anchor).expect("initialise db with anchor");

        // Save derived block pair - should error conflict
        let err = db.save_derived_block_pair(derived_pair).unwrap_err();
        assert!(matches!(err, StorageError::ConflictError(_)));

        db.store_block_logs(
            &BlockInfo {
                hash: B256::from([6u8; 32]),
                number: 1,
                parent_hash: anchor.derived.hash,
                timestamp: 0,
            },
            vec![],
        )
        .expect("storing logs failed");

        // Save derived block pair
        db.save_derived_block_pair(derived_pair).expect("save derived pair");

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

    #[test]
    fn test_safety_head_ref_storage() {
        let tmp_dir = TempDir::new().expect("create temp dir");
        let db_path = tmp_dir.path().join("chaindb_safety_head");
        let db = ChainDb::new(&db_path).expect("create db");

        // Create test blocks for different safety levels
        let unsafe_block = BlockInfo {
            hash: B256::from([0u8; 32]),
            number: 100,
            parent_hash: B256::from([1u8; 32]),
            timestamp: 0,
        };
        let safe_block = BlockInfo {
            hash: B256::from([2u8; 32]),
            number: 99,
            parent_hash: B256::from([3u8; 32]),
            timestamp: 0,
        };
        let finalized_block = BlockInfo {
            hash: B256::from([4u8; 32]),
            number: 98,
            parent_hash: B256::from([5u8; 32]),
            timestamp: 0,
        };

        // Test optimistic safety level
        db.update_safety_head_ref(SafetyLevel::LocalUnsafe, &unsafe_block)
            .expect("update unsafe head");
        let retrieved_unsafe =
            db.get_safety_head_ref(SafetyLevel::LocalUnsafe).expect("get unsafe head");
        assert_eq!(
            retrieved_unsafe, unsafe_block,
            "Retrieved unsafe head should match stored block"
        );

        // Test safe safety level
        db.update_safety_head_ref(SafetyLevel::CrossSafe, &safe_block).expect("update safe head");
        let retrieved_safe = db.get_safety_head_ref(SafetyLevel::CrossSafe).expect("get safe head");
        assert_eq!(retrieved_safe, safe_block, "Retrieved safe head should match stored block");

        // Test finalized safety level
        db.update_safety_head_ref(SafetyLevel::Finalized, &finalized_block)
            .expect("update finalized head");
        let retrieved_finalized =
            db.get_safety_head_ref(SafetyLevel::Finalized).expect("get finalized head");
        assert_eq!(
            retrieved_finalized, finalized_block,
            "Retrieved finalized head should match stored block"
        );
    }

    #[test]
    fn test_get_super_head() {
        let tmp_dir = TempDir::new().expect("create temp dir");
        let db_path = tmp_dir.path().join("chaindb_super_head");
        let db = ChainDb::new(&db_path).expect("create db");

        // Create anchor block
        let anchor = DerivedRefPair {
            source: BlockInfo {
                hash: B256::from([0u8; 32]),
                number: 100,
                parent_hash: B256::from([1u8; 32]),
                timestamp: 0,
            },
            derived: BlockInfo {
                hash: B256::from([2u8; 32]),
                number: 0,
                parent_hash: B256::from([3u8; 32]),
                timestamp: 0,
            },
        };

        db.initialise(anchor).expect("initialise db with anchor");

        // Test blocks for each safety level
        let l1_block = BlockInfo {
            hash: B256::from([10u8; 32]),
            number: 101,
            parent_hash: anchor.source.hash,
            timestamp: 0,
        };

        let local_unsafe_block = BlockInfo {
            hash: B256::from([20u8; 32]),
            number: 1,
            parent_hash: anchor.derived.hash,
            timestamp: 0,
        };

        let cross_unsafe_block = BlockInfo {
            hash: B256::from([30u8; 32]),
            number: 2,
            parent_hash: local_unsafe_block.hash,
            timestamp: 0,
        };

        let local_safe_block = BlockInfo {
            hash: B256::from([40u8; 32]),
            number: 3,
            parent_hash: cross_unsafe_block.hash,
            timestamp: 0,
        };

        let cross_safe_block = BlockInfo {
            hash: B256::from([50u8; 32]),
            number: 4,
            parent_hash: local_safe_block.hash,
            timestamp: 0,
        };

        let finalized_block = BlockInfo {
            hash: B256::from([60u8; 32]),
            number: 5,
            parent_hash: cross_safe_block.hash,
            timestamp: 0,
        };

        // Set current L1
        db.update_current_l1(l1_block).expect("update current L1");

        // Test safety head refs
        db.update_safety_head_ref(SafetyLevel::LocalUnsafe, &local_unsafe_block)
            .expect("update unsafe head");
        db.update_safety_head_ref(SafetyLevel::LocalSafe, &local_safe_block)
            .expect("update local safe head");
        db.update_safety_head_ref(SafetyLevel::CrossSafe, &cross_safe_block)
            .expect("update safe head");
        db.update_safety_head_ref(SafetyLevel::Finalized, &finalized_block)
            .expect("update finalized head");
        db.update_safety_head_ref(SafetyLevel::CrossUnsafe, &cross_unsafe_block)
            .expect("update cross unsafe head");

        // Retrieve super head
        let super_head = db.get_super_head().expect("get super head");

        assert_eq!(super_head.l1_source, l1_block, "L1 source should match");
        assert_eq!(super_head.local_unsafe, local_unsafe_block, "Local unsafe should match");
        assert_eq!(super_head.cross_unsafe, cross_unsafe_block, "Cross unsafe should match");
        assert_eq!(super_head.local_safe, local_safe_block, "Local safe should match");
        assert_eq!(super_head.cross_safe, cross_safe_block, "Cross safe should match");
        assert_eq!(super_head.finalized, finalized_block, "Finalized should match");
    }

    #[test]
    fn test_update_and_get_current_l1() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let db_path = tmp_dir.path().join("chaindb_current_l1");
        let db = ChainDb::new(&db_path).unwrap();

        let block1 = BlockInfo { number: 10, ..Default::default() };
        let block2 = BlockInfo { number: 20, ..Default::default() };

        // Initially, get_current_l1 should return FutureData error
        let err = db.get_current_l1().unwrap_err();
        assert!(matches!(err, StorageError::FutureData));

        // Update current_l1 with block1
        db.update_current_l1(block1).unwrap();
        let got = db.get_current_l1().unwrap();
        assert_eq!(got, block1);

        // Update with a higher block number
        db.update_current_l1(block2).unwrap();
        let got = db.get_current_l1().unwrap();
        assert_eq!(got, block2);

        // Update with a lower block number should error
        let block3 = BlockInfo { number: 15, ..Default::default() };
        let err = db.update_current_l1(block3).unwrap_err();
        assert!(matches!(err, StorageError::BlockOutOfOrder));
    }
}
