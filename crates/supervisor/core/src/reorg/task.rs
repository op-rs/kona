use crate::{SupervisorError, syncnode::ManagedNodeController};
use alloy_eips::BlockNumHash;
use alloy_primitives::{B256, ChainId};
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_eth::Block;
use derive_more::Constructor;
use kona_supervisor_storage::{DbReader, StorageRewinder};
use std::sync::Arc;
use tracing::{debug, info, trace, warn};

/// Handles reorg for a single chain
#[derive(Debug, Constructor)]
pub(crate) struct ReorgTask<C, DB> {
    chain_id: ChainId,
    db: Arc<DB>,
    rpc_client: RpcClient,
    managed_node: Arc<C>,
}

impl<C, DB> ReorgTask<C, DB>
where
    C: ManagedNodeController + Send + Sync + 'static,
    DB: DbReader + StorageRewinder + Send + Sync + 'static,
{
    /// Processes reorg for a single chain
    pub(crate) async fn process_chain_reorg(&self) -> Result<(), SupervisorError> {
        // Find last valid source block for this chain
        let Some(rewind_target_source) = self.find_rewind_target().await? else {
            // No need to re-org for this chain
            return Ok(());
        };

        // Get the derived block at the target source block
        let rewind_target_derived = self.db.latest_derived_block_at_source(rewind_target_source)?;

        // rewind_to() method is inclusive, so we need to get the next block.
        let rewind_to = self.db.get_block(rewind_target_derived.number + 1)?;

        // Call the rewinder to handle the DB rewinding
        self.db.rewind(&rewind_to.id()).inspect_err(|err| {
            warn!(
                target: "supervisor::reorg_handler",
                chain_id = %self.chain_id,
                %err,
                "Failed to rewind DB to derived block"
            );
        })?;

        trace!(
            target: "supervisor::reorg_handler",
            chain_id = %self.chain_id,
            "Calling resetter to reset the node after reorg"
        );

        // Reset the node after rewinding the DB.
        self.managed_node.reset().await.map_err(|err| {
            warn!(
                target: "supervisor::reorg_handler",
                chain_id = %self.chain_id,
                %err,
                "Failed to reset node after reorg"
            );
            SupervisorError::from(err)
        })?;

        Ok(())
    }

    /// Finds the rewind target for a chain during a reorg
    async fn find_rewind_target(&self) -> Result<Option<BlockNumHash>, SupervisorError> {
        trace!(
            target: "supervisor::reorg_handler",
            chain_id = %self.chain_id,
            "Finding rewind target..."
        );

        let latest_state = self.db.latest_derivation_state()?;

        // Check if the latest source block is still canonical
        if self.is_block_canonical(latest_state.source.number, latest_state.source.hash).await? {
            debug!(
                target: "supervisor::reorg_handler",
                chain_id = %self.chain_id,
                block_number = latest_state.source.number,
                "Latest source block is still canonical, no reorg needed"
            );
            return Ok(None);
        }

        // Get finalized block and derive common ancestor
        let finalized_block = self.db.get_safety_head_ref(kona_interop::SafetyLevel::Finalized)?;
        let mut common_ancestor = self.db.derived_to_source(finalized_block.id())?.id();
        let mut current_source = latest_state.source.id();

        while current_source.number > common_ancestor.number {
            if current_source.number % 5 == 0 {
                trace!(
                    target: "supervisor::reorg_handler",
                    current_block=current_source.number,
                    common_ancestor=common_ancestor.number,
                    "Finding rewind target..."
                )
            }
            // If the current source block is canonical, we found the rewind target
            if self.is_block_canonical(current_source.number, current_source.hash).await? {
                info!(
                    target: "supervisor::reorg_handler",
                    chain_id = %self.chain_id,
                    block_number = current_source.number,
                    "Found canonical block as rewind target"
                );
                common_ancestor = current_source;
                break;
            }

            // Otherwise, walk back to the previous source block
            current_source = self.db.get_source_block(current_source.number - 1)?.id();
        }

        Ok(Some(common_ancestor))
    }

    /// Checks if a block is canonical on L1
    async fn is_block_canonical(
        &self,
        block_number: u64,
        expected_hash: B256,
    ) -> Result<bool, SupervisorError> {
        match self
            .rpc_client
            .request::<_, Block>("eth_getBlockByNumber", (block_number, false))
            .await
        {
            Ok(canonical_l1) => Ok(canonical_l1.hash() == expected_hash),
            Err(err) => {
                warn!(
                    target: "supervisor::reorg_handler",
                    block_number,
                    %err,
                    "Failed to fetch canonical L1 block"
                );
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncnode::{ManagedNodeController, ManagedNodeError};
    use alloy_rpc_types_eth::Header;
    use alloy_transport::mock::*;
    use async_trait::async_trait;
    use kona_interop::{DerivedRefPair, SafetyLevel};
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{
        DerivationStorageReader, HeadRefStorageReader, LogStorageReader, StorageError,
    };
    use kona_supervisor_types::{BlockSeal, Log, SuperHead};
    use mockall::mock;

    mock!(
        #[derive(Debug)]
        pub Db {}

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
            fn get_source_block(&self, source_block_number: u64) -> Result<BlockInfo, StorageError>;
        }

        impl HeadRefStorageReader for Db {
            fn get_safety_head_ref(&self, safety_level: SafetyLevel) -> Result<BlockInfo, StorageError>;
            fn get_super_head(&self) -> Result<SuperHead, StorageError>;
        }

        impl StorageRewinder for Db {
            fn rewind(&self, to: &BlockNumHash) -> Result<(), StorageError>;
            fn rewind_log_storage(&self, to: &BlockNumHash) -> Result<(), StorageError>;
        }
    );

    mock! (
        pub chain_db {}
    );

    mock! (
        #[derive(Debug)]
        pub ManagedNode {}

        #[async_trait]
        impl ManagedNodeController for ManagedNode {
            async fn reset(&self) -> Result<(), ManagedNodeError>;
            async fn update_finalized(&self, finalized_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn update_cross_unsafe(&self, cross_unsafe_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn update_cross_safe(&self, source_block_id: BlockNumHash, derived_block_id: BlockNumHash) -> Result<(), ManagedNodeError>;
            async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ManagedNodeError>;
        }
    );

    #[tokio::test]
    async fn test_find_rewind_target_without_reorg() {
        let mut mock_db = MockDb::new();
        let latest_source: Block = Block {
            header: Header {
                hash: B256::ZERO,
                inner: alloy_consensus::Header {
                    number: 42,
                    parent_hash: B256::ZERO,
                    timestamp: 12345,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let latest_state = DerivedRefPair {
            source: BlockInfo::new(
                latest_source.header.hash,
                latest_source.header.number,
                latest_source.header.parent_hash,
                latest_source.header.timestamp,
            ),
            derived: BlockInfo::new(B256::from([5u8; 32]), 200, B256::ZERO, 1100),
        };

        // Mock the latest derivation state and expect this to be called once
        mock_db.expect_latest_derivation_state().times(1).returning(move || Ok(latest_state));

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter.clone());
        let rpc_client = RpcClient::new(transport, false);
        // Mock RPC response
        asserter.push_success(&latest_source);

        let managed_node = Arc::new(MockManagedNode::new());
        let reorg_task = ReorgTask::new(1, Arc::new(mock_db), rpc_client, managed_node);
        let rewind_target = reorg_task.find_rewind_target().await;

        // Should succeed since the latest source block is still canonical
        assert!(rewind_target.is_ok());
    }

    #[tokio::test]
    async fn test_find_rewind_target_with_reorg() {
        let mut mock_db = MockDb::new();
        let latest_source: Block = Block {
            header: Header {
                hash: B256::from([1u8; 32]),
                inner: alloy_consensus::Header {
                    number: 41,
                    parent_hash: B256::from([2u8; 32]),
                    timestamp: 12345,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let latest_state = DerivedRefPair {
            source: BlockInfo::new(
                latest_source.header.hash,
                latest_source.header.number,
                latest_source.header.parent_hash,
                latest_source.header.timestamp,
            ),
            derived: BlockInfo::new(B256::from([10u8; 32]), 200, B256::ZERO, 1100),
        };

        let finalized_source: Block = Block {
            header: Header {
                hash: B256::from([2u8; 32]),
                inner: alloy_consensus::Header {
                    number: 38,
                    parent_hash: B256::from([1u8; 32]),
                    timestamp: 12345,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let finalized_state = DerivedRefPair {
            source: BlockInfo::new(
                finalized_source.header.hash,
                finalized_source.header.number,
                finalized_source.header.parent_hash,
                finalized_source.header.timestamp,
            ),
            derived: BlockInfo::new(B256::from([20u8; 32]), 200, B256::ZERO, 1100),
        };

        let reorg_source: Block = Block {
            header: Header {
                hash: B256::from([14u8; 32]),
                inner: alloy_consensus::Header {
                    number: 40,
                    parent_hash: B256::from([13u8; 32]),
                    timestamp: 12345,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let reorg_source_info = BlockInfo::new(
            reorg_source.header.hash,
            reorg_source.header.number,
            reorg_source.header.parent_hash,
            reorg_source.header.timestamp,
        );

        let mut source_39: Block = reorg_source.clone();
        source_39.header.inner.number = 39;
        let source_39_info = BlockInfo::new(
            source_39.header.hash,
            source_39.header.number,
            source_39.header.parent_hash,
            source_39.header.timestamp,
        );

        let incorrect_source: Block = Block {
            header: Header {
                hash: B256::from([15u8; 32]),
                inner: alloy_consensus::Header {
                    number: 5000,
                    parent_hash: B256::from([13u8; 32]),
                    timestamp: 12345,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        mock_db.expect_latest_derivation_state().times(1).returning(move || Ok(latest_state));
        mock_db
            .expect_get_safety_head_ref()
            .times(1)
            .returning(move |_| Ok(finalized_state.derived));
        mock_db.expect_derived_to_source().times(1).returning(move |_| Ok(finalized_state.source));

        mock_db.expect_get_source_block().times(3).returning(
            move |block_number| match block_number {
                41 => Ok(latest_state.source),
                40 => Ok(reorg_source_info),
                39 => Ok(source_39_info),
                38 => Ok(finalized_state.source),
                _ => Ok(finalized_state.source),
            },
        );

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter.clone());
        let rpc_client = RpcClient::new(transport, false);

        // First return the reorged block
        asserter.push_success(&reorg_source);

        // Then returning some random incorrect blocks 3 times till it reaches the finalized block
        asserter.push_success(&incorrect_source);
        asserter.push_success(&incorrect_source);
        asserter.push_success(&incorrect_source);

        // Finally returning the correct block
        asserter.push_success(&finalized_source);

        let managed_node = Arc::new(MockManagedNode::new());
        let reorg_task = ReorgTask::new(1, Arc::new(mock_db), rpc_client, managed_node);
        let rewind_target = reorg_task.find_rewind_target().await;

        // Should succeed since the latest source block is still canonical
        assert!(rewind_target.is_ok());
        assert_eq!(rewind_target.unwrap(), Some(finalized_state.source.id()));
    }

    #[tokio::test]
    async fn test_is_block_canonical() {
        let canonical_hash = B256::from([1u8; 32]);
        let non_canonical_hash = B256::from([2u8; 32]);

        let canonical_block: Block = Block {
            header: Header {
                hash: canonical_hash,
                inner: alloy_consensus::Header {
                    number: 100,
                    parent_hash: B256::ZERO,
                    timestamp: 12345,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let non_canonical_block: Block = Block {
            header: Header {
                hash: non_canonical_hash,
                inner: alloy_consensus::Header {
                    number: 100,
                    parent_hash: B256::ZERO,
                    timestamp: 12345,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter.clone());
        let rpc_client = RpcClient::new(transport, false);
        asserter.push_success(&canonical_block);
        asserter.push_success(&non_canonical_block);

        let managed_node = Arc::new(MockManagedNode::new());
        let reorg_task = ReorgTask::new(1, Arc::new(MockDb::new()), rpc_client, managed_node);

        let result = reorg_task.is_block_canonical(100, canonical_hash).await;
        assert!(result.is_ok());

        // Should return false
        let result = reorg_task.is_block_canonical(100, canonical_hash).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
