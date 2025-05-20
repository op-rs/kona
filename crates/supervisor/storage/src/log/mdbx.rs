//! Reth's MDBX-backed implementation of [`LogStorage`] for supervisor state.
//!
//! This module provides the [`MdbxLogStorage`] struct, which uses the
//! [`reth-db`] abstraction of reth to store execution logs
//! and block metadata required by the Optimism supervisor.
//!
//! It supports:
//! - Writing full blocks of logs with metadata
//! - Retrieving block metadata by number
//! - Finding a block from a specific log (with hash/index match)
//! - Fetching logs per block using dup-sorted key layout
//!
//! Logs are stored in [`LogEntries`] under dup-sorted tables, with log index
//! used as the subkey. Block metadata is stored in [`BlockRefs`].

use crate::{
    error::{SourceError, StorageError},
    log::LogStorage,
    models::{BlockRefs, LogEntries},
};
use kona_protocol::BlockInfo;
use kona_supervisor_types::Log;
use reth_db::{
    DatabaseEnv,
    mdbx::{DatabaseArguments, init_db_for},
};
use reth_db_api::{
    Database,
    cursor::{DbCursorRO, DbDupCursorRO, DbDupCursorRW},
    transaction::{DbTx, DbTxMut},
};
use std::{fmt::Debug, path::Path};
use tracing::{debug, error};

/// Reth's MDBX-backed log storage implementation for the supervisor.
///
/// This wraps a [`DatabaseEnv`] from `reth-db` and provides durable storage
/// for L2 execution logs and block metadata using dup-sorted tables.
///
/// Used by the [`LogStorage`] trait to implement log ingestion, lookup,
/// and block metadata resolution in the context of Optimism state tracking.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct MdbxLogStorage {
    db: DatabaseEnv,
}

/// Internal constructor and setup methods for [`MdbxLogStorage`].
impl MdbxLogStorage {
    #[allow(dead_code)]
    pub(crate) fn init<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let db = init_db_for::<_, crate::models::Tables>(path, DatabaseArguments::default())
            .map_err(|e| StorageError::DatabaseInit(SourceError::from(e)))?;
        Ok(Self { db })
    }
}

/// Implements the [`LogStorage`] trait
impl LogStorage for MdbxLogStorage {
    type Error = StorageError;

    fn store_block_logs(&self, block: &BlockInfo, logs: Vec<Log>) -> Result<(), Self::Error> {
        debug!(target: "supervisor_storage", "Storing {} logs for block {}", logs.len(), block.number);

        let tx = self.db.tx_mut().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to start transaction: {e}");
            StorageError::TransactionInit(e.into())
        })?;

        if let Err(e) = tx.put::<BlockRefs>(block.number, (*block).into()) {
            error!(target: "supervisor_storage", "Failed to write block for {}: {e}", block.number);
            return Err(StorageError::DatabaseWrite(e.into()));
        }

        let mut cursor = tx.cursor_dup_write::<LogEntries>().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to get dup cursor: {e}");
            StorageError::CursorInit(e.into())
        })?;

        for log in logs {
            if let Err(e) = cursor.append_dup(block.number, log.into()) {
                error!(target: "supervisor_storage", "Failed to append log for block {}: {e}", block.number);
                return Err(StorageError::DatabaseWrite(e.into()));
            }
        }

        tx.commit().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to commit transaction for block {}: {e}", block.number);
            StorageError::TransactionCommit(e.into())
        })?;
        Ok(())
    }

    fn get_block(&self, block_number: u64) -> Result<BlockInfo, Self::Error> {
        debug!(target: "supervisor_storage", "Fetching block {}", block_number);

        let tx = self.db.tx().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to start read transaction: {e}");
            StorageError::TransactionInit(e.into())
        })?;

        let block_option = tx.get::<BlockRefs>(block_number).map_err(|e| {
            error!(target: "supervisor_storage", "Failed to read block {}: {e}", block_number);
            StorageError::DatabaseRead(e.into())
        })?;

        let block = block_option.ok_or_else(|| {
            error!(target: "supervisor_storage", "Block {} not found", block_number);
            StorageError::EntryNotFound(format!("Block {} not found", block_number))
        })?;
        Ok(block.into())
    }

    fn get_latest_block(&self) -> Result<BlockInfo, Self::Error> {
        debug!(target: "supervisor_storage", "Fetching latest block");

        let tx = self.db.tx().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to start read transaction: {e}");
            StorageError::TransactionInit(e.into())
        })?;

        let mut cursor = tx.cursor_read::<BlockRefs>().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to get cursor for Block: {e}");
            StorageError::CursorInit(e.into())
        })?;

        let result = cursor.last().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to seek to last block: {e}");
            StorageError::DatabaseRead(e.into())
        })?;

        let (_, block) = result.ok_or_else(|| {
            error!(target: "supervisor_storage", "No blocks found in storage");
            StorageError::EntryNotFound("No blocks found".to_string())
        })?;
        Ok(block.into())
    }

    fn get_block_by_log(&self, block_number: u64, log: &Log) -> Result<BlockInfo, Self::Error> {
        debug!(target: "supervisor_storage", "Fetching block {} by log index {}", block_number, log.index);

        let tx = self.db.tx().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to start read transaction: {e}");
            StorageError::TransactionInit(e.into())
        })?;

        let mut cursor = tx.cursor_dup_read::<LogEntries>().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to get cursor for LogEntries: {e}");
            StorageError::CursorInit(e.into())
        })?;

        let result = cursor
            .seek_by_key_subkey(block_number, log.index)
            .map_err(|e| {
                error!(target: "supervisor_storage", "Failed to seek log (block {}, index {}): {e}", block_number, log.index);
                StorageError::DatabaseRead(e.into())
            })?;

        let log_entry = result.ok_or_else(|| {
            error!(target: "supervisor_storage", "Log not found at block {} index {}", block_number, log.index);
            StorageError::EntryNotFound(format!("Log not found at block {} index {}", block_number, log.index))
        })?;

        if log_entry.hash != log.hash {
            error!(target: "supervisor_storage", "Log hash mismatch at block {} index {}", block_number, log.index);
            return Err(StorageError::EntryNotFound("Log hash mismatch".to_string()));
        }

        let block_option = tx.get::<BlockRefs>(block_number).map_err(|e| {
            error!(target: "supervisor_storage", "Failed to read block {}: {e}", block_number);
            StorageError::DatabaseRead(e.into())
        })?;

        let block = block_option.ok_or_else(|| {
            error!(target: "supervisor_storage", "Block {} not found", block_number);
            StorageError::EntryNotFound(format!("Block {} not found", block_number))
        })?;
        Ok(block.into())
    }

    fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, Self::Error> {
        debug!(target: "supervisor_storage", "Fetching logs for block {}", block_number);

        let tx = self.db.tx().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to start read transaction: {e}");
            StorageError::TransactionInit(e.into())
        })?;

        let mut cursor = tx.cursor_dup_read::<LogEntries>().map_err(|e| {
            error!(target: "supervisor_storage", "Failed to get dup cursor for block {}: {e}", block_number);
            StorageError::CursorInit(e.into())
        })?;

        let walker = cursor.walk_range(block_number..=block_number).map_err(|e| {
            error!(target: "supervisor_storage", "Failed to walk dup range for block {}: {e}", block_number);
            StorageError::DatabaseRead(e.into())
        })?;

        let mut logs = Vec::new();
        for row in walker {
            match row {
                Ok((_, entry)) => logs.push(entry.into()),
                Err(e) => {
                    error!(target: "supervisor_storage", "Failed to read log entry for block {}: {e}", block_number);
                    return Err(StorageError::DatabaseRead(e.into()));
                }
            }
        }
        Ok(logs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use kona_protocol::BlockInfo;
    use kona_supervisor_types::{ExecutingMessage, Log};
    use tempfile::TempDir;

    fn sample_block_info(block_number: u64) -> BlockInfo {
        BlockInfo {
            number: block_number,
            hash: B256::from([0x11; 32]),
            parent_hash: B256::from([0x22; 32]),
            timestamp: 123456,
        }
    }

    fn sample_log(log_index: u32, with_msg: bool) -> Log {
        Log {
            index: log_index,
            hash: B256::from([log_index as u8; 32]),
            executing_message: if with_msg {
                Some(ExecutingMessage {
                    chain_id: 10,
                    block_number: 999,
                    log_index: 7,
                    hash: B256::from([0x44; 32]),
                    timestamp: 88888,
                })
            } else {
                None
            },
        }
    }

    #[test]
    fn test_storage_read_write_success() {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        let storage = MdbxLogStorage::init(temp_dir.path()).expect("Failed to init MdbxLogStorage");

        let block1 = sample_block_info(1);
        let logs1 = vec![
            sample_log(0, false),
            sample_log(1, true),
            sample_log(3, false),
            sample_log(4, true),
        ];

        let block2 = sample_block_info(2);
        let logs2 = vec![sample_log(0, false), sample_log(1, true)];

        let block3 = sample_block_info(3);
        let logs3 = vec![sample_log(0, false), sample_log(1, true), sample_log(2, true)];

        // Store logs
        storage.store_block_logs(&block1, logs1.clone()).expect("Failed to store logs1");
        storage.store_block_logs(&block2, logs2.clone()).expect("Failed to store logs2");
        storage.store_block_logs(&block3, logs3).expect("Failed to store logs3");

        // get_block
        let block = storage.get_block(block2.number).expect("Failed to get block");
        assert_eq!(block, block2);

        // get_latest_block
        let block = storage.get_latest_block().expect("Failed to get latest block");
        assert_eq!(block, block3);

        // get_block_by_log
        let block =
            storage.get_block_by_log(1, &logs1[1].clone()).expect("Failed to get block by log");
        assert_eq!(block, block1);

        // get_logs
        let logs = storage.get_logs(block2.number).expect("Failed to get logs");
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], logs2[0]);
        assert_eq!(logs[1], logs2[1]);
    }

    #[test]
    fn test_init_error_path_exercises_database_init() {
        let invalid_path = Path::new("/this/should/not/exist");
        let err = MdbxLogStorage::init(invalid_path).unwrap_err();
        match err {
            StorageError::DatabaseInit(_) => { /* ok */ }
            _ => panic!("Expected DatabaseInit error"),
        }
    }

    #[test]
    fn test_not_found_error_and_empty_results() {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        let storage = MdbxLogStorage::init(temp_dir.path()).expect("Failed to init MdbxLogStorage");

        let err = storage.get_latest_block().unwrap_err();
        match err {
            StorageError::EntryNotFound(_) => { /* ok */ }
            _ => panic!("Expected EntryNotFound error"),
        }

        storage
            .store_block_logs(&sample_block_info(1), vec![sample_log(0, true)])
            .expect("Failed to store logs1");

        let err = storage.get_block(0).unwrap_err();
        match err {
            StorageError::EntryNotFound(_) => { /* ok */ }
            _ => panic!("Expected EntryNotFound error"),
        }

        // should return empty logs but not an error
        let logs = storage.get_logs(2).expect("Should not return error");
        assert_eq!(logs.len(), 0);

        let err = storage.get_block_by_log(1, &sample_log(1, false)).unwrap_err();
        match err {
            StorageError::EntryNotFound(_) => { /* ok */ }
            _ => panic!("Expected EntryNotFound error"),
        }
    }
}
