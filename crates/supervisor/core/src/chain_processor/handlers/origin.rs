use super::EventHandler;
use crate::{ChainProcessorError, ProcessorState, syncnode::ManagedNodeProvider};
use alloy_primitives::ChainId;
use async_trait::async_trait;
use derive_more::Constructor;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DerivationStorageWriter, StorageError};
use std::sync::Arc;
use tracing::{debug, error, trace, warn};

/// Handler for origin updates in the chain.
#[derive(Debug, Constructor)]
pub struct OriginHandler<P, W> {
    chain_id: ChainId,
    managed_node: Arc<P>,
    db_provider: Arc<W>,
}

#[async_trait]
impl<P, W> EventHandler<BlockInfo> for OriginHandler<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: DerivationStorageWriter + Send + Sync + 'static,
{
    async fn handle(
        &self,
        origin: BlockInfo,
        state: &mut ProcessorState,
    ) -> Result<(), ChainProcessorError> {
        trace!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            %origin,
            "Processing derivation origin update"
        );

        if state.is_invalidated() {
            trace!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                %origin,
                "Invalidated block set, skipping derivation origin update"
            );
            return Ok(());
        }

        match self.db_provider.save_source_block(origin) {
            Ok(()) => Ok(()),
            Err(StorageError::BlockOutOfOrder) => {
                debug!(
                    target: "supervisor::chain_processor",
                    chain_id = self.chain_id,
                    %origin,
                    "Block out of order detected, resetting managed node"
                );

                if let Err(err) = self.managed_node.reset().await {
                    warn!(
                        target: "supervisor::chain_processor::managed_node",
                        chain_id = self.chain_id,
                        %origin,
                        %err,
                        "Failed to reset managed node after block out of order"
                    );
                    return Err(err.into());
                }
                Ok(())
            }
            Err(err) => {
                error!(
                    target: "supervisor::chain_processor",
                    chain_id = self.chain_id,
                    %origin,
                    %err,
                    "Failed to save source block during derivation origin update"
                );
                Err(err.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event::ChainEvent,
        syncnode::{
            BlockProvider, ManagedNodeController, ManagedNodeDataProvider, ManagedNodeError,
            NodeSubscriber,
        },
    };
    use alloy_primitives::B256;
    use alloy_rpc_types_eth::BlockNumHash;
    use async_trait::async_trait;
    use kona_interop::DerivedRefPair;
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{DerivationStorageWriter, StorageError};
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
    );

    #[tokio::test]
    async fn test_handle_derivation_origin_update_triggers() {
        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let origin =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        let origin_clone = origin;
        mockdb.expect_save_source_block().returning(move |block_info: BlockInfo| {
            assert_eq!(block_info, origin_clone);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = OriginHandler::new(
            1, // chain_id
            managed_node,
            writer,
        );

        let result = handler.handle(origin, &mut state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_derivation_origin_update_block_out_of_order_triggers_reset() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let origin =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        mockdb.expect_save_source_block().returning(|_| Err(StorageError::BlockOutOfOrder));
        mocknode.expect_reset().returning(|| Ok(()));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = OriginHandler::new(1, managed_node, writer);

        let result = handler.handle(origin, &mut state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_derivation_origin_update_reset_fails() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let origin =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        mockdb.expect_save_source_block().returning(|_| Err(StorageError::BlockOutOfOrder));
        mocknode.expect_reset().returning(|| Err(ManagedNodeError::ResetFailed));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = OriginHandler::new(1, managed_node, writer);

        let result = handler.handle(origin, &mut state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_derivation_origin_update_other_storage_error() {
        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let origin =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        mockdb.expect_save_source_block().returning(|_| Err(StorageError::DatabaseNotInitialised));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let handler = OriginHandler::new(1, managed_node, writer);

        let result = handler.handle(origin, &mut state).await;
        assert!(result.is_err());
    }
}
