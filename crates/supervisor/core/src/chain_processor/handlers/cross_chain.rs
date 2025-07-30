use super::EventHandler;
use crate::{
    ChainProcessorError, ProcessorState, chain_processor::Metrics, syncnode::ManagedNodeProvider,
};
use alloy_primitives::ChainId;
use async_trait::async_trait;
use derive_more::Constructor;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use std::sync::Arc;
use tracing::{trace, warn};

/// Handler for cross unsafe blocks.
/// This handler processes cross unsafe blocks by updating the managed node.
#[derive(Debug, Constructor)]
pub struct CrossUnsafeHandler<P> {
    chain_id: ChainId,
    managed_node: Arc<P>,
}

#[async_trait]
impl<P> EventHandler<BlockInfo> for CrossUnsafeHandler<P>
where
    P: ManagedNodeProvider + 'static,
{
    async fn handle(
        &self,
        block: BlockInfo,
        _state: &mut ProcessorState,
    ) -> Result<(), ChainProcessorError> {
        trace!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = block.number,
            "Processing cross unsafe block"
        );

        let result = self.inner_handle(block).await;
        Metrics::record_block_processing(
            self.chain_id,
            Metrics::BLOCK_TYPE_CROSS_UNSAFE,
            block,
            &result,
        );

        result
    }
}

impl<P> CrossUnsafeHandler<P>
where
    P: ManagedNodeProvider + 'static,
{
    async fn inner_handle(&self, block: BlockInfo) -> Result<(), ChainProcessorError> {
        self.managed_node.update_cross_unsafe(block.id()).await.inspect_err(|err| {
            warn!(
                target: "supervisor::chain_processor::managed_node",
                chain_id = self.chain_id,
                %block,
                %err,
                "Failed to update cross unsafe block"
            );
        })?;
        Ok(())
    }
}

/// Handler for cross safe blocks.
/// This handler processes cross safe blocks by updating the managed node.
#[derive(Debug, Constructor)]
pub struct CrossSafeHandler<P> {
    chain_id: ChainId,
    managed_node: Arc<P>,
}

#[async_trait]
impl<P> EventHandler<DerivedRefPair> for CrossSafeHandler<P>
where
    P: ManagedNodeProvider + 'static,
{
    async fn handle(
        &self,
        derived_ref_pair: DerivedRefPair,
        _state: &mut ProcessorState,
    ) -> Result<(), ChainProcessorError> {
        trace!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = derived_ref_pair.derived.number,
            "Processing cross safe block"
        );

        let result = self.inner_handle(derived_ref_pair).await;
        Metrics::record_block_processing(
            self.chain_id,
            Metrics::BLOCK_TYPE_CROSS_SAFE,
            derived_ref_pair.derived,
            &result,
        );

        result
    }
}

impl<P> CrossSafeHandler<P>
where
    P: ManagedNodeProvider + 'static,
{
    async fn inner_handle(
        &self,
        derived_ref_pair: DerivedRefPair,
    ) -> Result<(), ChainProcessorError> {
        self.managed_node
            .update_cross_safe(derived_ref_pair.source.id(), derived_ref_pair.derived.id())
            .await
            .inspect_err(|err| {
                warn!(
                    target: "supervisor::chain_processor::managed_node",
                    chain_id = self.chain_id,
                    %derived_ref_pair,
                    %err,
                    "Failed to update cross safe block"
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

    #[tokio::test]
    async fn test_handle_cross_unsafe_update_triggers() {
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let block =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        mocknode.expect_update_cross_unsafe().returning(move |cross_unsafe_block| {
            assert_eq!(cross_unsafe_block, block.id());
            Ok(())
        });

        let managed_node = Arc::new(mocknode);

        let handle = CrossUnsafeHandler::new(
            1, // chain_id
            managed_node,
        );
        let result = handle.handle(block, &mut state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_cross_unsafe_update_error() {
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let block =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        mocknode.expect_update_cross_unsafe().returning(move |_cross_unsafe_block| {
            Err(ManagedNodeError::ClientError(ClientError::Authentication(
                AuthenticationError::InvalidJwt,
            )))
        });

        let managed_node = Arc::new(mocknode);

        let handle = CrossUnsafeHandler::new(1, managed_node);
        let result = handle.handle(block, &mut state).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_handle_cross_safe_update_triggers() {
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let derived =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };
        let source =
            BlockInfo { number: 1, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        mocknode.expect_update_cross_safe().returning(move |source_id, derived_id| {
            assert_eq!(derived_id, derived.id());
            assert_eq!(source_id, source.id());
            Ok(())
        });

        let managed_node = Arc::new(mocknode);

        let handle = CrossSafeHandler::new(
            1, // chain_id
            managed_node,
        );
        let result = handle.handle(DerivedRefPair { source, derived }, &mut state).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_cross_safe_update_error() {
        let mut mocknode = MockNode::new();
        let mut state = ProcessorState::new();

        let derived =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };
        let source =
            BlockInfo { number: 1, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        mocknode.expect_update_cross_safe().returning(move |_source_id, _derived_id| {
            Err(ManagedNodeError::ClientError(ClientError::Authentication(
                AuthenticationError::InvalidJwt,
            )))
        });

        let managed_node = Arc::new(mocknode);

        let handle = CrossSafeHandler::new(1, managed_node);
        let result = handle.handle(DerivedRefPair { source, derived }, &mut state).await;
        assert!(result.is_err());
    }
}
