use crate::syncnode::{ManagedNode, NodeEvent};
use alloy_primitives::ChainId;
use tokio::sync::{mpsc, watch};
use tracing::warn;

/// Responsible for managing [`ManagedNode`] and processing [`NodeEvent`].
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessor {
    // The chainId that this processor is associated with
    chain_id: ChainId,

    // The managed node that this processor will handle
    managed_node: Option<ManagedNode>,

    // Channels for sending and receiving node events
    event_tx: mpsc::Sender<NodeEvent>,
    event_rx: mpsc::Receiver<NodeEvent>,

    // Channel for shutdown signal
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

impl ChainProcessor {
    /// Creates a new instance of [`ChainProcessor`].
    pub fn new(chain_id: ChainId) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self { chain_id, managed_node: None, event_tx, event_rx, shutdown_tx, shutdown_rx }
    }

    /// Returns the chain ID associated with this processor.
    pub const fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Adds a managed node to the processor.
    // added the method to allow the processor to manage multiple managed nodes in the future
    pub async fn add_managed_node(&mut self, managed_node: ManagedNode) {
        // todo: check if the managed node chain ID matches the processor's chain ID
        self.managed_node = Some(managed_node);
    }

    /// Starts the chain processor, which begins listening for events from the managed node.
    pub async fn start(&mut self) {
        if let Some(managed_node) = &mut self.managed_node {
            managed_node.start_subscription(self.event_tx.clone()).await.unwrap();
        }

        loop {
            tokio::select! {
                maybe_event = self.event_rx.recv() => {
                    if let Some(event) = maybe_event {
                        self.handle_event(event).await;
                    }
                }
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        // Shutdown signal received
                        break;
                    }
                }
            }
        }
    }

    /// Triggers a graceful shutdown of the processor.
    pub async fn shutdown(&mut self) {
        if let Some(managed_node) = &mut self.managed_node {
            // Stop the managed node's subscription
            if let Err(err) = managed_node.stop_subscription().await {
                warn!(target: "chain_processor", %err, "Failed to stop managed node subscription");
            }
        }
        let _ = self.shutdown_tx.send(true);
    }

    async fn handle_event(&self, event: NodeEvent) {
        match event {
            NodeEvent::UnsafeBlock { block: _ } => {
                // Handle unsafe block
            }
            NodeEvent::DerivedBlock { derived_ref_pair: _ } => {
                // Handle derived block
            }
            NodeEvent::BlockReplaced { replacement: _ } => {
                // Handle block replacement
            }
        }
    }
}
