use crate::{
    logindexer::{log_to_log_hash, payload_hash_to_log_hash},
    syncnode::{ManagedNodeError, ReceiptProvider},
};
use kona_interop::parse_log_to_executing_message;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{LogStorageWriter, StorageError};
use kona_supervisor_types::{ExecutingMessage, Log};
use std::sync::Arc;
use thiserror::Error;

/// The [`LogIndexer`] is responsible for processing L2 receipts, extracting [`ExecutingMessage`]s,
/// and persisting them to the state manager.
#[derive(Debug)]
pub struct LogIndexer<P, W> {
    /// Component that provides receipts for a given block hash.
    pub receipt_provider: Arc<P>,
    /// Component that persists parsed log entries to storage.
    pub log_writer: Arc<W>,
}

impl<P, W> LogIndexer<P, W>
where
    P: ReceiptProvider,
    W: LogStorageWriter,
{
    /// Creates a new [`LogIndexer`] with the given receipt provider and state manager.
    ///
    /// # Arguments
    /// - `receipt_provider`: Shared reference to a component capable of fetching receipts.
    /// - `log_writer`: Shared reference to the storage layer for persisting parsed logs.
    pub const fn new(receipt_provider: Arc<P>, log_writer: Arc<W>) -> Self {
        Self { receipt_provider, log_writer }
    }

    /// Processes and stores the logs of a given block in into the state manager.
    ///
    /// This function:
    /// - Fetches all receipts for the given block from the specified chain.
    /// - Iterates through all logs in all receipts.
    /// - For each log, computes a hash from the log and optionally parses an [`ExecutingMessage`].
    /// - Records each [`Log`] including the message if found.
    /// - Saves all log entries atomically using the [`LogStorageWriter`].
    ///
    /// # Arguments
    /// - `block`: Metadata about the block being processed.
    pub async fn process_and_store_logs(&self, block: &BlockInfo) -> Result<(), LogIndexerError> {
        let receipts = self.receipt_provider.fetch_receipts(block.hash).await?;
        let mut log_entries = Vec::with_capacity(receipts.len());
        let mut log_index: u32 = 0;

        for receipt in receipts {
            for log in receipt.logs() {
                let log_hash = log_to_log_hash(log);

                let executing_message = parse_log_to_executing_message(log).map(|msg| {
                    let payload_hash =
                        payload_hash_to_log_hash(msg.payloadHash, msg.identifier.origin);
                    ExecutingMessage {
                        chain_id: msg.identifier.chainId.try_into().unwrap(),
                        block_number: msg.identifier.blockNumber.try_into().unwrap(),
                        log_index: msg.identifier.logIndex.try_into().unwrap(),
                        timestamp: msg.identifier.timestamp.try_into().unwrap(),
                        hash: payload_hash,
                    }
                });

                log_entries.push(Log { index: log_index, hash: log_hash, executing_message });

                log_index += 1;
            }
        }

        log_entries.shrink_to_fit();

        self.log_writer.store_block_logs(block, log_entries)?;
        Ok(())
    }
}

/// Error type for the [`LogIndexer`].
#[derive(Error, Debug)]
pub enum LogIndexerError {
    /// Failed to write processed logs for a block to the state manager.
    #[error(transparent)]
    StateWrite(#[from] StorageError),

    /// Failed to fetch logs for a block from the state manager.   
    #[error(transparent)]
    FetchReceipt(#[from] ManagedNodeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, B256, Bytes};
    use async_trait::async_trait;
    use jsonrpsee::core::ClientError;
    use kona_interop::{ExecutingMessageBuilder, InteropProvider, SuperchainBuilder};
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::StorageError;
    use kona_supervisor_types::{Log, Receipts};
    use op_alloy_consensus::{OpReceiptEnvelope, OpTxType};
    use std::{
        fmt::Debug,
        sync::{Arc, Mutex},
    };

    #[derive(Debug)]
    struct MockReceiptProvider {
        receipts: Receipts,
        should_error: bool,
    }

    impl MockReceiptProvider {
        const fn new(receipts: Receipts) -> Self {
            Self { receipts, should_error: false }
        }
        const fn new_with_error() -> Self {
            Self { receipts: vec![], should_error: true }
        }
    }

    #[async_trait]
    impl ReceiptProvider for MockReceiptProvider {
        async fn fetch_receipts(&self, _block_hash: B256) -> Result<Receipts, ManagedNodeError> {
            if self.should_error {
                Err(ManagedNodeError::Client(ClientError::Custom("forced error".to_string())))
            } else {
                Ok(self.receipts.clone())
            }
        }
    }

    #[derive(Debug, Default)]
    struct MockLogStorage {
        pub blocks: Mutex<Vec<BlockInfo>>,
        pub logs: Mutex<Vec<Log>>,
    }

    impl LogStorageWriter for MockLogStorage {
        fn store_block_logs(&self, block: &BlockInfo, logs: Vec<Log>) -> Result<(), StorageError> {
            self.blocks.lock().unwrap().push(*block);
            self.logs.lock().unwrap().extend(logs);
            Ok(())
        }
    }

    async fn build_receipts() -> Receipts {
        let mut builder = SuperchainBuilder::new();
        builder
            .chain(10)
            .with_timestamp(123456)
            .add_initiating_message(Bytes::from_static(b"init-msg"))
            .add_executing_message(
                ExecutingMessageBuilder::default()
                    .with_message_hash(B256::repeat_byte(0xaa))
                    .with_origin_address(Address::ZERO)
                    .with_origin_log_index(0)
                    .with_origin_block_number(1)
                    .with_origin_chain_id(10)
                    .with_origin_timestamp(123456),
            );
        let (headers, _, mock_provider) = builder.build();
        let block = headers.get(&10).unwrap();

        mock_provider.receipts_by_hash(10, block.hash()).await.unwrap()
    }

    #[tokio::test]
    async fn test_process_and_store_logs_success() {
        let receipt_provider = Arc::new(MockReceiptProvider::new(build_receipts().await));
        let log_writer = Arc::new(MockLogStorage::default());
        let log_indexer = LogIndexer::new(receipt_provider, log_writer.clone());

        let block_info = BlockInfo {
            number: 1,
            hash: B256::random(),
            timestamp: 123456789,
            ..Default::default()
        };

        let result = log_indexer.process_and_store_logs(&block_info).await;

        assert!(result.is_ok());
        let blocks = log_writer.blocks.lock().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], block_info);

        // Check logs match known injected values
        let logs = log_writer.logs.lock().unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].index, 0);
        assert_eq!(logs[0].executing_message, None);

        assert_eq!(logs[1].index, 1);
        assert_eq!(
            logs[1].executing_message,
            Some(ExecutingMessage {
                chain_id: 10,
                block_number: 1,
                log_index: 0,
                timestamp: 123456,
                hash: payload_hash_to_log_hash(B256::repeat_byte(0xaa), Address::ZERO),
            })
        );
    }

    #[tokio::test]
    async fn test_process_and_store_logs_with_empty_logs() {
        let empty_log_receipt =
            OpReceiptEnvelope::from_parts(true, 21000, vec![], OpTxType::Eip1559, None, None);

        let receipts = vec![empty_log_receipt];

        let receipt_provider = Arc::new(MockReceiptProvider::new(receipts.clone()));
        let log_writer = Arc::new(MockLogStorage::default());
        let log_indexer = LogIndexer::new(receipt_provider.clone(), log_writer.clone());

        let block_info = BlockInfo {
            number: 2,
            hash: B256::random(),
            timestamp: 111111111,
            ..Default::default()
        };

        let result = log_indexer.process_and_store_logs(&block_info).await;

        assert!(result.is_ok());

        let blocks = log_writer.blocks.lock().unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], block_info);

        let logs = log_writer.logs.lock().unwrap();
        assert!(logs.is_empty(), "Expected no logs to be stored");
    }

    #[tokio::test]
    async fn test_process_and_store_logs_receipt_fetch_fails() {
        let receipt_provider = Arc::new(MockReceiptProvider::new_with_error());
        let log_writer = Arc::new(MockLogStorage::default());
        let log_indexer = LogIndexer::new(receipt_provider, log_writer.clone());

        let block_info =
            BlockInfo { number: 3, hash: B256::random(), timestamp: 123456, ..Default::default() };

        let result = log_indexer.process_and_store_logs(&block_info).await;
        assert!(result.is_err());

        // Ensure no data was written to storage
        assert!(log_writer.blocks.lock().unwrap().is_empty());
        assert!(log_writer.logs.lock().unwrap().is_empty());
    }
}
