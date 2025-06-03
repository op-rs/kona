use crate::{
    LogIndexer,
    syncnode::{ManagedNodeProvider, NodeEvent},
};
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::LogStorageWriter;
use kona_supervisor_types::BlockReplacement;
use std::{fmt::Debug, sync::Arc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Represents a task that processes chain events from a managed node.
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessorTask<
    P: ManagedNodeProvider,
    W: LogStorageWriter, // TODO: replace with more wider traits to cover others
> {
    log_indexer: Arc<LogIndexer<P, W>>,

    cancel_token: CancellationToken,

    /// The channel for receiving node events.
    event_rx: mpsc::Receiver<NodeEvent>,
}

impl<P, W> ChainProcessorTask<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: LogStorageWriter + 'static,
{
    /// Creates a new [`ChainProcessorTask`].
    pub fn new(
        managed_node: Arc<P>,
        state_manager: Arc<W>,
        cancel_token: CancellationToken,
        event_rx: mpsc::Receiver<NodeEvent>,
    ) -> Self {
        Self {
            cancel_token,
            event_rx,
            log_indexer: Arc::from(LogIndexer::new(managed_node, state_manager)),
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
                _ = self.cancel_token.cancelled() => break,
            }
        }
    }

    async fn handle_event(&self, event: NodeEvent) {
        match event {
            NodeEvent::UnsafeBlock { block } => self.handle_unsafe_event(block).await,
            NodeEvent::DerivedBlock { derived_ref_pair } => {
                self.handle_safe_event(derived_ref_pair).await
            }
            NodeEvent::BlockReplaced { replacement } => {
                self.handle_block_replacement(replacement).await
            }
        }
    }

    async fn handle_block_replacement(&self, _replacement: BlockReplacement) {
        // Logic to handle block replacement
    }

    async fn handle_safe_event(&self, _derived_ref_pair: DerivedRefPair) {
        // Logic to handle safe events
    }

    async fn handle_unsafe_event(&self, block_info: BlockInfo) {
        info!(
            target: "chain_processor",
            block_number = block_info.number,
            "Processing unsafe block"
        );
        if let Err(err) = self.log_indexer.process_and_store_logs(&block_info).await {
            error!(
                target: "chain_processor",
                block_number = block_info.number,
                %err,
                "Failed to process unsafe block"
            );
            // TODO: take next action based on the error
        }
    }
}
