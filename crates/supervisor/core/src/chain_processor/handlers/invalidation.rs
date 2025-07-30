use super::EventHandler;
use crate::{ChainProcessorError, LogIndexer, ProcessorState, syncnode::ManagedNodeProvider};
use alloy_primitives::ChainId;
use async_trait::async_trait;
use derive_more::Constructor;
use kona_interop::{BlockReplacement, DerivedRefPair};
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DerivationStorage, LogStorage, StorageRewinder};
use kona_supervisor_types::BlockSeal;
use std::sync::Arc;
use tracing::{debug, error, trace, warn};

/// Handler for block invalidation events.
/// This handler processes block invalidation by rewinding the state and updating the managed node.
#[derive(Debug, Constructor)]
pub struct InvalidationHandler<P, W> {
    chain_id: ChainId,
    managed_node: Arc<P>,
    db_provider: Arc<W>,
}

#[async_trait]
impl<P, W> EventHandler<BlockInfo> for InvalidationHandler<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: DerivationStorage + StorageRewinder + Send + Sync + 'static,
{
    async fn handle(
        &self,
        block: BlockInfo,
        state: &mut ProcessorState,
    ) -> Result<(), ChainProcessorError> {
        trace!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = block.number,
            "Invalidating block"
        );

        if state.is_invalidated() {
            trace!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                block_number = block.number,
                "Invalidated block already set, skipping"
            );
            return Ok(());
        }

        let source_block = self.db_provider.derived_to_source(block.id()).inspect_err(|err| {
            warn!(
                target: "supervisor::chain_processor::db",
                chain_id = self.chain_id,
                %block,
                %err,
                "Failed to get source block for invalidation"
            );
        })?;

        self.db_provider.rewind(&block.id()).inspect_err(|err| {
            warn!(
                target: "supervisor::chain_processor::db",
                chain_id = self.chain_id,
                %block,
                %err,
                "Failed to rewind state for invalidation"
            );
        })?;

        let block_seal = BlockSeal::new(block.hash, block.number, block.timestamp);
        self.managed_node.invalidate_block(block_seal).await.inspect_err(|err| {
            warn!(
                target: "supervisor::chain_processor::managed_node",
                chain_id = self.chain_id,
                %block,
                %err,
                "Failed to invalidate block in managed node"
            );
        })?;

        state.set_invalidated(DerivedRefPair { source: source_block, derived: block });
        Ok(())
    }
}

/// Handler for block replacement events.
/// This handler processes block replacements by resyncing the log and derivation storage.
#[derive(Debug, Constructor)]
pub struct ReplacementHandler<P, W> {
    chain_id: ChainId,
    log_indexer: Arc<LogIndexer<P, W>>,
    db_provider: Arc<W>,
}

#[async_trait]
impl<P, W> EventHandler<BlockReplacement> for ReplacementHandler<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: LogStorage + DerivationStorage + 'static,
{
    async fn handle(
        &self,
        replacement: BlockReplacement,
        state: &mut ProcessorState,
    ) -> Result<(), ChainProcessorError> {
        trace!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            %replacement,
            "Handling block replacement"
        );

        let invalidated_ref_pair = match state.get_invalidated() {
            Some(block) => block,
            None => {
                debug!(
                    target: "supervisor::chain_processor",
                    chain_id = self.chain_id,
                    %replacement,
                    "No invalidated block set, skipping replacement"
                );
                return Ok(())
            }
        };

        if invalidated_ref_pair.derived.hash != replacement.invalidated {
            debug!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                invalidated_block = %invalidated_ref_pair.derived,
                replacement_block = %replacement.replacement,
                "Invalidated block hash does not match replacement, skipping"
            );
            return Ok(());
        }

        let derived_ref_pair = DerivedRefPair {
            source: invalidated_ref_pair.source,
            derived: replacement.replacement,
        };

        self.retry_with_resync_derived_block(derived_ref_pair).await?;
        state.clear_invalidated();
        Ok(())
    }
}

impl<P, W> ReplacementHandler<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: LogStorage + DerivationStorage + 'static,
{
    async fn retry_with_resync_derived_block(
        &self,
        derived_ref_pair: DerivedRefPair,
    ) -> Result<(), ChainProcessorError> {
        trace!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            derived_block_number = derived_ref_pair.derived.number,
            "Retrying with resync of derived block"
        );

        self.log_indexer.process_and_store_logs(&derived_ref_pair.derived).await.inspect_err(
            |err| {
                error!(
                    target: "supervisor::chain_processor::log_indexer",
                    chain_id = self.chain_id,
                    %derived_ref_pair,
                    %err,
                    "Failed to process and store logs for derived block"
                );
            },
        )?;

        self.db_provider.save_derived_block(derived_ref_pair).inspect_err(|err| {
            error!(
                target: "supervisor::chain_processor::db",
                chain_id = self.chain_id,
                %derived_ref_pair,
                %err,
                "Failed to save derived block"
            );
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event::ChainEvent,
        syncnode::{
            AuthenticationError, BlockProvider, ClientError, ManagedNodeController,
            ManagedNodeDataProvider, ManagedNodeError, NodeSubscriber,
        },
    };
    use alloy_primitives::B256;
    use alloy_rpc_types_eth::BlockNumHash;
    use async_trait::async_trait;
    use kona_interop::DerivedRefPair;
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{
        DerivationStorageReader, DerivationStorageWriter, LogStorageReader, LogStorageWriter,
        StorageError,
    };
    use kona_supervisor_types::{BlockSeal, Log, OutputV0, Receipts};
    use mockall::mock;
    use tokio::sync::mpsc;

    mock!(
        #[derive(Debug)]
        pub Node {}

        #[async_trait]
        impl NodeSubscriber for Node {
            async fn start_subscription(
                &self,
                _event_tx: mpsc::Sender<ChainEvent>,
            ) -> Result<(), ManagedNodeError>;
        }

        #[async_trait]
        impl BlockProvider for Node {
            async fn fetch_receipts(&self, _block_hash: B256) -> Result<Receipts, ManagedNodeError>;
            async fn block_by_number(&self, _number: u64) -> Result<BlockInfo, ManagedNodeError>;
        }

        #[async_trait]
        impl ManagedNodeDataProvider for Node {
            async fn output_v0_at_timestamp(
                &self,
                _timestamp: u64,
            ) -> Result<OutputV0, ManagedNodeError>;

            async fn pending_output_v0_at_timestamp(
                &self,
                _timestamp: u64,
            ) -> Result<OutputV0, ManagedNodeError>;

            async fn l2_block_ref_by_timestamp(
                &self,
                _timestamp: u64,
            ) -> Result<BlockInfo, ManagedNodeError>;
        }

        #[async_trait]
        impl ManagedNodeController for Node {
            async fn update_finalized(
                &self,
                _finalized_block_id: BlockNumHash,
            ) -> Result<(), ManagedNodeError>;

            async fn update_cross_unsafe(
                &self,
                cross_unsafe_block_id: BlockNumHash,
            ) -> Result<(), ManagedNodeError>;

            async fn update_cross_safe(
                &self,
                source_block_id: BlockNumHash,
                derived_block_id: BlockNumHash,
            ) -> Result<(), ManagedNodeError>;

            async fn reset(&self) -> Result<(), ManagedNodeError>;

            async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ManagedNodeError>;
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

        impl DerivationStorageReader for Db {
            fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derived_block_at_source(&self, source_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derivation_state(&self) -> Result<DerivedRefPair, StorageError>;
            fn get_source_block(&self, source_block_id: u64) -> Result<BlockInfo, StorageError>;
        }

        impl DerivationStorageWriter for Db {
            fn initialise_derivation_storage(
                &self,
                incoming_pair: DerivedRefPair,
            ) -> Result<(), StorageError>;

            fn save_derived_block(
                &self,
                incoming_pair: DerivedRefPair,
            ) -> Result<(), StorageError>;

            fn save_source_block(
                &self,
                source: BlockInfo,
            ) -> Result<(), StorageError>;
        }

        impl StorageRewinder for Db {
            fn rewind_log_storage(&self, to: &BlockNumHash) -> Result<(), StorageError>;
            fn rewind(&self, to: &BlockNumHash) -> Result<(), StorageError>;
        }
    );

    #[tokio::test]
    async fn test_handle_invalidate_block_already_set_skips() {
        let mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);

        // Set up state: invalidated_block is already set
        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = InvalidationHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );

        state.set_invalidated(DerivedRefPair { source: block, derived: block });

        let result = handler.handle(block, &mut state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_derived_to_source_error() {
        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);

        mockdb.expect_derived_to_source().returning(move |_id| Err(StorageError::FutureData));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = InvalidationHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );

        let result = handler.handle(block, &mut state).await;
        assert!(matches!(result, Err(ChainProcessorError::StorageError(StorageError::FutureData))));

        // make sure invalidated_block is not set
        assert!(state.get_invalidated().is_none());
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_rewind_error() {
        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);

        mockdb.expect_derived_to_source().returning(move |_id| Ok(block));
        mockdb.expect_rewind().returning(move |_to| Err(StorageError::DatabaseNotInitialised));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = InvalidationHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );

        let result = handler.handle(block, &mut state).await;
        assert!(matches!(
            result,
            Err(ChainProcessorError::StorageError(StorageError::DatabaseNotInitialised))
        ));

        // make sure invalidated_block is not set
        assert!(state.get_invalidated().is_none());
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_managed_node_error() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);

        mockdb.expect_derived_to_source().returning(move |_id| Ok(block));
        mockdb.expect_rewind().returning(move |_to| Ok(()));
        mocknode.expect_invalidate_block().returning(move |_seal| {
            Err(ManagedNodeError::ClientError(ClientError::Authentication(
                AuthenticationError::InvalidHeader,
            )))
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = InvalidationHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );

        let result = handler.handle(block, &mut state).await;
        assert!(matches!(result, Err(ChainProcessorError::ManagedNode(_))));

        // make sure invalidated_block is not set
        assert!(state.get_invalidated().is_none());
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_success_sets_invalidated() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let derived_block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);
        let source_block = BlockInfo::new(B256::from([2u8; 32]), 41, B256::ZERO, 12344);

        mockdb.expect_derived_to_source().returning(move |_id| Ok(source_block));
        mockdb.expect_rewind().returning(move |_to| Ok(()));
        mocknode.expect_invalidate_block().returning(move |_seal| Ok(()));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = InvalidationHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );

        let result = handler.handle(derived_block, &mut state).await;
        assert!(result.is_ok());

        // make sure invalidated_block is set
        let pair = state.get_invalidated().unwrap();
        assert_eq!(pair.derived, derived_block);
        assert_eq!(pair.source, source_block);
    }

    #[tokio::test]
    async fn test_handle_block_replacement_no_invalidated_block() {
        let mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let replacement = BlockReplacement {
            invalidated: B256::from([1u8; 32]),
            replacement: BlockInfo::new(B256::from([2u8; 32]), 43, B256::ZERO, 12346),
        };

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        // Create a mock log indexer
        let log_indexer = Arc::new(LogIndexer::new(1, managed_node.clone(), writer.clone()));

        let handler = ReplacementHandler::new(
            1, // chain_id
            log_indexer,
            writer,
        );

        let result = handler.handle(replacement, &mut state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_block_replacement_invalidated_hash_mismatch() {
        let mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let invalidated_block = BlockInfo::new(B256::from([3u8; 32]), 42, B256::ZERO, 12345);
        let replacement = BlockReplacement {
            invalidated: B256::from([1u8; 32]), // does not match invalidated_block.hash
            replacement: BlockInfo::new(B256::from([2u8; 32]), 43, B256::ZERO, 12346),
        };

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        // Create a mock log indexer
        let log_indexer = Arc::new(LogIndexer::new(1, managed_node.clone(), writer.clone()));

        state.set_invalidated(DerivedRefPair {
            source: invalidated_block,
            derived: invalidated_block,
        });

        let handler = ReplacementHandler::new(
            1, // chain_id
            log_indexer,
            writer,
        );

        let result = handler.handle(replacement, &mut state).await;
        assert!(result.is_ok());

        // invalidated_block should remain set
        let invalidated = state.get_invalidated();
        assert!(invalidated.is_some());
    }

    #[tokio::test]
    async fn test_handle_block_replacement_success() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let source_block = BlockInfo::new(B256::from([1u8; 32]), 45, B256::ZERO, 12345);
        let invalidated_block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);
        let replacement_block = BlockInfo::new(B256::from([2u8; 32]), 42, B256::ZERO, 12346);

        mockdb.expect_save_derived_block().returning(move |_pair| Ok(()));
        mockdb.expect_store_block_logs().returning(move |_block, _logs| Ok(()));

        mocknode.expect_fetch_receipts().returning(move |_block_hash| {
            assert_eq!(_block_hash, replacement_block.hash);
            Ok(Receipts::default())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        // Create a mock log indexer
        let log_indexer = Arc::new(LogIndexer::new(1, managed_node.clone(), writer.clone()));

        state.set_invalidated(DerivedRefPair { source: source_block, derived: invalidated_block });

        let handler = ReplacementHandler::new(
            1, // chain_id
            log_indexer,
            writer,
        );

        let result = handler
            .handle(
                BlockReplacement {
                    invalidated: invalidated_block.hash,
                    replacement: replacement_block,
                },
                &mut state,
            )
            .await;
        assert!(result.is_ok());

        // invalidated_block should be cleared
        assert!(state.get_invalidated().is_none());
    }
}
