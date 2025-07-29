use super::EventHandler;
use crate::{
    ChainProcessorError, LogIndexer, ProcessorState, chain_processor::Metrics,
    config::RollupConfig, syncnode::BlockProvider,
};
use alloy_primitives::ChainId;
use async_trait::async_trait;
use derive_more::Constructor;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::LogStorage;
use std::sync::Arc;
use tracing::{debug, info};

/// Handler for unsafe blocks.
/// This handler processes unsafe blocks by syncing logs and initializing log storage.
#[derive(Debug, Constructor)]
pub struct UnsafeBlockHandler<P, W> {
    rollup_config: RollupConfig,
    chain_id: ChainId,
    state_manager: Arc<W>,
    log_indexer: Arc<LogIndexer<P, W>>,
}

#[async_trait]
impl<P, W> EventHandler<BlockInfo> for UnsafeBlockHandler<P, W>
where
    P: BlockProvider + 'static,
    W: LogStorage + 'static,
{
    async fn handle(
        &self,
        block: BlockInfo,
        state: Arc<ProcessorState>,
    ) -> Result<(), ChainProcessorError> {
        debug!(
            target: "chain_processor",
            chain_id = self.chain_id,
            block_number = block.number,
            "Processing unsafe block"
        );

        if state.is_invalidated().await {
            debug!(
                target: "chain_processor",
                chain_id = self.chain_id,
                block_number = block.number,
                "Invalidated block already set, skipping unsafe event processing"
            );
            return Ok(());
        }

        let result = self.inner_handle(block).await;
        Metrics::record_block_processing(
            self.chain_id,
            Metrics::BLOCK_TYPE_LOCAL_UNSAFE,
            block,
            &result,
        );

        result
    }
}

impl<P, W> UnsafeBlockHandler<P, W>
where
    P: BlockProvider + 'static,
    W: LogStorage + 'static,
{
    async fn inner_handle(&self, block: BlockInfo) -> Result<(), ChainProcessorError> {
        if self.rollup_config.is_post_interop(block.timestamp) {
            self.log_indexer.clone().sync_logs(block);
            return Ok(());
        }

        if self.rollup_config.is_interop_activation_block(block) {
            info!(
                target: "chain_processor",
                chain_id = self.chain_id,
                block_number = block.number,
                "Initialising log storage for interop activation block"
            );
            self.state_manager.initialise_log_storage(block)?;
            return Ok(());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ProcessorState,
        config::{Genesis, RollupConfig},
        syncnode::{BlockProvider, ManagedNodeError},
    };
    use alloy_primitives::B256;
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{LogStorageReader, LogStorageWriter, StorageError};
    use kona_supervisor_types::{Log, Receipts};
    use mockall::mock;
    use std::sync::Arc;

    mock!(
        #[derive(Debug)]
        pub Node {}

        #[async_trait]
        impl BlockProvider for Node {
            async fn fetch_receipts(&self, _block_hash: B256) -> Result<Receipts, ManagedNodeError>;
            async fn block_by_number(&self, _number: u64) -> Result<BlockInfo, ManagedNodeError>;
        }
    );

    mock!(
        #[derive(Debug)]
        pub Db {}

        impl LogStorageWriter for Db {
            fn initialise_log_storage(
                &self,
                block: BlockInfo,
            ) -> Result<(), StorageError>;

            fn store_block_logs(
                &self,
                block: &BlockInfo,
                logs: Vec<Log>,
            ) -> Result<(), StorageError>;
        }

        impl LogStorageReader for Db {
            fn get_block(&self, block_number: u64) -> Result<BlockInfo, StorageError>;
            fn get_latest_block(&self) -> Result<BlockInfo, StorageError>;
            fn get_log(&self,block_number: u64,log_index: u32) -> Result<Log, StorageError>;
            fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, StorageError>;
        }
    );

    fn genesis() -> Genesis {
        let l2 = BlockInfo::new(B256::from([1u8; 32]), 0, B256::ZERO, 50);
        let l1 = BlockInfo::new(B256::from([2u8; 32]), 10, B256::ZERO, 1000);
        Genesis::new(l1, l2)
    }

    fn get_rollup_config(interop_time: u64) -> RollupConfig {
        RollupConfig::new(genesis(), 2, Some(interop_time))
    }

    #[tokio::test]
    async fn test_handle_unsafe_event_pre_interop() {
        let mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let state = Arc::new(ProcessorState::new());
        let rollup_config = get_rollup_config(1000);

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        // Create a mock log indexer
        let log_indexer = Arc::new(LogIndexer::new(1, managed_node.clone(), writer.clone()));

        let handler = UnsafeBlockHandler::new(
            rollup_config,
            1, // chain_id
            writer,
            log_indexer,
        );

        // Pre-interop block (timestamp < 1000)
        let block = BlockInfo::new(B256::ZERO, 123, B256::ZERO, 10);

        let result = handler.handle(block, state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_unsafe_event_post_interop() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();
        let state = Arc::new(ProcessorState::new());
        let rollup_config = get_rollup_config(1000);

        // Send unsafe block event
        let block = BlockInfo::new(B256::ZERO, 123, B256::ZERO, 1003);

        mockdb.expect_store_block_logs().returning(move |_block, _log| Ok(()));
        mocknode.expect_fetch_receipts().returning(move |block_hash| {
            assert!(block_hash == block.hash);
            Ok(Receipts::default())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        // Create a mock log indexer
        let log_indexer = Arc::new(LogIndexer::new(1, managed_node.clone(), writer.clone()));

        let handler = UnsafeBlockHandler::new(
            rollup_config,
            1, // chain_id
            writer,
            log_indexer,
        );

        let result = handler.handle(block, state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_unsafe_event_interop_activation() {
        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let state = Arc::new(ProcessorState::new());
        let rollup_config = get_rollup_config(1000);

        // Block that triggers interop activation
        let block = BlockInfo::new(B256::ZERO, 123, B256::ZERO, 1001);

        mockdb.expect_initialise_log_storage().times(1).returning(move |b| {
            assert_eq!(b, block);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        // Create a mock log indexer
        let log_indexer = Arc::new(LogIndexer::new(1, managed_node.clone(), writer.clone()));

        let handler = UnsafeBlockHandler::new(
            rollup_config,
            1, // chain_id
            writer,
            log_indexer,
        );

        let result = handler.handle(block, state).await;
        assert!(result.is_ok());
    }
}
