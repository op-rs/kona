//! [SupervisorActor] implementation for an L1 chain watcher that checks for L1 head updates over
//! RPC.

use crate::actors::traits::SupervisorActor;
use alloy_eips::BlockId;
use alloy_primitives::B256;
use alloy_provider::{Provider, RootProvider};
use async_trait::async_trait;
use futures::StreamExt;
use kona_genesis::RollupConfig;
use kona_protocol::BlockInfo;
use kona_rpc::{L1State, L1WatcherQueries};
use std::{mem, sync::Arc};
use thiserror::Error;
use tokio::{
    select,
    sync::{
        mpsc::{UnboundedSender, error::SendError},
        watch,
    },
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// An L1 chain watcher that checks for L1 head updates over RPC.
#[derive(Debug)]
pub struct L1WatcherRpc {
    /// The [`RollupConfig`] to tell if ecotone is active.
    /// This is used to determine if the L1 watcher should
    /// check for unsafe block signer updates.
    config: Arc<RollupConfig>,
    /// The L1 provider.
    l1_provider: RootProvider,
    /// The latest L1 head sent to the derivation pipeline and watcher. Can be subscribed to, in
    /// order to get the state from the external watcher.
    latest_head: watch::Sender<Option<BlockInfo>>,
    /// The outbound event sender.
    head_sender: UnboundedSender<BlockInfo>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
    /// Inbound queries to the L1 watcher.
    inbound_queries: Option<tokio::sync::mpsc::Receiver<L1WatcherQueries>>,
}

impl L1WatcherRpc {
    /// Creates a new [`L1WatcherRpc`] instance.
    pub fn new(
        config: Arc<RollupConfig>,
        l1_provider: RootProvider,
        head_sender: UnboundedSender<BlockInfo>,
        cancellation: CancellationToken,
        // Can be None if we disable communication with the L1 watcher.
        inbound_queries: Option<tokio::sync::mpsc::Receiver<L1WatcherQueries>>,
    ) -> Self {
        let (head_updates, _) = tokio::sync::watch::channel(None);
        Self {
            config,
            l1_provider,
            head_sender,
            latest_head: head_updates,
            cancellation,
            inbound_queries,
        }
    }

    /// Fetches the block info for the current L1 head.
    async fn block_info_by_hash(
        &self,
        block_hash: B256,
    ) -> Result<BlockInfo, L1WatcherRpcError<BlockInfo>> {
        // Fetch the block of the current unsafe L1 head.
        let block = self
            .l1_provider
            .get_block_by_hash(block_hash)
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

    /// Spins up a task to process inbound queries.
    fn start_query_processor(
        &self,
        mut inbound_queries: tokio::sync::mpsc::Receiver<L1WatcherQueries>,
    ) -> JoinHandle<()> {
        // Start the inbound query processor in a separate task to avoid blocking the main task.
        // We can cheaply clone the l1 provider here because it is an Arc.
        let l1_provider = self.l1_provider.clone();
        let head_updates_recv = self.latest_head.subscribe();
        let rollup_config = self.config.clone();

        tokio::spawn(async move {
            while let Some(query) = inbound_queries.recv().await {
                match query {
                    L1WatcherQueries::Config(sender) => {
                        if let Err(error) = sender.send((*rollup_config).clone()) {
                            warn!(target: "l1_watcher", ?error, "Failed to send L1 config to the query sender");
                        }
                    }
                    L1WatcherQueries::L1State(sender) => {
                        let current_l1 = *head_updates_recv.borrow();

                        let head_l1 = match l1_provider.get_block(BlockId::latest()).await {
                            Ok(block) => block,
                            Err(e) => {
                                warn!(target: "l1_watcher", error = ?e, "failed to query l1 provider for latest head block");
                                None
                            }}.map(|block| block.into_consensus().into());

                        let finalized_l1 = match l1_provider.get_block(BlockId::finalized()).await {
                            Ok(block) => block,
                            Err(e) => {
                                warn!(target: "l1_watcher", error = ?e, "failed to query l1 provider for latest finalized block");
                                None
                            }}.map(|block| block.into_consensus().into());

                        let safe_l1 = match l1_provider.get_block(BlockId::safe()).await {
                            Ok(block) => block,
                            Err(e) => {
                                warn!(target: "l1_watcher", error = ?e, "failed to query l1 provider for latest safe block");
                                None
                            }}.map(|block| block.into_consensus().into());

                        if let Err(e) = sender.send(L1State {
                            current_l1,
                            current_l1_finalized: finalized_l1,
                            head_l1,
                            safe_l1,
                            finalized_l1,
                        }) {
                            warn!(target: "l1_watcher", error = ?e, "Failed to send L1 state to the query sender");
                        }
                    }
                }
            }

            error!(target: "l1_watcher", "L1 watcher query channel closed unexpectedly, exiting query processor task.");
        })
    }
}

#[async_trait]
impl SupervisorActor for L1WatcherRpc {
    type InboundEvent = ();
    type Error = L1WatcherRpcError<BlockInfo>;

    async fn start(mut self) -> Result<(), Self::Error> {
        let mut unsafe_head_stream = self
            .l1_provider
            .watch_blocks()
            .await
            .map_err(|e| L1WatcherRpcError::Transport(e.to_string()))?
            .into_stream()
            .flat_map(futures::stream::iter);

        let inbound_queries = mem::take(&mut self.inbound_queries);
        let inbound_query_processor =
            inbound_queries.map(|queries| self.start_query_processor(queries));

        // Start the main processing loop.
        loop {
            select! {
                _ = self.cancellation.cancelled() => {
                    // Exit the task on cancellation.
                    info!(
                        target: "l1_watcher",
                        "Received shutdown signal. Exiting L1 watcher task."
                    );

                    // Kill the inbound query processor.
                    if let Some(inbound_query_processor) = inbound_query_processor { inbound_query_processor.abort() }

                    return Ok(());
                }
                new_head = unsafe_head_stream.next() => {
                    let Some(new_head) = new_head else {
                        // The stream ended, which should never happen.
                        return Err(L1WatcherRpcError::Transport(
                            "L1 block stream ended unexpectedly".to_string(),
                        ));
                    };
                    // Send the head update event to all consumers.
                    let head_block_info = self.block_info_by_hash(new_head).await?;
                    self.head_sender.send(head_block_info)?;
                    self.latest_head.send_replace(Some(head_block_info));
                }
            }
        }
    }
}

/// The error type for the [`L1WatcherRpc`].
#[derive(Error, Debug)]
pub enum L1WatcherRpcError<T> {
    /// Error sending the head update event.
    #[error("error sending the head update event: {0}")]
    SendError(#[from] SendError<T>),
    /// Error in the transport layer.
    #[error("transport error: {0}")]
    Transport(String),
    /// The L1 block was not found.
    #[error("l1 block not found: {0}")]
    L1BlockNotFound(B256),
    /// Nothing to update.
    #[error("nothing to update; l1 head is the same as the last observed head")]
    NothingToUpdate,
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_provider::{RootProvider, mock::Asserter};
    use alloy_rpc_client::RpcClient;
    use std::sync::Arc;
    use tokio::sync::{mpsc, oneshot};

    #[tokio::test]
    async fn test_l1_watcher_creation() {
        let config = Arc::new(RollupConfig::default());
        let provider = RootProvider::new(RpcClient::mocked(Asserter::new()));
        let (head_sender, _) = mpsc::unbounded_channel();
        let cancellation = CancellationToken::new();
        let (_query_sender, query_receiver) = mpsc::channel(1);

        let watcher =
            L1WatcherRpc::new(config, provider, head_sender, cancellation, Some(query_receiver));

        assert!(watcher.inbound_queries.is_some());
    }

    #[tokio::test]
    async fn test_query_processor_config() {
        let config = Arc::new(RollupConfig::default());
        let provider = RootProvider::new(RpcClient::mocked(Asserter::new()));
        let (head_sender, _) = mpsc::unbounded_channel();
        let cancellation = CancellationToken::new();
        let (query_sender, query_receiver) = mpsc::channel(1);

        let watcher = L1WatcherRpc::new(config.clone(), provider, head_sender, cancellation, None);

        let (response_sender, response_receiver) = oneshot::channel();
        query_sender.send(L1WatcherQueries::Config(response_sender)).await.unwrap();

        // Start the query processor
        let processor = watcher.start_query_processor(query_receiver);
        // Wait for the response
        let received_config = response_receiver.await.unwrap();
        assert_eq!(received_config, *config);

        processor.abort();
    }

    #[tokio::test]
    async fn test_l1_state_query() {
        let config = Arc::new(RollupConfig::default());
        let provider = RootProvider::new(RpcClient::mocked(Asserter::new()));
        let (head_sender, _) = mpsc::unbounded_channel();
        let cancellation = CancellationToken::new();
        let (query_sender, query_receiver) = mpsc::channel(1);

        let watcher = L1WatcherRpc::new(config.clone(), provider, head_sender, cancellation, None);

        let (response_sender, response_receiver) = oneshot::channel();
        query_sender.send(L1WatcherQueries::L1State(response_sender)).await.unwrap();

        // Start the query processor
        let processor = watcher.start_query_processor(query_receiver);
        // Wait for the response
        let state = response_receiver.await.unwrap();
        assert!(state.current_l1.is_none()); // Initially should be None
        assert!(state.head_l1.is_none());
        assert!(state.safe_l1.is_none());
        assert!(state.finalized_l1.is_none());

        processor.abort();
    }
}
