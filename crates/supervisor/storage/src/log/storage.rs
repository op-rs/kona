use std::path::Path;
use reth_db::{DatabaseError};
use reth_db::database::{Database};
use reth_db::transaction::{DbTx, DbTxMut};
use reth_db::cursor::{DbCursorRO, DbCursorRW};
use reth_db::mdbx::{init_db_for, DatabaseArguments, DatabaseEnv};
use crate::log::models::{LogEntries, LogKey, LogValue};
use crate::log::table::LogStorageTables;

/// Minimal LogStorage with open/insert/get capabilities.
pub struct LogStorage {
    db: DatabaseEnv,
}

impl LogStorage {
    /// Open or create the database at the given path.
    pub fn init<P: AsRef<Path>>(path: P, args: DatabaseArguments) -> eyre::Result<Self> {
        let db = init_db_for::<_,LogStorageTables>(path, args)?;
        Ok(Self { db })
    }

    /// Insert a log entry.
    pub fn insert_log(&self, key: LogKey, value: LogValue) -> Result<bool, DatabaseError> {
        let mut tx = self.db.tx_mut()?;
        tx.put::<LogEntries>(key, value)?;
        tx.commit()
    }

    /// Retrieve a log entry by key.
    pub fn get_log(&self, key: LogKey) -> Result<Option<LogValue>, DatabaseError> {
        let tx = self.db.tx()?;
        tx.get::<LogEntries>(key)
    }
    /// Insert multiple logs for a single block in a single transaction.
    pub fn insert_logs(
        &self,
        chain_id: u64,
        block_number: u64,
        logs: impl IntoIterator<Item = (u32, LogValue)>
    ) -> Result<bool, DatabaseError> {
        let mut tx = self.db.tx_mut()?;

        for (log_index, log_value) in logs {
            let key = LogKey {
                chain_id,
                block_number,
                log_index,
            };
            tx.put::<LogEntries>(key, log_value)?;
        }

        tx.commit()
    }

    /// Get all logs in a block (sorted by log_index).
    pub fn get_logs_in_block(
        &self,
        chain_id: u64,
        block_number: u64,
    ) -> Result<Vec<LogValue>, reth_db_api::DatabaseError> {
        let tx = self.db.tx()?;

        let start_key = LogKey {
            chain_id,
            block_number,
            log_index: 0,
        };
        let end_key = LogKey {
            chain_id,
            block_number,
            log_index: u32::MAX,
        };

        let mut cursor = tx.cursor_read::<LogEntries>()?;
        let mut walker = cursor.walk_range(start_key..=end_key)?;

        let mut logs = Vec::new();
        for row in walker {
            let (_, value) = row?;
            logs.push(value);
        }

        Ok(logs)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use alloy_primitives::{hex, B256};
    use rand::Rng;

    fn random_b256() -> B256 {
        let mut bytes = [0u8; 32];
        rand::rng().fill(&mut bytes);
        B256::from(bytes)
    }

    #[test]
    fn test_insert_and_get_log() {
        let tmp_dir = tempdir().expect("failed to create temp dir");
        let db_args = DatabaseArguments::default();
        let storage = LogStorage::init(tmp_dir.path(), db_args).expect("db init failed");

        let key = LogKey {
            chain_id: 1,
            block_number: 100000000,
            log_index: 0,
        };

        let value = LogValue {
            hash: B256::from([0xAB; 32]),
            executing_message_hash: B256::from([0xAB; 32]),
            timestamp: 1000000000,
        };

        storage.insert_log(key.clone(), value.clone()).expect("insert failed");

        let loaded = storage.get_log(key.clone()).expect("get failed");

        assert_eq!(loaded, Some(value));
    }

    #[test]
    fn test_insert_and_get_logs_multiple_chains_blocks() {
        let tmp_dir = tempdir().expect("failed to create temp dir");
        let db_args = DatabaseArguments::default();
        let storage = LogStorage::init(tmp_dir.path(), db_args).expect("db init failed");

        let test_data = vec![
            (1, 100), // chain 1, block 100
            (1, 101), // chain 1, block 101
            (2, 200), // chain 2, block 200
        ];

        let mut expected_map = std::collections::HashMap::new();

        for (chain_id, block_number) in &test_data {
            let logs: Vec<(u32, LogValue)> = (0..3).map(|i| {
                (i, LogValue {
                    hash: random_b256(),
                    executing_message_hash: random_b256(),
                    timestamp: 10000000 + i as u64,
                })
            }).collect();

            storage.insert_logs(*chain_id, *block_number, logs.clone()).expect("batch insert failed");
            expected_map.insert((*chain_id, *block_number), logs);
        }

        // Validate each set of inserted logs
        for (chain_id, block_number) in &test_data {
            let fetched_logs = storage.get_logs_in_block(*chain_id, *block_number).expect("fetch failed");
            let expected_logs = expected_map.get(&(*chain_id, *block_number)).unwrap();
            let expected_values: Vec<LogValue> = expected_logs.iter().map(|(_, val)| val.clone()).collect();
            assert_eq!(fetched_logs, expected_values, "Mismatch for chain {chain_id}, block {block_number}");
        }
    }
}