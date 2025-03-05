//! [NodeActor] implementation for an L1 chain watcher that checks for L1 head updates over RPC.

use crate::node::{NodeActor, NodeEvent};
use alloy_network::primitives::BlockTransactionsKind;
use alloy_provider::{Provider, RootProvider};
use async_trait::async_trait;
use kona_protocol::BlockInfo;
use thiserror::Error;
use tokio::{
    select,
    sync::broadcast::{Sender, error::SendError},
};
use tokio_util::sync::CancellationToken;

/// An L1 chain watcher that checks for L1 head updates over RPC.
pub struct L1WatcherRpc {
    /// The L1 provider.
    l1_provider: RootProvider,
    /// The last observed L1 head.
    last_head: Option<BlockInfo>,
    /// The outbound event sender.
    sender: Sender<NodeEvent>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl L1WatcherRpc {
    /// Creates a new [L1WatcherRpc] instance.
    pub fn new(
        l1_provider: RootProvider,
        sender: Sender<NodeEvent>,
        cancellation: CancellationToken,
    ) -> Self {
        Self { l1_provider, last_head: None, sender, cancellation }
    }

    /// Attempts to update the latest observed L1 head.
    async fn update_head(&mut self) -> Result<BlockInfo, L1WatcherRpcError<NodeEvent>> {
        // Fetch the block number of the current unsafe L1 head.
        let head_number = self.l1_provider.get_block_number().await.unwrap();

        // If the head number is the same as the last observed head, skip the update.
        if let Some(block_info) = self.last_head.as_ref() {
            if block_info.number == head_number {
                return Err(L1WatcherRpcError::NothingToUpdate);
            }
        }

        // Fetch the block of the current unsafe L1 head.
        let block = self
            .l1_provider
            .get_block_by_number(head_number.into(), BlockTransactionsKind::Hashes)
            .await
            .map_err(|e| L1WatcherRpcError::Transport(e.to_string()))?
            .ok_or(L1WatcherRpcError::L1BlockNotFound(head_number))?;

        // Update the last observed head. The producer does not care about re-orgs, as this is
        // handled downstream by receivers of the head update signal.
        let head_block_info = BlockInfo {
            hash: block.header.hash,
            number: head_number,
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
        };

        // Update the last observed head.
        self.last_head = Some(head_block_info);

        Ok(head_block_info)
    }
}

#[async_trait]
impl NodeActor for L1WatcherRpc {
    type Event = NodeEvent;
    type Error = L1WatcherRpcError<Self::Event>;

    async fn start(mut self) -> Result<(), Self::Error> {
        // TODO: We can use an exponential backoff strategy here, rather than coupling the ticker
        // to an assumed L1 slot time. This will allow the watcher to be more resilient to network
        // issues and other transient failures.
        let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(12));

        loop {
            // Wait for the next tick, or exit early if a shutdown signal has been received.
            select! {
                _ = ticker.tick() => { /* continue */ },
                _ = self.cancellation.cancelled() => {
                    // Exit the task on cancellation.
                    info!(
                        target: "l1_watcher",
                        "Received shutdown signal. Exiting L1 watcher task."
                    );
                    return Ok(());
                }
            };

            // Attempt to update the L1 head. If for any reason this fails, log the error and
            // continue.
            let head_block_info = match self.update_head().await {
                Ok(head_block_info) => head_block_info,
                Err(e) => {
                    warn!(
                        target: "l1_watcher",
                        "Failed to update L1 head: {e}"
                    );
                    continue;
                }
            };

            // Send the head update event to all consumers.
            self.sender.send(NodeEvent::L1HeadUpdate(head_block_info))?;
        }
    }

    async fn process(&mut self, _msg: Self::Event) -> Result<(), Self::Error> {
        // The L1 watcher does not process any incoming messages.
        Ok(())
    }
}

/// The error type for the [L1WatcherRpc].
#[derive(Error, Debug)]
pub enum L1WatcherRpcError<E> {
    /// Error sending the head update event.
    #[error("Error sending the head update event: {0}")]
    SendError(#[from] SendError<E>),
    /// Error in the transport layer.
    #[error("Transport error: {0}")]
    Transport(String),
    /// The L1 block was not found.
    #[error("L1 block not found: {0}")]
    L1BlockNotFound(u64),
    /// Nothing to update.
    #[error("Nothing to update; L1 head is the same as the last observed head")]
    NothingToUpdate,
}
