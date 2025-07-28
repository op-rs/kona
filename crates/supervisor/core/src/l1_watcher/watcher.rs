use crate::{SupervisorError, event::ChainEvent, rewinder::ChainRewinder};
use alloy_eips::{BlockNumHash, BlockNumberOrTag};
use alloy_primitives::{B256, ChainId};
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_eth::{Block, Header};
use futures::StreamExt;
use kona_interop::DependencySet;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{
    ChainDb, ChainDbFactory, DerivationStorageReader, FinalizedL1Storage, HeadRefStorageReader
};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// A watcher that polls the L1 chain for finalized blocks.
#[derive(Debug)]
pub struct L1Watcher<F> {
    /// The Alloy RPC client for L1.
    rpc_client: RpcClient,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
    /// The finalized L1 block storage.
    finalized_l1_storage: Arc<F>,
    /// The event senders for each chain.
    event_txs: HashMap<ChainId, mpsc::Sender<ChainEvent>>,
    /// The superchain dependency set.
    dependency_set: Arc<DependencySet>,
    /// The database factory.
    db_factory: Arc<ChainDbFactory>,
}

impl<F> L1Watcher<F>
where
    F: FinalizedL1Storage + 'static,
{
    /// Creates a new [`L1Watcher`] instance.
    pub const fn new(
        rpc_client: RpcClient,
        finalized_l1_storage: Arc<F>,
        event_txs: HashMap<ChainId, mpsc::Sender<ChainEvent>>,
        cancellation: CancellationToken,
        dependency_set: Arc<DependencySet>,
        db_factory: Arc<ChainDbFactory>,
    ) -> Self {
        Self {
            rpc_client,
            finalized_l1_storage,
            event_txs,
            cancellation,
            dependency_set,
            db_factory,
        }
    }

    /// Starts polling for finalized and latest blocks and processes them.
    pub async fn run(&self) {
        // TODO: Change the polling interval to 1535 seconds with mainnet config.
        let finalized_head_poller = self
            .rpc_client
            .prepare_static_poller::<_, Block>(
                "eth_getBlockByNumber",
                (BlockNumberOrTag::Finalized, false),
            )
            .with_poll_interval(Duration::from_secs(47));

        let finalized_head_stream = finalized_head_poller.into_stream();

        // TODO: Change the polling interval to 11 seconds with mainnet config.
        let latest_head_poller = self
            .rpc_client
            .prepare_static_poller::<_, Block>(
                "eth_getBlockByNumber",
                (BlockNumberOrTag::Latest, false),
            )
            .with_poll_interval(Duration::from_secs(5));

        let latest_head_stream = latest_head_poller.into_stream();

        self.poll_blocks(finalized_head_stream, latest_head_stream).await;
    }

    /// Helper function to poll blocks using a provided stream and handler closure.
    async fn poll_blocks<S>(&self, mut finalized_head_stream: S, mut latest_head_stream: S)
    where
        S: futures::Stream<Item = Block> + Unpin,
    {
        let mut last_finalized_number = 0;
        let mut last_latest_number = BlockNumHash { number: 0, hash: B256::ZERO };

        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    info!(target: "l1_watcher", "L1Watcher cancellation requested, stopping polling");
                    break;
                }
                latest_block = latest_head_stream.next() => {
                    if let Some(latest_block) = latest_block {
                        info!(target: "l1_watcher", "Latest L1 block received: {:?}", latest_block.header.number);
                        self.handle_new_latest_block(latest_block, &mut last_latest_number).await;
                    }
                }
                finalized_block = finalized_head_stream.next() => {
                    if let Some(finalized_block) = finalized_block {
                        info!(target: "l1_watcher", "Finalized L1 block received: {:?}", finalized_block.header.number);
                        self.handle_new_finalized_block(finalized_block, &mut last_finalized_number);
                    }
                }
            }
        }
    }

    fn handle_new_finalized_block(&self, block: Block, last_finalized_number: &mut u64) {
        let block_number = block.header.number;
        if block_number == *last_finalized_number {
            return;
        }

        let Header {
            hash,
            inner: alloy_consensus::Header { number, parent_hash, timestamp, .. },
            ..
        } = block.header;
        let finalized_source_block = BlockInfo::new(hash, number, parent_hash, timestamp);

        info!(
            target: "l1_watcher",
            block_number = finalized_source_block.number,
            "New finalized L1 block received"
        );

        if let Err(err) = self.finalized_l1_storage.update_finalized_l1(finalized_source_block) {
            error!(target: "l1_watcher", %err, "Failed to update finalized L1 block");
            return;
        }

        self.broadcast_finalized_source_update(finalized_source_block);

        *last_finalized_number = block_number;
    }

    fn broadcast_finalized_source_update(&self, finalized_source_block: BlockInfo) {
        for (chain_id, sender) in &self.event_txs {
            if let Err(err) =
                sender.try_send(ChainEvent::FinalizedSourceUpdate { finalized_source_block })
            {
                error!(
                    target: "l1_watcher",
                    chain_id = %chain_id,
                    %err, "Failed to send finalized L1 update event",
                );
            }
        }
    }

    async fn handle_new_latest_block(&self, incoming_block: Block, previous_block: &mut BlockNumHash) {
        let incoming_block_number = incoming_block.header.number;
        if incoming_block_number <= previous_block.number {
            info!(
                target: "l1_watcher",
                "Incoming latest L1 block is not greater than the stored latest block"
            );
            return;
        }

        let Header {
            hash,
            inner: alloy_consensus::Header { number, parent_hash, timestamp, .. },
            ..
        } = incoming_block.header;
        let latest_block = BlockInfo::new(hash, number, parent_hash, timestamp);

        info!(
            target: "l1_watcher",
            block_number = latest_block.number,
            "New latest L1 block received"
        );

        if latest_block.parent_hash != previous_block.hash {
            for (chain_id, _) in &self.dependency_set.dependencies {
                let chain_db = self.db_factory.get_db(*chain_id).unwrap();
                let Ok(rewind_target_source) = self.find_rewind_target(chain_db.clone()).await else {
                    error!(target: "l1_watcher", "Failed to find rewind target for chain {}", chain_id);
                    continue;
                };
                let rewinder = ChainRewinder::new(*chain_id, chain_db);
                rewinder.handle_l1_reorg(rewind_target_source).unwrap();
                
            }
        }

        *previous_block = latest_block.id();
    }

    async fn find_rewind_target(
        &self,
        db: Arc<ChainDb>
    ) -> Result<BlockNumHash, SupervisorError> {
        let latest_state = db.latest_derivation_state()?;

        if let Ok(canonical_l1) = self
            .rpc_client
            .request::<_, Block>("eth_getBlockByNumber", (latest_state.source.number, false))
            .await
        {
            if canonical_l1.hash() == latest_state.source.hash {
                return Ok(latest_state.source.id());
            }
        }

        let finalized_block = db.get_safety_head_ref(kona_interop::SafetyLevel::Finalized)?;

        let mut common_ancestor = db.derived_to_source(finalized_block.id())?.id();
        let mut current_source = latest_state.source.id();

        while current_source.number >= common_ancestor.number {
            if let Ok(canonical_l1) = self
                .rpc_client
                .request::<_, Block>("eth_getBlockByNumber", (current_source.number, false))
                .await
            {
                if canonical_l1.hash() == current_source.hash {
                    common_ancestor = current_source;
                    break;
                }
            }
            current_source = db.get_source_block(current_source.number - 1)?.id();
        }
        Ok(common_ancestor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use alloy_transport::mock::*;
    use kona_supervisor_storage::{FinalizedL1Storage, StorageError};
    use mockall::{mock, predicate::*};
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tempfile::TempDir;

    fn temp_factory() -> Arc<ChainDbFactory> {
        let tmp = TempDir::new().expect("create temp dir");
        let factory = ChainDbFactory::new(tmp.path().to_path_buf());
        Arc::new(factory)
    }

    // Mock the FinalizedL1Storage trait
    mock! {
        pub finalized_l1_storage {}
        impl FinalizedL1Storage for finalized_l1_storage {
            fn update_finalized_l1(&self, block: BlockInfo) -> Result<(), StorageError>;
            fn get_finalized_l1(&self) -> Result<BlockInfo, StorageError>;
        }
    }

    #[tokio::test]
    async fn test_broadcast_finalized_source_update_sends_to_all() {
        let (tx1, mut rx1) = mpsc::channel(1);
        let (tx2, mut rx2) = mpsc::channel(1);

        let mut event_txs = HashMap::new();
        event_txs.insert(1, tx1);
        event_txs.insert(2, tx2);

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter);
        let rpc_client = RpcClient::new(transport, false);

        let depset = DependencySet {
            dependencies: HashMap::default(),
            override_message_expiry_window: Some(3600),
        };

        let watcher = L1Watcher {
            rpc_client,
            cancellation: CancellationToken::new(),
            finalized_l1_storage: Arc::new(Mockfinalized_l1_storage::new()),
            event_txs,
            dependency_set: Arc::new(depset),
            db_factory: temp_factory(),
        };

        let block = BlockInfo::new(B256::ZERO, 42, B256::ZERO, 12345);
        watcher.broadcast_finalized_source_update(block);

        assert!(
            matches!(rx1.recv().await, Some(ChainEvent::FinalizedSourceUpdate { finalized_source_block }) if finalized_source_block == block)
        );
        assert!(
            matches!(rx2.recv().await, Some(ChainEvent::FinalizedSourceUpdate { finalized_source_block }) if finalized_source_block == block)
        );
    }

    #[tokio::test]
    async fn test_handle_new_finalized_block_updates_and_broadcasts() {
        let (tx, mut rx) = mpsc::channel(1);
        let event_txs = [(1, tx)].into_iter().collect();

        let mut mock_storage = Mockfinalized_l1_storage::new();
        mock_storage.expect_update_finalized_l1().returning(|_block| Ok(()));

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter);
        let rpc_client = RpcClient::new(transport, false);

        let depset = DependencySet {
            dependencies: HashMap::default(),
            override_message_expiry_window: Some(3600),
        };

        let watcher = L1Watcher {
            rpc_client,
            cancellation: CancellationToken::new(),
            finalized_l1_storage: Arc::new(mock_storage),
            event_txs,
            dependency_set: Arc::new(depset),
            db_factory: temp_factory(),
        };

        let block = Block {
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
        let mut last_finalized_number = 0;
        watcher.handle_new_finalized_block(block.clone(), &mut last_finalized_number);

        let event = rx.recv().await.unwrap();
        let expected = BlockInfo::new(
            block.header.hash,
            block.header.number,
            block.header.parent_hash,
            block.header.timestamp,
        );
        assert!(
            matches!(event, ChainEvent::FinalizedSourceUpdate { ref finalized_source_block } if *finalized_source_block == expected),
            "Expected FinalizedSourceUpdate with block {:?}, got {:?}",
            expected,
            event
        );
    }

    #[tokio::test]
    async fn test_handle_new_finalized_block_storage_error() {
        let (tx, mut rx) = mpsc::channel(1);
        let event_txs = [(1, tx)].into_iter().collect();

        let mut mock_storage = Mockfinalized_l1_storage::new();
        mock_storage
            .expect_update_finalized_l1()
            .returning(|_block| Err(StorageError::DatabaseNotInitialised));

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter);
        let rpc_client = RpcClient::new(transport, false);

        let depset = DependencySet {
            dependencies: HashMap::default(),
            override_message_expiry_window: Some(3600),
        };

        let watcher = L1Watcher {
            rpc_client,
            cancellation: CancellationToken::new(),
            finalized_l1_storage: Arc::new(mock_storage),
            event_txs,
            dependency_set: Arc::new(depset),
            db_factory: temp_factory(),
        };

        let block = Block {
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
        let mut last_finalized_number = 0;
        watcher.handle_new_finalized_block(block, &mut last_finalized_number);

        // Should NOT broadcast if storage update fails
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_handle_new_latest_block_updates() {
        let (tx, mut rx) = mpsc::channel(1);
        let event_txs = [(1, tx)].into_iter().collect();

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter);
        let rpc_client = RpcClient::new(transport, false);

        let depset = DependencySet {
            dependencies: HashMap::default(),
            override_message_expiry_window: Some(3600),
        };

        let watcher = L1Watcher {
            rpc_client,
            cancellation: CancellationToken::new(),
            finalized_l1_storage: Arc::new(Mockfinalized_l1_storage::new()),
            event_txs,
            dependency_set: Arc::new(depset),
            db_factory: temp_factory(),
        };

        let block = Block {
            header: Header {
                hash: B256::ZERO,
                inner: alloy_consensus::Header {
                    number: 1,
                    parent_hash: B256::ZERO,
                    timestamp: 123456,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let mut last_latest_number = BlockNumHash { number: 0, hash: B256::ZERO };
        watcher.handle_new_latest_block(block, &mut last_latest_number).await;
        assert_eq!(last_latest_number.number, 1);
        // Should NOT send any event for latest block
        assert!(rx.try_recv().is_err());
    }
}
