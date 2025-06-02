use crate::logindexer::{log_to_log_hash, payload_hash_to_log_hash};
use kona_interop::parse_log_to_executing_message;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::LogStorageWriter;
use std::sync::Arc;
use thiserror::Error;

use kona_supervisor_types::{ExecutingMessage, Log, ReceiptProvider};

/// The [`LogIndexer`] is responsible for processing L2 receipts, extracting [`ExecutingMessage`]s,
/// and persisting them to the state manager.
#[derive(Debug)]
pub struct LogIndexer {
    /// Component that provides receipts for a given block hash.
    pub receipt_provider: Arc<dyn ReceiptProvider>,
    /// Component that persists parsed log entries to storage.
    pub log_writer: Arc<dyn LogStorageWriter>,
}

impl LogIndexer {
    /// Creates a new [`LogIndexer`] with the given receipt provider and state manager.
    ///
    /// # Arguments
    /// - `receipt_provider`: Shared reference to a component capable of fetching receipts.
    /// - `log_writer`: Shared reference to the storage layer for persisting parsed logs.
    pub fn new(
        receipt_provider: Arc<dyn ReceiptProvider>,
        log_writer: Arc<dyn LogStorageWriter>,
    ) -> Self {
        Self { receipt_provider, log_writer }
    }

    /// Processes the logs of a given block and stores them into the state manager.
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
    FetchFailed(#[from] jsonrpsee::core::ClientError),
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
    use std::{
        fmt::Debug,
        sync::{Arc, Mutex},
    };

    #[derive(Debug)]
    struct MockReceiptProvider {
        pub receipts: Receipts,
    }

    impl MockReceiptProvider {
        const fn new(receipts: Receipts) -> Self {
            Self { receipts }
        }
    }

    #[async_trait]
    impl ReceiptProvider for MockReceiptProvider {
        async fn fetch_receipts(&self, _block_hash: B256) -> Result<Receipts, ClientError> {
            Ok(self.receipts.clone())
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
        let log_indexer = LogIndexer::new(receipt_provider, log_writer);

        let block_info = BlockInfo {
            number: 1,
            hash: B256::random(),
            timestamp: 123456789,
            ..Default::default()
        };

        let result = log_indexer.process_and_store_logs(&block_info).await;

        assert!(result.is_ok());
    }
}
