use super::{ChainProcessorError, ChainProcessorTask};
use crate::{event::ChainEvent, syncnode::ManagedNodeProvider};
use alloy_primitives::ChainId;
use kona_supervisor_storage::{DerivationStorageWriter, HeadRefStorageWriter, LogStorageWriter};
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
    // The chainId that this processor is associated with
    chain_id: ChainId,

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
    W: LogStorageWriter + DerivationStorageWriter + HeadRefStorageWriter + 'static,
{
    /// Creates a new instance of [`ChainProcessor`].
    pub fn new(
        chain_id: ChainId,
        managed_node: Arc<P>,
        state_manager: Arc<W>,
        cancel_token: CancellationToken,
    ) -> Self {
        // todo: validate chain_id against managed_node
        Self { chain_id, managed_node, state_manager, cancel_token, task_handle: Mutex::new(None) }
    }

    /// Returns the [`ChainId`] associated with this processor.
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Starts the chain processor, which begins listening for events from the managed node.
    pub async fn start(&self) -> Result<(), ChainProcessorError> {
        let mut handle_guard = self.task_handle.lock().await;
        if handle_guard.is_some() {
            warn!(target: "chain_processor", "ChainProcessor is already running");
            return Ok(())
        }

        // todo: figure out value for buffer size
        let (event_tx, event_rx) = mpsc::channel::<ChainEvent>(100);
        self.managed_node.start_subscription(event_tx).await?;

        let task = ChainProcessorTask::new(
            self.chain_id,
            self.managed_node.clone(),
            self.state_manager.clone(),
            self.cancel_token.clone(),
            event_rx,
        );
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
        syncnode::{ManagedNodeError, NodeSubscriber, ReceiptProvider},
    };
    use alloy_primitives::B256;
    use async_trait::async_trait;
    use kona_interop::{DerivedRefPair, SafetyLevel};
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{
        DerivationStorageWriter, HeadRefStorageWriter, LogStorageWriter, StorageError,
    };
    use kona_supervisor_types::{Log, Receipts};
    use mockall::mock;
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    use tokio::time::sleep;

    #[derive(Debug)]
    struct MockNode {
        subscribed: Arc<AtomicBool>,
    }

    impl MockNode {
        fn new() -> Self {
            Self { subscribed: Arc::new(AtomicBool::new(false)) }
        }
    }

    #[async_trait]
    impl NodeSubscriber for MockNode {
        async fn start_subscription(
            &self,
            _tx: mpsc::Sender<ChainEvent>,
        ) -> Result<(), ManagedNodeError> {
            self.subscribed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[async_trait]
    impl ReceiptProvider for MockNode {
        async fn fetch_receipts(&self, _block_hash: B256) -> Result<Receipts, ManagedNodeError> {
            Ok(vec![]) // dummy
        }
    }

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

            fn update_safety_head_ref(
                &self,
                safety_level: SafetyLevel,
                block_info: &BlockInfo,
            ) -> Result<(), StorageError>;
        }
    );

    #[tokio::test]
    async fn test_chain_processor_start_sets_task_and_calls_subscription() {
        let mock_node = Arc::new(MockNode::new());
        let storage = Arc::new(MockDb::new());
        let cancel_token = CancellationToken::new();

        let processor =
            ChainProcessor::new(1, Arc::clone(&mock_node), Arc::clone(&storage), cancel_token);

        assert!(processor.start().await.is_ok());

        // Wait a moment for task to spawn and subscription to run
        sleep(Duration::from_millis(50)).await;

        // Ensure start_subscription was called
        assert!(mock_node.subscribed.load(Ordering::SeqCst));

        let handle_guard = processor.task_handle.lock().await;
        assert!(handle_guard.is_some());
    }
}
