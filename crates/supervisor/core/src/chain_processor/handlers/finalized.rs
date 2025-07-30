use super::EventHandler;
use crate::{
    ChainProcessorError, ProcessorState, chain_processor::Metrics, syncnode::ManagedNodeProvider,
};
use alloy_primitives::ChainId;
use async_trait::async_trait;
use derive_more::Constructor;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::HeadRefStorageWriter;
use std::sync::Arc;
use tracing::{trace, warn};

/// Handler for finalized block updates.
/// This handler processes finalized block updates by updating the managed node and state manager.
#[derive(Debug, Constructor)]
pub struct FinalizedHandler<P, W> {
    chain_id: ChainId,
    managed_node: Arc<P>,
    db_provider: Arc<W>,
}

#[async_trait]
impl<P, W> EventHandler<BlockInfo> for FinalizedHandler<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: HeadRefStorageWriter + Send + Sync + 'static,
{
    async fn handle(
        &self,
        finalized_source_block: BlockInfo,
        _state: &mut ProcessorState,
    ) -> Result<(), ChainProcessorError> {
        trace!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = finalized_source_block.number,
            "Processing finalized L1 update"
        );

        let result = self.inner_handle(finalized_source_block).await;
        Metrics::record_block_processing(
            self.chain_id,
            Metrics::BLOCK_TYPE_FINALIZED,
            finalized_source_block,
            &result,
        );

        result
    }
}

impl<P, W> FinalizedHandler<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: HeadRefStorageWriter + Send + Sync + 'static,
{
    async fn inner_handle(
        &self,
        finalized_source_block: BlockInfo,
    ) -> Result<(), ChainProcessorError> {
        let finalized_derived_block = self
            .db_provider
            .update_finalized_using_source(finalized_source_block)
            .inspect_err(|err| {
                warn!(
                    target: "supervisor::chain_processor::db",
                    chain_id = self.chain_id,
                    %finalized_source_block,
                    %err,
                    "Failed to update finalized block using source"
                );
            })?;

        self.managed_node.update_finalized(finalized_derived_block.id()).await.inspect_err(
            |err| {
                warn!(
                    target: "supervisor::chain_processor::managed_node",
                    chain_id = self.chain_id,
                    %finalized_derived_block,
                    %err,
                    "Failed to update finalized block"
                );
            },
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event::ChainEvent,
        syncnode::{
            AuthenticationError, BlockProvider, ManagedNodeController, ManagedNodeDataProvider,
            ManagedNodeError, NodeSubscriber,
        },
    };
    use alloy_primitives::B256;
    use alloy_rpc_types_eth::BlockNumHash;
    use async_trait::async_trait;
    use kona_interop::DerivedRefPair;
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{HeadRefStorageWriter, StorageError};
    use kona_supervisor_types::{BlockSeal, OutputV0, Receipts};
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

        impl HeadRefStorageWriter for Db {
            fn update_finalized_using_source(
                &self,
                block_info: BlockInfo,
            ) -> Result<BlockInfo, StorageError>;

            fn update_current_cross_unsafe(
                &self,
                block: &BlockInfo,
            ) -> Result<(), StorageError>;

            fn update_current_cross_safe(
                &self,
                block: &BlockInfo,
            ) -> Result<DerivedRefPair, StorageError>;
        }
    );

    #[tokio::test]
    async fn test_handle_finalized_source_update_triggers() {
        let mut mocknode = MockNode::new();
        let mut mockdb = MockDb::new();
        let mut state = ProcessorState::new();

        let finalized_source_block =
            BlockInfo { number: 99, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        // The finalized_derived_block returned by update_finalized_using_source
        let finalized_derived_block =
            BlockInfo { number: 5, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        // Expect update_finalized_using_source to be called with finalized_source_block
        mockdb.expect_update_finalized_using_source().returning(move |block_info: BlockInfo| {
            assert_eq!(block_info, finalized_source_block);
            Ok(finalized_derived_block)
        });

        // Expect update_finalized to be called with the derived block's id
        let finalized_derived_block_id = finalized_derived_block.id();
        mocknode.expect_update_finalized().returning(move |block_id| {
            assert_eq!(block_id, finalized_derived_block_id);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = FinalizedHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );
        let result = handler.handle(finalized_source_block, &mut state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_finalized_source_update_db_error() {
        let mut mocknode = MockNode::new();
        let mut mockdb = MockDb::new();
        let mut state = ProcessorState::new();

        let finalized_source_block =
            BlockInfo { number: 99, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        // DB returns error
        mockdb
            .expect_update_finalized_using_source()
            .returning(|_block_info: BlockInfo| Err(StorageError::DatabaseNotInitialised));

        // Managed node's update_finalized should NOT be called
        mocknode.expect_update_finalized().never();

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = FinalizedHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );
        let result = handler.handle(finalized_source_block, &mut state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_finalized_source_update_managed_node_error() {
        let mut mocknode = MockNode::new();
        let mut mockdb = MockDb::new();
        let mut state = ProcessorState::new();

        let finalized_source_block =
            BlockInfo { number: 99, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        let finalized_derived_block =
            BlockInfo { number: 5, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        mockdb.expect_update_finalized_using_source().returning(move |block_info: BlockInfo| {
            assert_eq!(block_info, finalized_source_block);
            Ok(finalized_derived_block)
        });

        mocknode.expect_update_finalized().returning(|_| {
            Err(ManagedNodeError::ClientError(crate::syncnode::ClientError::Authentication(
                AuthenticationError::InvalidJwt,
            )))
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = FinalizedHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );
        let result = handler.handle(finalized_source_block, &mut state).await;
        assert!(result.is_err());
    }
}
