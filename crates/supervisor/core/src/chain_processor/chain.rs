use super::{ChainProcessorError, ChainProcessorTask};
use crate::{config::RollupConfig, event::ChainEvent, syncnode::ManagedNodeProvider};
use alloy_primitives::ChainId;
use kona_supervisor_storage::{
    DerivationStorageWriter, HeadRefStorageWriter, LogStorageReader, LogStorageWriter,
};
use std::sync::Arc;
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Responsible for managing [`ManagedNodeProvider`] and processing
/// [`ChainEvent`]. It listens for events emitted by the managed node
/// and handles them accordingly.
// chain processor will support multiple managed nodes in the future.
#[derive(Debug)]
pub struct ChainProcessor<P, W> {
    // The rollup configuration for the chain
    rollup_config: RollupConfig,

    // The chainId that this processor is associated with
    chain_id: ChainId,

    // The sender for chain events, used to communicate with the event loop
    event_tx: Option<mpsc::Sender<ChainEvent>>,

    // Whether metrics are enabled for the processor
    metrics_enabled: Option<bool>,

    // The managed node that this processor will handle
    managed_node: Arc<P>,

    // State manager to update and view state
    state_manager: Arc<W>,

    // Cancellation token to stop the processor
    cancel_token: CancellationToken,

    // Handle for the task running the processor
    task_handle: Mutex<Option<JoinHandle<()>>>,
}

impl<P, W> ChainProcessor<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: LogStorageWriter
        + LogStorageReader
        + DerivationStorageWriter
        + HeadRefStorageWriter
        + 'static,
{
    /// Creates a new instance of [`ChainProcessor`].
    pub fn new(
        rollup_config: RollupConfig,
        chain_id: ChainId,
        managed_node: Arc<P>,
        state_manager: Arc<W>,
        cancel_token: CancellationToken,
    ) -> Self {
        // todo: validate chain_id against managed_node
        Self {
            rollup_config,
            chain_id,
            event_tx: None,
            metrics_enabled: None,
            managed_node,
            state_manager,
            cancel_token,
            task_handle: Mutex::new(None),
        }
    }

    /// Enables metrics on the database environment.
    pub fn with_metrics(mut self) -> Self {
        self.metrics_enabled = Some(true);
        super::Metrics::init(self.chain_id);
        self
    }

    /// Returns the [`ChainId`] associated with this processor.
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Returns the [`mpsc::Sender`] for [`ChainEvent`]s.
    pub fn event_sender(&self) -> Option<mpsc::Sender<ChainEvent>> {
        self.event_tx.clone()
    }

    /// Starts the chain processor, which begins listening for events from the managed node.
    pub async fn start(&mut self) -> Result<(), ChainProcessorError> {
        let mut handle_guard = self.task_handle.lock().await;
        if handle_guard.is_some() {
            warn!(target: "chain_processor", chain_id = %self.chain_id, "ChainProcessor is already running");
            return Ok(());
        }

        // todo: figure out value for buffer size
        let (event_tx, event_rx) = mpsc::channel::<ChainEvent>(1000);
        self.event_tx = Some(event_tx.clone());
        self.managed_node.start_subscription(event_tx.clone()).await?;

        let mut task = ChainProcessorTask::new(
            self.rollup_config.clone(),
            self.chain_id,
            self.managed_node.clone(),
            self.state_manager.clone(),
            self.cancel_token.clone(),
            event_rx,
        );
        if self.metrics_enabled.unwrap_or(false) {
            task = task.with_metrics();
        }

        let handle = tokio::spawn(async move {
            task.run().await;
        });

        *handle_guard = Some(handle);
        Ok(())
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
    use kona_supervisor_storage::{
        DerivationStorageWriter, HeadRefStorageWriter, LogStorageWriter, StorageError,
    };
    use kona_supervisor_types::{BlockSeal, Log, OutputV0, Receipts};
    use mockall::mock;
    use std::time::Duration;
    use tokio::time::sleep;

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
    async fn test_chain_processor_start_sets_task_and_calls_subscription() {
        let mut mock_node = MockNode::new();
        mock_node.expect_start_subscription().returning(|_| Ok(()));

        let storage = Arc::new(MockDb::new());
        let cancel_token = CancellationToken::new();

        let rollup_config = RollupConfig::default();
        let mut processor = ChainProcessor::new(
            rollup_config,
            1,
            Arc::new(mock_node),
            Arc::clone(&storage),
            cancel_token,
        );

        assert!(processor.start().await.is_ok());

        // Wait a moment for task to spawn and subscription to run
        sleep(Duration::from_millis(50)).await;

        let handle_guard = processor.task_handle.lock().await;
        assert!(handle_guard.is_some());
    }
}
