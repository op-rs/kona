use super::task::ChainProcessorTask;
use crate::syncnode::{ManagedNodeProvider, NodeEvent};
use alloy_primitives::ChainId;
use kona_supervisor_storage::LogStorageWriter;
use std::sync::Arc;
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Responsible for managing [`ManagedNodeProvider`] and processing
/// [`NodeEvent`]. It listens for events emitted by the managed node
/// and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessor<
    P: ManagedNodeProvider,
    W: LogStorageWriter, // TODO: replace with more wider traits to cover others
> {
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
    W: LogStorageWriter + 'static,
{
    /// Creates a new instance of [`ChainProcessor`].
    pub fn new(
        chain_id: ChainId,
        managed_node: Arc<P>,
        state_manager: Arc<W>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self { chain_id, managed_node, state_manager, cancel_token, task_handle: Mutex::new(None) }
    }

    /// Returns the [`ChainId`] associated with this processor.
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Starts the chain processor, which begins listening for events from the managed node.
    pub async fn start(&self) {
        let mut handle_guard = self.task_handle.lock().await;
        if handle_guard.is_some() {
            warn!(target: "chain_processor", "ChainProcessor is already running");
            return;
        }

        // todo: figure out value for buffer size
        let (event_tx, event_rx) = mpsc::channel::<NodeEvent>(100);
        self.managed_node.start_subscription(event_tx).await.unwrap();

        let task = ChainProcessorTask::new(
            Arc::clone(&self.managed_node),
            Arc::clone(&self.state_manager),
            self.cancel_token.clone(),
            event_rx,
        );
        let handle = tokio::spawn(async move {
            task.run().await;
        });

        *handle_guard = Some(handle);
    }
}
