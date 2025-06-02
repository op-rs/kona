use super::{ChainProcessorError, task::ChainProcessorTask};
use crate::syncnode::{ManagedNode, NodeEvent};
use alloy_primitives::ChainId;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Responsible for managing [`ManagedNode`] and processing
/// [`NodeEvent`]. It listens for events emitted by the managed node
/// and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessor {
    // The chainId that this processor is associated with
    chain_id: ChainId,

    // The managed node that this processor will handle
    managed_node: Option<ManagedNode>,

    // Cancellation token to stop the processor
    cancel_token: CancellationToken,

    // Handle for the task running the processor
    task_handle: Option<JoinHandle<()>>,
}

impl ChainProcessor {
    /// Creates a new instance of [`ChainProcessor`].
    pub const fn new(chain_id: ChainId, cancel_token: CancellationToken) -> Self {
        Self { chain_id, managed_node: None, cancel_token, task_handle: None }
    }

    /// Returns the [`ChainId`] associated with this processor.
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Adds a managed node to the processor.
    // added the method to allow the processor to manage multiple managed nodes in the future
    pub async fn add_managed_node(
        &mut self,
        managed_node: ManagedNode,
    ) -> Result<(), ChainProcessorError> {
        // check if chain_id matches
        let m_chain_id = managed_node.chain_id().await?;
        if m_chain_id != self.chain_id {
            warn!(target: "chain_processor",
                managed_node_chain_id = m_chain_id,
                processor_chain_id = self.chain_id,
                "ManagedNode chain_id does not match ChainProcessor chain_id",
            );
            return Err(ChainProcessorError::InvalidChainId);
        }

        self.managed_node = Some(managed_node);
        Ok(())
    }

    /// Starts the chain processor, which begins listening for events from the managed node.
    pub async fn start(&mut self) {
        if self.task_handle.is_some() {
            warn!(target: "chain_processor", "ChainProcessor is already running");
            return;
        }

        // todo: figure out value for buffer size
        let (event_tx, event_rx) = mpsc::channel::<NodeEvent>(100);
        if let Some(managed_node) = &mut self.managed_node {
            managed_node.start_subscription(event_tx).await.unwrap();
        }

        let task = ChainProcessorTask::new(self.cancel_token.clone(), event_rx);
        let handle = tokio::spawn(async move {
            task.run().await;
        });

        self.task_handle = Some(handle);
    }
}
