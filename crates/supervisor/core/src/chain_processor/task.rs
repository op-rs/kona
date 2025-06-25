use crate::{LogIndexer, event::ChainEvent, syncnode::ManagedNodeProvider};
use alloy_primitives::ChainId;
use kona_interop::{BlockReplacement, DerivedRefPair};
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DerivationStorageWriter, HeadRefStorageWriter, LogStorageWriter};
use std::{fmt::Debug, sync::Arc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

/// Represents a task that processes chain events from a managed node.
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessorTask<P, W> {
    chain_id: ChainId,

    managed_node: Arc<P>,

    state_manager: Arc<W>,

    log_indexer: Arc<LogIndexer<P, W>>,

    cancel_token: CancellationToken,

    /// The channel for receiving node events.
    event_rx: mpsc::Receiver<ChainEvent>,
}

impl<P, W> ChainProcessorTask<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: LogStorageWriter + DerivationStorageWriter + HeadRefStorageWriter + 'static,
{
    /// Creates a new [`ChainProcessorTask`].
    pub fn new(
        chain_id: u64,
        managed_node: Arc<P>,
        state_manager: Arc<W>,
        cancel_token: CancellationToken,
        event_rx: mpsc::Receiver<ChainEvent>,
    ) -> Self {
        let log_indexer = LogIndexer::new(managed_node.clone(), state_manager.clone());
        Self {
            chain_id,
            cancel_token,
            managed_node,
            event_rx,
            state_manager,
            log_indexer: Arc::from(log_indexer),
        }
    }

    /// Runs the chain processor task, which listens for events and processes them.
    /// This method will run indefinitely until the cancellation token is triggered.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                maybe_event = self.event_rx.recv() => {
                    if let Some(event) = maybe_event {
                        self.handle_event(event).await;
                    }
                }
                _ = self.cancel_token.cancelled() => {
                    info!(
                        target: "chain_processor",
                        chain_id = self.chain_id,
                        "ChainProcessorTask cancellation requested, stopping..."
                    );
                    break;
                }
            }
        }
    }

    async fn handle_event(&self, event: ChainEvent) {
        match event {
            ChainEvent::UnsafeBlock { block } => self.handle_unsafe_event(block).await,
            ChainEvent::DerivedBlock { derived_ref_pair } => {
                self.handle_safe_event(derived_ref_pair).await
            }
            ChainEvent::DerivationOriginUpdate { origin } => {
                self.handle_derivation_origin_update(origin)
            }
            ChainEvent::BlockReplaced { replacement } => {
                self.handle_block_replacement(replacement).await
            }
            ChainEvent::FinalizedSourceUpdate { finalized_source_block } => {
                self.handle_finalized_l1_update(finalized_source_block).await
            }
        }
    }

    async fn handle_block_replacement(&self, _replacement: BlockReplacement) {
        // Logic to handle block replacement
    }

    async fn handle_finalized_l1_update(&self, finalized_source_block: BlockInfo) {
        debug!(
            target: "chain_processor",
            chain_id = self.chain_id,
            block_number = finalized_source_block.number,
            "Processing finalized L1 update"
        );
        let finalized_derived_block =
            match self.state_manager.update_finalized_using_source(finalized_source_block) {
                Ok(finalized_l2) => finalized_l2,
                Err(err) => {
                    error!(
                        target: "chain_processor",
                        chain_id = self.chain_id,
                        block_number = finalized_source_block.number,
                        %err,
                        "Failed to update finalized L1 block"
                    );
                    return;
                }
            };

        if let Err(err) = self.managed_node.update_finalized(finalized_derived_block.id()).await {
            error!(
                target: "chain_processor",
                chain_id = self.chain_id,
                block_number = finalized_source_block.number,
                %err,
                "Failed to update finalized L2 block on managed node"
            );
        }
    }

    fn handle_derivation_origin_update(&self, origin: BlockInfo) {
        debug!(
            target: "chain_processor",
            chain_id = self.chain_id,
            block_number = origin.number,
            "Processing derivation origin update"
        );
        if let Err(err) = self.state_manager.update_current_l1(origin) {
            error!(
                target: "chain_processor",
                chain_id = self.chain_id,
                block_number = origin.number,
                %err,
                "Failed to update current L1 block"
            );
        }
    }

    async fn handle_safe_event(&self, derived_ref_pair: DerivedRefPair) {
        debug!(
            target: "chain_processor",
            chain_id = self.chain_id,
            block_number = derived_ref_pair.derived.number,
            "Processing local safe derived block pair"
        );
        if let Err(err) = self.state_manager.save_derived_block_pair(derived_ref_pair) {
            error!(
                target: "chain_processor",
                chain_id = self.chain_id,
                block_number = derived_ref_pair.derived.number,
                %err,
                "Failed to process safe block"
            );
            // TODO: take next action based on the error
        }
    }

    async fn handle_unsafe_event(&self, block_info: BlockInfo) {
        debug!(
            target: "chain_processor",
            chain_id = self.chain_id,
            block_number = block_info.number,
            "Processing unsafe block"
        );

        if let Err(err) = self.log_indexer.process_and_store_logs(&block_info).await {
            error!(
                target: "chain_processor",
                chain_id = self.chain_id,
                block_number = block_info.number,
                %err,
                "Failed to process unsafe block"
            );
            // TODO: take next action based on the error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        event::ChainEvent,
        syncnode::{ManagedNodeApiProvider, ManagedNodeError, NodeSubscriber, ReceiptProvider},
    };
    use alloy_primitives::B256;
    use alloy_rpc_types_eth::BlockNumHash;
    use async_trait::async_trait;
    use kona_interop::{DerivedRefPair, SafetyLevel};
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{
        DerivationStorageWriter, HeadRefStorageWriter, LogStorageWriter, StorageError,
    };
    use kona_supervisor_types::{Log, OutputV0, Receipts};
    use mockall::mock;
    use std::time::Duration;
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
    impl ReceiptProvider for Node {
        async fn fetch_receipts(&self, _block_hash: B256) -> Result<Receipts, ManagedNodeError>;
    }

    #[async_trait]
    impl ManagedNodeApiProvider for Node {
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

        async fn update_finalized(
            &self,
            _finalized_block_id: BlockNumHash,
        ) -> Result<(), ManagedNodeError>;
    }
    );

    mock!(
        #[derive(Debug)]
        pub Db {}

        impl LogStorageWriter for Db {
            fn store_block_logs(
                &self,
                block: &BlockInfo,
                logs: Vec<Log>,
            ) -> Result<(), StorageError>;
        }

        impl DerivationStorageWriter for Db {
            fn save_derived_block_pair(
                &self,
                incoming_pair: DerivedRefPair,
            ) -> Result<(), StorageError>;
        }

        impl HeadRefStorageWriter for Db {
            fn update_current_l1(
                &self,
                block_info: BlockInfo,
            ) -> Result<(), StorageError>;

            fn update_finalized_using_source(
                &self,
                block_info: BlockInfo,
            ) -> Result<BlockInfo, StorageError>;

            fn update_safety_head_ref(
                &self,
                safety_level: SafetyLevel,
                block_info: &BlockInfo,
            ) -> Result<(), StorageError>;
        }
    );

    #[tokio::test]
    async fn test_handle_unsafe_event_triggers() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();

        // Send unsafe block event
        let block = BlockInfo::new(B256::ZERO, 123, B256::ZERO, 0);

        mockdb.expect_store_block_logs().returning(move |_block, _log| Ok(()));
        mocknode.expect_fetch_receipts().returning(move |block_hash| {
            assert!(block_hash == block.hash);
            Ok(Receipts::default())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let task = ChainProcessorTask::new(1, managed_node, writer, cancel_token.clone(), rx);

        tx.send(ChainEvent::UnsafeBlock { block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derived_event_triggers() {
        let block_pair = DerivedRefPair {
            source: BlockInfo {
                number: 123,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
            derived: BlockInfo {
                number: 1234,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
        };

        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();

        mockdb.expect_save_derived_block_pair().returning(move |_pair: DerivedRefPair| {
            assert_eq!(_pair, block_pair);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let task = ChainProcessorTask::new(1, managed_node, writer, cancel_token.clone(), rx);

        // Send unsafe block event
        tx.send(ChainEvent::DerivedBlock { derived_ref_pair: block_pair }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derivation_origin_update_triggers() {
        let origin =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();

        let origin_clone = origin;
        mockdb.expect_update_current_l1().returning(move |block_info: BlockInfo| {
            assert_eq!(block_info, origin_clone);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let task = ChainProcessorTask::new(1, managed_node, writer, cancel_token.clone(), rx);

        // Send derivation origin update event
        tx.send(ChainEvent::DerivationOriginUpdate { origin }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_finalized_source_update_triggers() {
        let finalized_source_block =
            BlockInfo { number: 99, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        let mut mocknode = MockNode::new();
        let mut mockdb = MockDb::new();

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

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let task = ChainProcessorTask::new(1, managed_node, writer, cancel_token.clone(), rx);

        // Send FinalizedSourceUpdate event
        tx.send(ChainEvent::FinalizedSourceUpdate { finalized_source_block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_finalized_source_update_db_error() {
        let finalized_source_block =
            BlockInfo { number: 99, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        let mut mocknode = MockNode::new();
        let mut mockdb = MockDb::new();

        // DB returns error
        mockdb
            .expect_update_finalized_using_source()
            .returning(|_block_info: BlockInfo| Err(StorageError::DatabaseNotInitialised));

        // Managed node's update_finalized should NOT be called
        mocknode.expect_update_finalized().never();

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let task = ChainProcessorTask::new(1, managed_node, writer, cancel_token.clone(), rx);

        // Send FinalizedSourceUpdate event
        tx.send(ChainEvent::FinalizedSourceUpdate { finalized_source_block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }
}
