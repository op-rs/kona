use crate::syncnode::NodeEvent;
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use kona_supervisor_types::BlockReplacement;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Represents a task that processes chain events from a managed node.
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessorTask {
    cancel_token: CancellationToken,

    /// The channel for receiving node events.
    event_rx: mpsc::Receiver<NodeEvent>,
}

impl ChainProcessorTask {
    /// Creates a new [`ChainProcessorTask`].
    pub const fn new(cancel_token: CancellationToken, event_rx: mpsc::Receiver<NodeEvent>) -> Self {
        Self { cancel_token, event_rx }
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

    async fn handle_unsafe_event(&self, _block_info: BlockInfo) {
        // Logic to handle unsafe events
    }
}
