use crate::event::ChainEvent;
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::ChainId;
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_eth::{Block, Header};
use futures::StreamExt;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::FinalizedL1Storage;
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
    finalized_l1_storage: Arc<F>,

    event_txs: HashMap<ChainId, mpsc::Sender<ChainEvent>>,
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
    ) -> Self {
        Self { rpc_client, finalized_l1_storage, event_txs, cancellation }
    }

    /// Starts polling for finalized blocks and processes them.
    pub async fn run(&self) {
        let finalized_head_poller = self
            .rpc_client
            .prepare_static_poller::<_, Block>(
                "eth_getBlockByNumber",
                (BlockNumberOrTag::Finalized, false),
            )
            .with_poll_interval(Duration::from_secs(5));
        let mut finalized_head_stream = finalized_head_poller.into_stream();

        let mut last_finalized_number = 0;

        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    info!(target: "l1_watcher", "L1Watcher cancellation requested, stopping...");
                    break;
                }
                finalized_block = finalized_head_stream.next() => {
                    if let Some(block) = finalized_block {
                        self.handle_new_finalized_block(block, &mut last_finalized_number);
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

    // Mock the FinalizedL1Storage trait
    mock! {
        pub db {}
        impl FinalizedL1Storage for db {
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

        let watcher = L1Watcher {
            rpc_client,
            cancellation: CancellationToken::new(),
            finalized_l1_storage: Arc::new(Mockdb::new()),
            event_txs,
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

        let mut mock_storage = Mockdb::new();
        mock_storage.expect_update_finalized_l1().returning(|_block| Ok(()));

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter);
        let rpc_client = RpcClient::new(transport, false);

        let watcher = L1Watcher {
            rpc_client,
            cancellation: CancellationToken::new(),
            finalized_l1_storage: Arc::new(mock_storage),
            event_txs,
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

        let mut mock_storage = Mockdb::new();
        mock_storage
            .expect_update_finalized_l1()
            .returning(|_block| Err(StorageError::DatabaseNotInitialised));

        let asserter = Asserter::new();
        let transport = MockTransport::new(asserter);
        let rpc_client = RpcClient::new(transport, false);

        let watcher = L1Watcher {
            rpc_client,
            cancellation: CancellationToken::new(),
            finalized_l1_storage: Arc::new(mock_storage),
            event_txs,
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
}
