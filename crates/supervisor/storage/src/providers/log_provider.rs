//! Reth's MDBX-backed abstraction of [`LogProvider`] for superchain state.
//!
//! This module provides the [`LogProvider`] struct, which uses the
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
    models::{BlockRefs, LogEntries},
};
use kona_protocol::BlockInfo;
use kona_supervisor_types::Log;
use reth_db_api::{
    cursor::{DbCursorRO, DbDupCursorRO, DbDupCursorRW},
    transaction::{DbTx, DbTxMut},
};
use std::fmt::Debug;
use tracing::{debug, error, warn};

/// A log storage that wraps a transactional reference to the MDBX backend.
#[derive(Debug)]
pub(crate) struct LogProvider<'tx, TX> {
    tx: &'tx TX,
}

/// Internal constructor and setup methods for [`LogProvider`].
impl<'tx, TX> LogProvider<'tx, TX> {
    pub(crate) const fn new(tx: &'tx TX) -> Self {
        Self { tx }
    }
}

impl<TX> LogProvider<'_, TX>
where
    TX: DbTxMut + DbTx,
{
    pub(crate) fn initialise(&self, anchor: BlockInfo) -> Result<(), StorageError> {
        match self.get_block(0) {
            Ok(block) if block.hash == anchor.hash => Ok(()),
            Ok(_) => Err(StorageError::InvalidAnchor),
            Err(StorageError::EntryNotFound(_)) => {
                self.store_block_logs_internal(&anchor, Vec::new())
            }

            Err(err) => Err(err),
        }
    }

    pub(crate) fn store_block_logs(
        &self,
        block: &BlockInfo,
        logs: Vec<Log>,
    ) -> Result<(), StorageError> {
        debug!(target: "supervisor_storage", block_number = block.number, "Storing logs");

        let latest_block = match self.get_latest_block() {
            Ok(block) => block,
            Err(StorageError::EntryNotFound(_)) => return Err(StorageError::DatabaseNotInitialised),
            Err(e) => return Err(e),
        };

        if !latest_block.is_parent_of(block) {
            warn!(
                target: "supervisor_storage",
                %latest_block,
                incoming_block = %block,
                "Incoming block does not follow latest stored block"
            );
            return Err(StorageError::BlockOutOfOrder);
        }

        self.store_block_logs_internal(block, logs)
    }

    fn store_block_logs_internal(
        &self,
        block: &BlockInfo,
        logs: Vec<Log>,
    ) -> Result<(), StorageError> {
        self.tx.put::<BlockRefs>(block.number, (*block).into()).inspect_err(|err| {
            error!(target: "supervisor_storage", block_number = block.number, %err, "Failed to insert block");
        })?;

        let mut cursor = self.tx.cursor_dup_write::<LogEntries>().inspect_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to get dup cursor");
        })?;

        for log in logs {
            cursor.append_dup(block.number, log.into()).inspect_err(|err| {
                error!(
                    target: "supervisor_storage",
                    block_number = block.number,
                    %err,
                    "Failed to append logs"
                );
            })?;
        }
        Ok(())
    }
}

impl<TX> LogProvider<'_, TX>
where
    TX: DbTx,
{
    pub(crate) fn get_block(&self, block_number: u64) -> Result<BlockInfo, StorageError> {
        debug!(target: "supervisor_storage", block_number, "Fetching block");

        let block_option = self.tx.get::<BlockRefs>(block_number).inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                block_number,
                %err,
                "Failed to read block",
            );
        })?;

        let block = block_option.ok_or_else(|| {
            warn!(target: "supervisor_storage", block_number, "Block not found");
            StorageError::EntryNotFound(format!("block {block_number} not found"))
        })?;
        Ok(block.into())
    }

    pub(crate) fn get_latest_block(&self) -> Result<BlockInfo, StorageError> {
        debug!(target: "supervisor_storage", "Fetching latest block");

        let mut cursor = self.tx.cursor_read::<BlockRefs>().inspect_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to get cursor");
        })?;

        let result = cursor.last().inspect_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to seek to last block");
        })?;

        let (_, block) = result.ok_or_else(|| {
            warn!(target: "supervisor_storage", "No blocks found in storage");
            StorageError::EntryNotFound("no blocks found".to_string())
        })?;
        Ok(block.into())
    }

    pub(crate) fn get_block_by_log(
        &self,
        block_number: u64,
        log: &Log,
    ) -> Result<BlockInfo, StorageError> {
        debug!(
            target: "supervisor_storage",
            block_number,
            log_index = log.index,
            "Fetching block  by log"
        );

        let mut cursor = self.tx.cursor_dup_read::<LogEntries>().inspect_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to get cursor for LogEntries");
        })?;

        let result = cursor.seek_by_key_subkey(block_number, log.index).inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                block_number,
                log_index = log.index,
                %err,
                "Failed to read log entry"
            );
        })?;

        let log_entry = result.ok_or_else(|| {
            warn!(
                target: "supervisor_storage",
                block_number,
                log_index = log.index,
                "Log not found"
            );
            StorageError::EntryNotFound(format!(
                "log not found at block {block_number} index {}",
                log.index,
            ))
        })?;

        if log_entry.hash != log.hash {
            warn!(
                target: "supervisor_storage",
                block_number,
                log_index = log.index,
                "Log hash mismatch"
            );
            return Err(StorageError::EntryNotFound("log hash mismatch".to_string()));
        }
        self.get_block(block_number)
    }

    pub(crate) fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, StorageError> {
        debug!(target: "supervisor_storage", block_number, "Fetching logs");

        let mut cursor = self.tx.cursor_dup_read::<LogEntries>().inspect_err(|err| {
            error!(target: "supervisor_storage", %err, "Failed to get dup cursor");
        })?;

        let walker = cursor.walk_range(block_number..=block_number).inspect_err(|err| {
            error!(
                target: "supervisor_storage",
                block_number,
                %err,
                "Failed to walk dup range",
            );
        })?;

        let mut logs = Vec::new();
        for row in walker {
            match row {
                Ok((_, entry)) => logs.push(entry.into()),
                Err(err) => {
                    error!(
                        target: "supervisor_storage",
                        block_number,
                        %err,
                        "Failed to read log entry",
                    );
                    return Err(StorageError::Database(err));
                }
            }
        }
        Ok(logs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Tables;
    use alloy_primitives::B256;
    use kona_protocol::BlockInfo;
    use kona_supervisor_types::{ExecutingMessage, Log};
    use reth_db::{
        DatabaseEnv,
        mdbx::{DatabaseArguments, init_db_for},
    };
    use reth_db_api::Database;
    use tempfile::TempDir;

    fn genesis_block() -> BlockInfo {
        BlockInfo {
            hash: B256::from([0u8; 32]),
            number: 0,
            parent_hash: B256::ZERO,
            timestamp: 100,
        }
    }

    fn sample_block_info(block_number: u64, parent_hash: B256) -> BlockInfo {
        BlockInfo {
            number: block_number,
            hash: B256::from([0x11; 32]),
            parent_hash,
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

    /// Sets up a new temp DB
    fn setup_db() -> DatabaseEnv {
        let temp_dir = TempDir::new().expect("Could not create temp dir");
        init_db_for::<_, Tables>(temp_dir.path(), DatabaseArguments::default())
            .expect("Failed to init database")
    }

    /// Helper to initialize database in a new transaction, committing if successful.
    fn initialize_db(db: &DatabaseEnv, block: &BlockInfo) -> Result<(), StorageError> {
        let tx = db.tx_mut().expect("Could not get mutable tx");
        let provider = LogProvider::new(&tx);
        let res = provider.initialise(*block);
        if res.is_ok() {
            tx.commit().expect("Failed to commit transaction");
        } else {
            tx.abort();
        }
        res
    }

    /// Helper to insert a pair in a new transaction, committing if successful.
    fn insert_block_logs(
        db: &DatabaseEnv,
        block: &BlockInfo,
        logs: Vec<Log>,
    ) -> Result<(), StorageError> {
        let tx = db.tx_mut().expect("Could not get mutable tx");
        let provider = LogProvider::new(&tx);
        let res = provider.store_block_logs(block, logs);
        if res.is_ok() {
            tx.commit().expect("Failed to commit transaction");
        }
        res
    }

    #[test]
    fn initialise_inserts_anchor_if_not_exists() {
        let db = setup_db();
        let genesis = genesis_block();

        // Should succeed and insert the anchor
        assert!(initialize_db(&db, &genesis).is_ok());

        // Check that the anchor is present
        let tx = db.tx().expect("Could not get tx");
        let provider = LogProvider::new(&tx);
        let stored = provider.get_block(genesis.number).expect("should exist");
        assert_eq!(stored.hash, genesis.hash);
    }

    #[test]
    fn initialise_is_idempotent_if_anchor_matches() {
        let db = setup_db();
        let genesis = genesis_block();

        // First initialise
        assert!(initialize_db(&db, &genesis).is_ok());

        // Second initialise with the same anchor should succeed (idempotent)
        assert!(initialize_db(&db, &genesis).is_ok());
    }

    #[test]
    fn initialise_fails_if_anchor_mismatch() {
        let db = setup_db();

        // Initialize with the genesis block
        let genesis = genesis_block();
        assert!(initialize_db(&db, &genesis).is_ok());

        // Try to initialise with a different anchor (different hash)
        let mut wrong_genesis = genesis;
        wrong_genesis.hash = B256::from([42u8; 32]);

        let result = initialize_db(&db, &wrong_genesis);
        assert!(matches!(result, Err(StorageError::InvalidAnchor)));
    }

    #[test]
    fn test_storage_read_write_success() {
        let db = setup_db();

        // Initialize with genesis block
        let genesis = genesis_block();
        initialize_db(&db, &genesis).expect("Failed to initialize DB with genesis block");

        let block1 = sample_block_info(1, genesis.hash);
        let logs1 = vec![
            sample_log(0, false),
            sample_log(1, true),
            sample_log(3, false),
            sample_log(4, true),
        ];

        // Store logs for block1
        assert!(insert_block_logs(&db, &block1, logs1.clone()).is_ok());

        let block2 = sample_block_info(2, block1.hash);
        let logs2 = vec![sample_log(0, false), sample_log(1, true)];

        // Store logs for block2
        assert!(insert_block_logs(&db, &block2, logs2.clone()).is_ok());

        let block3 = sample_block_info(3, block2.hash);
        let logs3 = vec![sample_log(0, false), sample_log(1, true), sample_log(2, true)];

        // Store logs for block3
        assert!(insert_block_logs(&db, &block3, logs3).is_ok());

        let tx = db.tx().expect("Failed to start RO tx");
        let log_reader = LogProvider::new(&tx);

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
        let db = setup_db();

        let tx = db.tx().expect("Failed to start RO tx");
        let log_reader = LogProvider::new(&tx);

        let result = log_reader.get_latest_block();
        assert!(matches!(result, Err(StorageError::EntryNotFound(_))));

        // Initialize with genesis block
        let genesis = genesis_block();
        initialize_db(&db, &genesis).expect("Failed to initialize DB with genesis block");

        assert!(
            insert_block_logs(&db, &sample_block_info(1, genesis.hash), vec![sample_log(0, true)])
                .is_ok()
        );

        let result = log_reader.get_block(2);
        assert!(matches!(result, Err(StorageError::EntryNotFound(_))));

        // should return empty logs but not an error
        let logs = log_reader.get_logs(2).expect("Should not return error");
        assert_eq!(logs.len(), 0);

        let result = log_reader.get_block_by_log(1, &sample_log(1, false));
        assert!(matches!(result, Err(StorageError::EntryNotFound(_))));
    }

    #[test]
    fn test_block_append_failed_on_order_mismatch() {
        let db = setup_db();

        // Initialize with genesis block
        let genesis = genesis_block();
        initialize_db(&db, &genesis).expect("Failed to initialize DB with genesis block");

        let block1 = sample_block_info(1, genesis.hash);
        let logs1 = vec![sample_log(0, false)];

        let block2 = sample_block_info(3, genesis.hash);
        let logs2 = vec![sample_log(0, false), sample_log(1, true)];

        // Store logs
        assert!(insert_block_logs(&db, &block1, logs1).is_ok());

        let result = insert_block_logs(&db, &block2, logs2);
        assert!(matches!(result, Err(StorageError::BlockOutOfOrder)));
    }

    #[test]
    fn test_get_block_by_log_hash_mismatch() {
        let db = setup_db();
        let genesis = genesis_block();
        initialize_db(&db, &genesis).expect("Failed to initialize DB with genesis block");

        let block1 = sample_block_info(1, genesis.hash);
        let log1 = sample_log(0, false);
        insert_block_logs(&db, &block1, vec![log1.clone()]).expect("Should insert logs");

        // Create a log with the same index but different hash
        let mut wrong_log = log1;
        wrong_log.hash = B256::from([0xFF; 32]);

        let tx = db.tx().expect("Failed to start RO tx");
        let log_reader = LogProvider::new(&tx);
        let result = log_reader.get_block_by_log(block1.number, &wrong_log);
        assert!(matches!(result, Err(StorageError::EntryNotFound(_))));
    }
}
