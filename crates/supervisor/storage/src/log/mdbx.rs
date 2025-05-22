//! Reth's MDBX-backed abstraction of [`LogStorage`](crate::log::LogStorage) for superchain state.
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
    error::StorageError,
    log::{LogStorageReader, LogStorageWriter},
    models::{BlockRefs, LogEntries},
};
use kona_protocol::BlockInfo;
use kona_supervisor_types::Log;
use reth_db_api::{
    cursor::{DbCursorRO, DbDupCursorRO, DbDupCursorRW},
    transaction::{DbTx, DbTxMut},
};
use std::fmt::Debug;
use tracing::{debug, error};

/// A log storage that wraps a transactional reference to the MDBX backend.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct MdbxLogStorage<'tx, TX> {
    tx: &'tx TX,
}

/// Internal constructor and setup methods for [`MdbxLogStorage`].
impl<'tx, TX> MdbxLogStorage<'tx, TX> {
    #[allow(dead_code)]
    pub(crate) const fn new(tx: &'tx TX) -> Self {
        Self { tx }
    }
}

/// Implements the [`LogStorageWriter`] trait
impl<TX> LogStorageWriter for MdbxLogStorage<'_, TX>
where
    TX: DbTxMut,
{
    type Error = StorageError;

    fn store_block_logs(&self, block: &BlockInfo, logs: Vec<Log>) -> Result<(), Self::Error> {
        debug!(target: "supervisor_storage", "Storing {} logs for block {}", logs.len(), block.number);
        let tx = self.tx;
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
        Ok(())
    }
}
/// Implements the [`LogStorageReader`] trait
impl<TX> LogStorageReader for MdbxLogStorage<'_, TX>
where
    TX: DbTx,
{
    type Error = StorageError;
    fn get_block(&self, block_number: u64) -> Result<BlockInfo, Self::Error> {
        debug!(target: "supervisor_storage", "Fetching block {}", block_number);

        let block_option = self.tx.get::<BlockRefs>(block_number).map_err(|e| {
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

        let mut cursor = self.tx.cursor_read::<BlockRefs>().map_err(|e| {
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

        let mut cursor = self.tx.cursor_dup_read::<LogEntries>().map_err(|e| {
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

        let block_option = self.tx.get::<BlockRefs>(block_number).map_err(|e| {
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

        let mut cursor = self.tx.cursor_dup_read::<LogEntries>().map_err(|e| {
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
    use reth_db::mdbx::{DatabaseArguments, init_db_for};
    use reth_db_api::Database;
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
        let db =
            init_db_for::<_, crate::models::Tables>(temp_dir.path(), DatabaseArguments::default())
                .expect("Failed to init database");

        let tx_mut = db.tx_mut().expect("Failed to start RW tx");
        let log_writer = MdbxLogStorage::new(&tx_mut);

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
        log_writer.store_block_logs(&block1, logs1.clone()).expect("Failed to store logs1");
        log_writer.store_block_logs(&block2, logs2.clone()).expect("Failed to store logs2");
        log_writer.store_block_logs(&block3, logs3).expect("Failed to store logs3");

        tx_mut.commit().expect("Failed to commit tx");

        let tx = db.tx().expect("Failed to start RO tx");
        let log_reader = MdbxLogStorage::new(&tx);

        // get_block
        let block = log_reader.get_block(block2.number).expect("Failed to get block");
        assert_eq!(block, block2);

        // get_latest_block
        let block = log_reader.get_latest_block().expect("Failed to get latest block");
        assert_eq!(block, block3);

        // get_block_by_log
        let block =
            log_reader.get_block_by_log(1, &logs1[1].clone()).expect("Failed to get block by log");
        assert_eq!(block, block1);

        // get_logs
        let logs = log_reader.get_logs(block2.number).expect("Failed to get logs");
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], logs2[0]);
        assert_eq!(logs[1], logs2[1]);
    }

    #[test]
    fn test_not_found_error_and_empty_results() {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        let db =
            init_db_for::<_, crate::models::Tables>(temp_dir.path(), DatabaseArguments::default())
                .expect("Failed to init database");

        let tx_mut = db.tx_mut().expect("Failed to start RW tx");
        let log_writer = MdbxLogStorage::new(&tx_mut);

        let tx = db.tx().expect("Failed to start RO tx");
        let log_reader = MdbxLogStorage::new(&tx);

        let err = log_reader.get_latest_block().unwrap_err();
        match err {
            StorageError::EntryNotFound(_) => { /* ok */ }
            _ => panic!("Expected EntryNotFound error"),
        }

        log_writer
            .store_block_logs(&sample_block_info(1), vec![sample_log(0, true)])
            .expect("Failed to store logs1");

        tx_mut.commit().expect("Failed to commit tx");

        let err = log_reader.get_block(0).unwrap_err();
        match err {
            StorageError::EntryNotFound(_) => { /* ok */ }
            _ => panic!("Expected EntryNotFound error"),
        }

        // should return empty logs but not an error
        let logs = log_reader.get_logs(2).expect("Should not return error");
        assert_eq!(logs.len(), 0);

        let err = log_reader.get_block_by_log(1, &sample_log(1, false)).unwrap_err();
        match err {
            StorageError::EntryNotFound(_) => { /* ok */ }
            _ => panic!("Expected EntryNotFound error"),
        }
    }
}
