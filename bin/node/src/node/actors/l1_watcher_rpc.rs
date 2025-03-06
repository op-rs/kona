//! [NodeActor] implementation for an L1 chain watcher that checks for L1 head updates over RPC.

use crate::node::{NodeActor, NodeEvent};
use alloy_network::primitives::BlockTransactionsKind;
use alloy_primitives::B256;
use alloy_provider::{Provider, RootProvider};
use async_trait::async_trait;
use futures::StreamExt;
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
        Self { l1_provider, sender, cancellation }
    }

    /// Fetches the block info for the current L1 head.
    async fn block_info_by_hash(
        &mut self,
        block_hash: B256,
    ) -> Result<BlockInfo, L1WatcherRpcError<NodeEvent>> {
        // Fetch the block of the current unsafe L1 head.
        let block = self
            .l1_provider
            .get_block_by_hash(block_hash, BlockTransactionsKind::Hashes)
            .await
            .map_err(|e| L1WatcherRpcError::Transport(e.to_string()))?
            .ok_or(L1WatcherRpcError::L1BlockNotFound(block_hash))?;

        // Update the last observed head. The producer does not care about re-orgs, as this is
        // handled downstream by receivers of the head update signal.
        let head_block_info = BlockInfo {
            hash: block.header.hash,
            number: block.header.number,
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
        };

        Ok(head_block_info)
    }
}

#[async_trait]
impl NodeActor for L1WatcherRpc {
    type Event = NodeEvent;
    type Error = L1WatcherRpcError<Self::Event>;

    async fn start(mut self) -> Result<(), Self::Error> {
        let mut unsafe_head_stream = self
            .l1_provider
            .watch_blocks()
            .await
            .map_err(|e| L1WatcherRpcError::Transport(e.to_string()))?
            .into_stream()
            .flat_map(futures::stream::iter);

        loop {
            select! {
                _ = self.cancellation.cancelled() => {
                    // Exit the task on cancellation.
                    info!(
                        target: "l1_watcher",
                        "Received shutdown signal. Exiting L1 watcher task."
                    );
                    return Ok(());
                }
                new_head = unsafe_head_stream.next() => match new_head {
                    None => {
                        // The stream ended, which should never happen.
                        return Err(L1WatcherRpcError::Transport(
                            "L1 block stream ended unexpectedly".to_string(),
                        ));
                    }
                    Some(new_head) => {
                        // Send the head update event to all consumers.
                        let head_block_info = self.block_info_by_hash(new_head).await?;
                        self.sender.send(NodeEvent::L1HeadUpdate(head_block_info))?;
                    },
                }
            }
        }

        Ok(())
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
    L1BlockNotFound(B256),
    /// Nothing to update.
    #[error("Nothing to update; L1 head is the same as the last observed head")]
    NothingToUpdate,
}
