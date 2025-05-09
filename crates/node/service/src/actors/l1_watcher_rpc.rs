//! [NodeActor] implementation for an L1 chain watcher that checks for L1 head updates over RPC.

use crate::NodeActor;
use alloy_eips::BlockId;
use alloy_primitives::{Address, B256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types_eth::Log;
use async_trait::async_trait;
use futures::StreamExt;
use kona_genesis::{RollupConfig, SystemConfigLog, SystemConfigUpdate, UnsafeBlockSignerUpdate};
use kona_protocol::BlockInfo;
use kona_rpc::{L1State, L1WatcherQueries};
use std::sync::Arc;
use thiserror::Error;
use tokio::{
    select,
    sync::mpsc::{UnboundedSender, error::SendError},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

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
    latest_head: tokio::sync::watch::Sender<Option<BlockInfo>>,
    /// The outbound event sender.
    head_sender: UnboundedSender<BlockInfo>,
    /// The block signer sender.
    block_signer_sender: UnboundedSender<Address>,
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
        block_signer_sender: UnboundedSender<Address>,
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
            block_signer_sender,
            cancellation,
            inbound_queries,
        }
    }

    /// Fetches logs for the given block hash.
    async fn fetch_logs(
        &mut self,
        block_hash: B256,
    ) -> Result<Vec<Log>, L1WatcherRpcError<BlockInfo>> {
        let logs = self
            .l1_provider
            .get_logs(&alloy_rpc_types_eth::Filter::new().select(block_hash))
            .await
            .map_err(|e| L1WatcherRpcError::Transport(format!("Failed to fetch logs: {}", e)))?;

        Ok(logs)
    }

    /// Fetches the block info for the current L1 head.
    async fn block_info_by_hash(
        &mut self,
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
                        if let Err(e) = sender.send((*rollup_config).clone()) {
                            warn!(target: "l1_watcher", error = ?e, "Failed to send L1 config to the query sender");
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
impl NodeActor for L1WatcherRpc {
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

        let inbound_queries = std::mem::take(&mut self.inbound_queries);
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
                        self.head_sender.send(head_block_info)?;
                        self.latest_head.send_replace(Some(head_block_info));

                        // For each log, attempt to construct a `SystemConfigLog`.
                        // Build the `SystemConfigUpdate` from the log.
                        // If the update is an Unsafe block signer update, send the address
                        // to the block signer sender.
                        let logs = self.fetch_logs(new_head).await?;
                        let ecotone_active = self.config.is_ecotone_active(head_block_info.timestamp);
                        for log in logs {
                            let sys_cfg_log = SystemConfigLog::new(log.into(), ecotone_active);
                            if let Ok(SystemConfigUpdate::UnsafeBlockSigner(UnsafeBlockSignerUpdate { unsafe_block_signer })) = sys_cfg_log.build() {
                                info!(
                                    target: "l1_watcher",
                                    "Unsafe block signer update: {unsafe_block_signer}"
                                );
                                if let Err(e) = self.block_signer_sender.send(unsafe_block_signer) {
                                    error!(
                                        target: "l1_watcher",
                                        "Error sending unsafe block signer update: {e}"
                                    );
                                }
                            }
                        }
                    },
                }
            }
        }
    }

    async fn process(&mut self, _msg: Self::InboundEvent) -> Result<(), Self::Error> {
        // The L1 watcher does not process any incoming messages.
        Ok(())
    }
}

/// The error type for the [L1WatcherRpc].
#[derive(Error, Debug)]
pub enum L1WatcherRpcError<T> {
    /// Error sending the head update event.
    #[error("Error sending the head update event: {0}")]
    SendError(#[from] SendError<T>),
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
