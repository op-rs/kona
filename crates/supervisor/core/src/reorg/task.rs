use super::metrics::Metrics;
use crate::{ReorgHandlerError, syncnode::ManagedNodeController};
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::{B256, ChainId};
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_eth::Block;
use derive_more::Constructor;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DbReader, StorageError, StorageRewinder};
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
    /// Processes reorg for a single chain. If the chain is consistent with the L1 chain,
    /// does nothing.
    pub(crate) async fn process_chain_reorg(&self) -> Result<(), ReorgHandlerError> {
        let latest_state = self.db.latest_derivation_state()?;

        // Find last valid source block for this chain
        let rewound_state = match self.find_rewind_target(latest_state).await {
            Ok(Some(rewind_target_source)) => {
                Some(self.rewind_to_target_source(rewind_target_source).await?)
            }
            Ok(None) => {
                // No reorg needed, latest source block is still canonical
                return Ok(());
            }
            Err(ReorgHandlerError::RewindTargetPreInterop) => {
                self.rewind_to_activation_block().await?
            }
            Err(err) => {
                return Err(err);
            }
        };

        // Reset the node after rewinding the DB.
        self.managed_node.reset().await.inspect_err(|err| {
            warn!(
                target: "supervisor::reorg_handler::managed_node",
                chain_id = %self.chain_id,
                %err,
                "Failed to reset node after reorg"
            );
        })?;

        // record metrics
        if let Some(rewound_state) = rewound_state {
            Metrics::record_block_depth(
                self.chain_id,
                latest_state.source.number - rewound_state.source.number,
                latest_state.derived.number - rewound_state.derived.number,
            );
        }
        Ok(())
    }

    async fn rewind_to_target_source(
        &self,
        rewind_target_source: BlockInfo,
    ) -> Result<DerivedRefPair, ReorgHandlerError> {
        // Get the derived block at the target source block
        let rewind_target_derived =
            self.db.latest_derived_block_at_source(rewind_target_source.id())?;

        // rewind_to() method is inclusive, so we need to get the next block.
        let rewind_to = self.db.get_block(rewind_target_derived.number + 1)?;
        // Call the rewinder to handle the DB rewinding
        self.db.rewind(&rewind_to.id()).inspect_err(|err| {
            warn!(
                target: "supervisor::reorg_handler::db",
                chain_id = %self.chain_id,
                %err,
                "Failed to rewind DB to derived block"
            );
        })?;

        Ok(DerivedRefPair { source: rewind_target_source, derived: rewind_target_derived })
    }

    async fn rewind_to_activation_block(
        &self,
    ) -> Result<Option<DerivedRefPair>, ReorgHandlerError> {
        // If the rewind target is pre-interop, we need to rewind to the activation block
        match self.db.get_activation_block() {
            Ok(activation_block) => {
                let activation_source_block = self.db.derived_to_source(activation_block.id())?;
                self.db.rewind(&activation_block.id()).inspect_err(|err| {
                    warn!(
                        target: "supervisor::reorg_handler::db",
                        chain_id = %self.chain_id,
                        %err,
                        "Failed to rewind DB to activation block"
                    );
                })?;
                Ok(Some(DerivedRefPair {
                    source: activation_source_block,
                    derived: activation_block,
                }))
            }
            Err(StorageError::DatabaseNotInitialised) => {
                debug!(
                    target: "supervisor::reorg_handler",
                    chain_id = %self.chain_id,
                    "No activation block found, no rewind required"
                );
                Ok(None)
            }
            Err(err) => Err(ReorgHandlerError::StorageError(err)),
        }
    }

    /// Finds the rewind target for a chain during a reorg
    ///
    /// Returns `None` if no rewind is needed, or the target block to rewind to.
    /// Returns ReorgHandlerError::RewindTargetPreInterop if the rewind target is before the interop
    /// activation block.
    async fn find_rewind_target(
        &self,
        latest_state: DerivedRefPair,
    ) -> Result<Option<BlockInfo>, ReorgHandlerError> {
        trace!(
            target: "supervisor::reorg_handler",
            chain_id = %self.chain_id,
            "Finding rewind target..."
        );

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

        let mut common_ancestor = self.find_common_ancestor().await?;
        let mut current_source = latest_state.source;

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
            current_source = self.db.get_source_block(current_source.number - 1)?;
        }

        Ok(Some(common_ancestor))
    }

    async fn find_common_ancestor(&self) -> Result<BlockInfo, ReorgHandlerError> {
        trace!(
            target: "supervisor::reorg_handler",
            chain_id = %self.chain_id,
            "Finding common ancestor."
        );

        match self.db.get_safety_head_ref(kona_interop::SafetyLevel::Finalized) {
            Ok(finalized_block) => {
                let common_ancestor = self.db.derived_to_source(finalized_block.id())?;
                return Ok(common_ancestor)
            }
            Err(StorageError::FutureData) => { /* fall through to activation block */ }
            Err(err) => {
                return Err(ReorgHandlerError::StorageError(err));
            }
        }

        debug!(
            target: "supervisor::reorg_handler",
            chain_id = %self.chain_id,
            "No finalized block found, checking activation block."
        );

        match self.db.get_activation_block() {
            Ok(activation_block) => {
                let activation_source_block = self.db.derived_to_source(activation_block.id())?;
                if self
                    .is_block_canonical(
                        activation_source_block.number,
                        activation_source_block.hash,
                    )
                    .await?
                {
                    Ok(activation_source_block)
                } else {
                    debug!(
                        target: "supervisor::reorg_handler",
                        chain_id = %self.chain_id,
                        "Activation block is not canonical, no common ancestor found"
                    );
                    Err(ReorgHandlerError::RewindTargetPreInterop)
                }
            }
            Err(StorageError::DatabaseNotInitialised) => {
                Err(ReorgHandlerError::RewindTargetPreInterop)
            }
            Err(err) => Err(ReorgHandlerError::StorageError(err)),
        }
    }

    /// Checks if a block is canonical on L1
    async fn is_block_canonical(
        &self,
        block_number: u64,
        expected_hash: B256,
    ) -> Result<bool, ReorgHandlerError> {
        let canonical_l1 = self
            .rpc_client
            .request::<_, Block>(
                "eth_getBlockByNumber",
                (BlockNumberOrTag::Number(block_number), false),
            )
            .await
            .map_err(|err| {
                warn!(
                    target: "supervisor::reorg_handler",
                    block_number,
                    %err,
                    "Failed to fetch L1 block from RPC"
                );
                ReorgHandlerError::RPCError(err.to_string())
            })?;
        Ok(canonical_l1.hash() == expected_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncnode::{ManagedNodeController, ManagedNodeError};
    use alloy_eips::BlockNumHash;
    use alloy_rpc_types_eth::Header;
    use alloy_transport::mock::*;
    use async_trait::async_trait;
    use kona_interop::{DerivedRefPair, SafetyLevel};
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{
        DerivationStorageReader, HeadRefStorageReader, LogStorageReader, StorageError,
    };
    use kona_supervisor_types::{BlockSeal, Log, SuperHead};
    use mockall::{mock, predicate};

    mock!(
        #[derive(Debug)]
        pub Db {}

        impl LogStorageReader for Db {
            fn get_block(&self, block_number: u64) -> Result<BlockInfo, StorageError>;
            fn get_latest_block(&self) -> Result<BlockInfo, StorageError>;
            fn get_log(&self, block_number: u64,log_index: u32) -> Result<Log, StorageError>;
            fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, StorageError>;
        }

        impl DerivationStorageReader for Db {
            fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derived_block_at_source(&self, source_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derivation_state(&self) -> Result<DerivedRefPair, StorageError>;
            fn get_source_block(&self, source_block_number: u64) -> Result<BlockInfo, StorageError>;
            fn get_activation_block(&self) -> Result<BlockInfo, StorageError>;
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
    async fn test_process_chain_reorg_no_reorg_needed() {
        let mut mock_db = MockDb::new();
        let mut managed_node = MockManagedNode::new();

        let latest_source =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        let latest_state = DerivedRefPair {
            source: latest_source,
            derived: BlockInfo::new(B256::from([3u8; 32]), 50, B256::from([4u8; 32]), 12346),
        };

        // Mock the latest derivation state
        mock_db.expect_latest_derivation_state().times(1).returning(move || Ok(latest_state));

        // Mock the RPC to return the same block (no reorg)
        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter.clone());
        let rpc_client = RpcClient::new(transport, false);

        let canonical_block: Block = Block {
            header: Header {
                hash: latest_source.hash,
                inner: alloy_consensus::Header {
                    number: latest_source.number,
                    parent_hash: latest_source.parent_hash,
                    timestamp: latest_source.timestamp,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        asserter.push_success(&canonical_block);

        // Managed node should not be reset
        managed_node.expect_reset().times(0);

        let reorg_task = ReorgTask::new(1, Arc::new(mock_db), rpc_client, Arc::new(managed_node));

        let result = reorg_task.process_chain_reorg().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_chain_reorg_with_rewind() {
        let mut mock_db = MockDb::new();
        let mut managed_node = MockManagedNode::new();

        let latest_source =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        let latest_state = DerivedRefPair {
            source: latest_source,
            derived: BlockInfo::new(B256::from([3u8; 32]), 50, B256::from([4u8; 32]), 12346),
        };

        let rewind_target_source =
            BlockInfo::new(B256::from([10u8; 32]), 95, B256::from([11u8; 32]), 12340);

        let rewind_target_derived =
            BlockInfo::new(B256::from([12u8; 32]), 45, B256::from([13u8; 32]), 12341);

        let next_block = BlockInfo::new(B256::from([14u8; 32]), 46, B256::from([12u8; 32]), 12342);

        let finalized_block =
            BlockInfo::new(B256::from([20u8; 32]), 40, B256::from([21u8; 32]), 12330);

        // Mock the latest derivation state
        mock_db.expect_latest_derivation_state().times(1).returning(move || Ok(latest_state));

        // Mock finding common ancestor
        mock_db.expect_get_safety_head_ref().times(1).returning(move |_| Ok(finalized_block));

        mock_db.expect_derived_to_source().times(1).returning(move |_| Ok(rewind_target_source));

        mock_db.expect_get_source_block().times(5).returning(
            move |block_number| match block_number {
                99 => Ok(BlockInfo::new(B256::from([16u8; 32]), 99, B256::from([17u8; 32]), 12344)),
                98 => Ok(BlockInfo::new(B256::from([17u8; 32]), 98, B256::from([18u8; 32]), 12343)),
                97 => Ok(BlockInfo::new(B256::from([18u8; 32]), 97, B256::from([19u8; 32]), 12342)),
                96 => Ok(BlockInfo::new(B256::from([19u8; 32]), 96, B256::from([20u8; 32]), 12341)),
                95 => Ok(rewind_target_source),
                _ => Err(StorageError::ConflictError),
            },
        );

        // Mock the RPC to show reorg happened
        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter.clone());
        let rpc_client = RpcClient::new(transport, false);

        // First call shows different hash (reorg detected)
        let different_block: Block = Block {
            header: Header {
                hash: B256::from([99u8; 32]), // Different hash
                inner: alloy_consensus::Header {
                    number: latest_source.number,
                    parent_hash: latest_source.parent_hash,
                    timestamp: latest_source.timestamp,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        asserter.push_success(&different_block);
        asserter.push_success(&different_block);
        asserter.push_success(&different_block);
        asserter.push_success(&different_block);
        asserter.push_success(&different_block);

        // Second call for checking if rewind target is canonical
        let canonical_block: Block = Block {
            header: Header {
                hash: rewind_target_source.hash,
                inner: alloy_consensus::Header {
                    number: rewind_target_source.number,
                    parent_hash: rewind_target_source.parent_hash,
                    timestamp: rewind_target_source.timestamp,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        asserter.push_success(&canonical_block);

        // Mock rewind operations
        mock_db
            .expect_latest_derived_block_at_source()
            .times(1)
            .returning(move |_| Ok(rewind_target_derived));

        mock_db.expect_get_block().times(1).returning(move |_| Ok(next_block));

        mock_db.expect_rewind().times(1).returning(|_| Ok(()));

        // Managed node should be reset
        managed_node.expect_reset().times(1).returning(|| Ok(()));

        let reorg_task = ReorgTask::new(1, Arc::new(mock_db), rpc_client, Arc::new(managed_node));

        let result = reorg_task.process_chain_reorg().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_chain_reorg_rewind_pre_interop() {
        let mut mock_db = MockDb::new();
        let mut managed_node = MockManagedNode::new();

        let latest_source =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        let latest_state = DerivedRefPair {
            source: latest_source,
            derived: BlockInfo::new(B256::from([3u8; 32]), 50, B256::from([4u8; 32]), 12346),
        };

        let activation_block =
            BlockInfo::new(B256::from([10u8; 32]), 1, B256::from([11u8; 32]), 12000);

        let activation_source =
            BlockInfo::new(B256::from([12u8; 32]), 10, B256::from([13u8; 32]), 11999);

        // Mock the latest derivation state
        mock_db.expect_latest_derivation_state().times(1).returning(move || Ok(latest_state));

        // Mock finding common ancestor fails with pre-interop
        mock_db.expect_get_safety_head_ref().times(1).returning(|_| Err(StorageError::FutureData));

        mock_db
            .expect_get_activation_block()
            .times(2) // Once in find_common_ancestor, once in rewind_to_activation_block
            .returning(move || Ok(activation_block));

        mock_db
            .expect_derived_to_source()
            .times(2) // Once in find_common_ancestor, once in rewind_to_activation_block
            .returning(move |_| Ok(activation_source));

        // Mock the RPC calls
        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter.clone());
        let rpc_client = RpcClient::new(transport, false);

        // First call shows different hash (reorg detected)
        let different_block: Block = Block {
            header: Header {
                hash: B256::from([99u8; 32]),
                inner: alloy_consensus::Header {
                    number: latest_source.number,
                    parent_hash: latest_source.parent_hash,
                    timestamp: latest_source.timestamp,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        asserter.push_success(&different_block);

        // Activation block is not canonical
        let non_canonical_activation: Block = Block {
            header: Header {
                hash: B256::from([99u8; 32]), // Different from expected
                inner: alloy_consensus::Header {
                    number: activation_source.number,
                    parent_hash: activation_source.parent_hash,
                    timestamp: activation_source.timestamp,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        asserter.push_success(&non_canonical_activation);

        // Mock rewind to activation block
        mock_db.expect_rewind().times(1).returning(|_| Ok(()));

        // Managed node should be reset
        managed_node.expect_reset().times(1).returning(|| Ok(()));

        let reorg_task = ReorgTask::new(1, Arc::new(mock_db), rpc_client, Arc::new(managed_node));

        let result = reorg_task.process_chain_reorg().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_process_chain_reorg_storage_error() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        // DB fails to get latest derivation state
        mock_db
            .expect_latest_derivation_state()
            .times(1)
            .returning(|| Err(StorageError::LockPoisoned));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.process_chain_reorg().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReorgHandlerError::StorageError(StorageError::LockPoisoned)
        ));
    }

    #[tokio::test]
    async fn test_find_rewind_target_without_reorg() {
        let mut mock_db = MockDb::new();
        let latest_source: Block = Block {
            header: Header {
                hash: B256::from([1u8; 32]),
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
        let rewind_target = reorg_task.process_chain_reorg().await;

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

        mock_db.expect_latest_derivation_state().returning(move || Ok(latest_state));
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
        let rewind_target = reorg_task.find_rewind_target(latest_state).await;

        // Should succeed since the latest source block is still canonical
        assert!(rewind_target.is_ok());
        assert_eq!(rewind_target.unwrap(), Some(finalized_state.source));
    }

    #[tokio::test]
    async fn test_find_rewind_target_with_finalized_future_activation_canonical() {
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

        let activation_source: Block = Block {
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

        let activation_state = DerivedRefPair {
            source: BlockInfo::new(
                activation_source.header.hash,
                activation_source.header.number,
                activation_source.header.parent_hash,
                activation_source.header.timestamp,
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

        mock_db
            .expect_get_safety_head_ref()
            .times(1)
            .returning(move |_| Err(StorageError::FutureData));
        mock_db
            .expect_get_activation_block()
            .times(1)
            .returning(move || Ok(activation_state.derived));
        mock_db.expect_derived_to_source().times(1).returning(move |_| Ok(activation_state.source));

        mock_db.expect_get_source_block().times(3).returning(
            move |block_number| match block_number {
                41 => Ok(latest_state.source),
                40 => Ok(reorg_source_info),
                39 => Ok(source_39_info),
                38 => Ok(activation_state.source),
                _ => Ok(activation_state.source),
            },
        );

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter.clone());
        let rpc_client = RpcClient::new(transport, false);

        // First return the reorged block
        asserter.push_success(&reorg_source);

        // Return the activation block source to make sure it is canonical
        // Used in `find_common_ancestor`
        asserter.push_success(&activation_source);

        // Then returning some random incorrect blocks 3 times till it reaches the finalized block
        asserter.push_success(&incorrect_source);
        asserter.push_success(&incorrect_source);
        asserter.push_success(&incorrect_source);

        // Finally returning the correct block
        asserter.push_success(&activation_source);

        let managed_node = Arc::new(MockManagedNode::new());
        let reorg_task = ReorgTask::new(1, Arc::new(mock_db), rpc_client, managed_node);
        let rewind_target = reorg_task.find_rewind_target(latest_state).await;

        // Should succeed since the latest source block is still canonical
        assert!(rewind_target.is_ok());
        assert_eq!(rewind_target.unwrap(), Some(activation_state.source));
    }

    #[tokio::test]
    async fn test_find_rewind_target_with_finalized_future_activation_not_canonical() {
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

        let activation_source: Block = Block {
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

        let activation_state = DerivedRefPair {
            source: BlockInfo::new(
                activation_source.header.hash,
                activation_source.header.number,
                activation_source.header.parent_hash,
                activation_source.header.timestamp,
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

        mock_db
            .expect_get_safety_head_ref()
            .times(1)
            .returning(move |_| Err(StorageError::FutureData));
        mock_db
            .expect_get_activation_block()
            .times(1)
            .returning(move || Ok(activation_state.derived));
        mock_db.expect_derived_to_source().times(1).returning(move |_| Ok(activation_state.source));

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter.clone());
        let rpc_client = RpcClient::new(transport, false);

        // First return the reorged block
        asserter.push_success(&reorg_source);

        // Return the incorrect source to make sure activation block is not canonical
        // Used in `find_common_ancestor`
        asserter.push_success(&incorrect_source);

        let managed_node = Arc::new(MockManagedNode::new());
        let reorg_task = ReorgTask::new(1, Arc::new(mock_db), rpc_client, managed_node);
        let rewind_target = reorg_task.find_rewind_target(latest_state).await;

        assert!(matches!(rewind_target, Err(ReorgHandlerError::RewindTargetPreInterop)));
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

    #[tokio::test]
    async fn test_rewind_to_activation_block_success() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        let activation_block =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        let activation_source =
            BlockInfo::new(B256::from([3u8; 32]), 200, B256::from([4u8; 32]), 12346);

        // Expect get_activation_block to be called
        mock_db.expect_get_activation_block().times(1).returning(move || Ok(activation_block));

        // Expect derived_to_source to be called
        mock_db
            .expect_derived_to_source()
            .times(1)
            .with(mockall::predicate::eq(activation_block.id()))
            .returning(move |_| Ok(activation_source));

        // Expect rewind to be called
        mock_db
            .expect_rewind()
            .times(1)
            .with(mockall::predicate::eq(activation_block.id()))
            .returning(|_| Ok(()));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_activation_block().await;

        assert!(result.is_ok());
        let pair = result.unwrap().unwrap();
        assert_eq!(pair.source, activation_source);
        assert_eq!(pair.derived, activation_block);
    }

    #[tokio::test]
    async fn test_rewind_to_activation_block_database_not_initialized() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        // Expect get_activation_block to return DatabaseNotInitialised
        mock_db
            .expect_get_activation_block()
            .times(1)
            .returning(|| Err(StorageError::DatabaseNotInitialised));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_activation_block().await;

        // Should succeed with None (no-op case)
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_rewind_to_activation_block_storage_error() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        // Expect get_activation_block to return a different storage error
        mock_db
            .expect_get_activation_block()
            .times(1)
            .returning(|| Err(StorageError::LockPoisoned));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_activation_block().await;

        // Should return storage error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReorgHandlerError::StorageError(StorageError::LockPoisoned)
        ));
    }

    #[tokio::test]
    async fn test_rewind_to_activation_block_derived_to_source_fails() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        let activation_block =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        // Expect get_activation_block to succeed
        mock_db.expect_get_activation_block().times(1).returning(move || Ok(activation_block));

        // Expect derived_to_source to fail
        mock_db.expect_derived_to_source().times(1).returning(|_| Err(StorageError::LockPoisoned));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_activation_block().await;

        // Should return storage error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReorgHandlerError::StorageError(StorageError::LockPoisoned)
        ));
    }

    #[tokio::test]
    async fn test_rewind_to_activation_block_rewind_fails() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        let activation_block =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        let activation_source =
            BlockInfo::new(B256::from([3u8; 32]), 200, B256::from([4u8; 32]), 12346);

        // Expect get_activation_block to succeed
        mock_db.expect_get_activation_block().times(1).returning(move || Ok(activation_block));

        // Expect derived_to_source to succeed
        mock_db.expect_derived_to_source().times(1).returning(move |_| Ok(activation_source));

        // Expect rewind to fail
        mock_db.expect_rewind().times(1).returning(|_| Err(StorageError::LockPoisoned));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_activation_block().await;

        // Should return storage error
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReorgHandlerError::StorageError(StorageError::LockPoisoned)
        ));
    }

    #[tokio::test]
    async fn test_rewind_to_target_source_success() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        let rewind_target_source =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        let rewind_target_derived =
            BlockInfo::new(B256::from([3u8; 32]), 50, B256::from([4u8; 32]), 12346);

        let next_block = BlockInfo::new(B256::from([5u8; 32]), 51, B256::from([3u8; 32]), 12347);

        // Expect latest_derived_block_at_source to be called
        mock_db
            .expect_latest_derived_block_at_source()
            .times(1)
            .with(predicate::eq(rewind_target_source.id()))
            .returning(move |_| Ok(rewind_target_derived));

        // Expect get_block to be called for the next block
        mock_db
            .expect_get_block()
            .times(1)
            .with(predicate::eq(51))
            .returning(move |_| Ok(next_block));

        // Expect rewind to be called
        mock_db.expect_rewind().times(1).with(predicate::eq(next_block.id())).returning(|_| Ok(()));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_target_source(rewind_target_source).await;

        assert!(result.is_ok());
        let pair = result.unwrap();
        assert_eq!(pair.source, rewind_target_source);
        assert_eq!(pair.derived, rewind_target_derived);
    }

    #[tokio::test]
    async fn test_rewind_to_target_source_latest_derived_block_fails() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        let rewind_target_source =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        // Expect latest_derived_block_at_source to fail
        mock_db
            .expect_latest_derived_block_at_source()
            .times(1)
            .returning(|_| Err(StorageError::LockPoisoned));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_target_source(rewind_target_source).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReorgHandlerError::StorageError(StorageError::LockPoisoned)
        ));
    }

    #[tokio::test]
    async fn test_rewind_to_target_source_get_block_fails() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        let rewind_target_source =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        let rewind_target_derived =
            BlockInfo::new(B256::from([3u8; 32]), 50, B256::from([4u8; 32]), 12346);

        // Expect latest_derived_block_at_source to succeed
        mock_db
            .expect_latest_derived_block_at_source()
            .times(1)
            .returning(move |_| Ok(rewind_target_derived));

        // Expect get_block to fail
        mock_db.expect_get_block().times(1).returning(|_| Err(StorageError::LockPoisoned));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_target_source(rewind_target_source).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReorgHandlerError::StorageError(StorageError::LockPoisoned)
        ));
    }

    #[tokio::test]
    async fn test_rewind_to_target_source_rewind_fails() {
        let mut mock_db = MockDb::new();
        let managed_node = Arc::new(MockManagedNode::new());

        let rewind_target_source =
            BlockInfo::new(B256::from([1u8; 32]), 100, B256::from([2u8; 32]), 12345);

        let rewind_target_derived =
            BlockInfo::new(B256::from([3u8; 32]), 50, B256::from([4u8; 32]), 12346);

        let next_block = BlockInfo::new(B256::from([5u8; 32]), 51, B256::from([3u8; 32]), 12347);

        // Expect latest_derived_block_at_source to succeed
        mock_db
            .expect_latest_derived_block_at_source()
            .times(1)
            .returning(move |_| Ok(rewind_target_derived));

        // Expect get_block to succeed
        mock_db.expect_get_block().times(1).returning(move |_| Ok(next_block));

        // Expect rewind to fail
        mock_db.expect_rewind().times(1).returning(|_| Err(StorageError::LockPoisoned));

        let reorg_task = ReorgTask::new(
            1,
            Arc::new(mock_db),
            RpcClient::new(MockTransport::new(Asserter::new()), false),
            managed_node,
        );

        let result = reorg_task.rewind_to_target_source(rewind_target_source).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ReorgHandlerError::StorageError(StorageError::LockPoisoned)
        ));
    }
}
