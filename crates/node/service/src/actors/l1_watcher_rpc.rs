//! [`NodeActor`] implementation for an L1 chain watcher that polls for L1 block updates over HTTP
//! RPC.

use crate::NodeActor;
use alloy_eips::{BlockId, BlockNumberOrTag};
use alloy_primitives::{Address, B256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_client::PollerBuilder;
use alloy_rpc_types_eth::{Block, Log};
use alloy_transport::TransportError;
use async_stream::stream;
use async_trait::async_trait;
use futures::{Stream, StreamExt};
use kona_genesis::{RollupConfig, SystemConfigLog, SystemConfigUpdate, UnsafeBlockSignerUpdate};
use kona_protocol::BlockInfo;
use kona_rpc::{L1State, L1WatcherQueries};
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use tokio::{
    select,
    sync::mpsc::{UnboundedSender, error::SendError},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

/// An L1 chain watcher that checks for L1 block updates over RPC.
#[derive(Debug)]
pub struct L1WatcherRpc {
    /// The [`RollupConfig`] to tell if ecotone is active.
    /// This is used to determine if the L1 watcher should check for unsafe block signer updates.
    config: Arc<RollupConfig>,
    /// The L1 provider.
    l1_provider: RootProvider,
    /// The latest L1 head sent to the derivation pipeline and watcher. Can be subscribed to, in
    /// order to get the state from the external watcher.
    latest_head: tokio::sync::watch::Sender<Option<BlockInfo>>,
    /// The outbound L1 head block sender.
    head_sender: UnboundedSender<BlockInfo>,
    /// The outbound L1 finalized block sender.
    finalized_sender: UnboundedSender<BlockInfo>,
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
        finalized_sender: UnboundedSender<BlockInfo>,
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
            finalized_sender,
            latest_head: head_updates,
            block_signer_sender,
            cancellation,
            inbound_queries,
        }
    }

    /// Fetches logs for the given block hash.
    async fn fetch_logs(&self, block_hash: B256) -> Result<Vec<Log>, L1WatcherRpcError<BlockInfo>> {
        let logs = self
            .l1_provider
            .get_logs(&alloy_rpc_types_eth::Filter::new().select(block_hash))
            .await?;

        Ok(logs)
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
        let mut head_stream =
            BlockStream::new(&self.l1_provider, BlockNumberOrTag::Latest, Duration::from_secs(13))
                .into_stream();
        let mut finalized_stream = BlockStream::new(
            &self.l1_provider,
            BlockNumberOrTag::Finalized,
            Duration::from_secs(60),
        )
        .into_stream();

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
                },
                new_head = head_stream.next() => match new_head {
                    None => {
                        return Err(L1WatcherRpcError::StreamEnded);
                    }
                    Some(head_block_info) => {
                        // Send the head update event to all consumers.
                        self.head_sender.send(head_block_info)?;
                        self.latest_head.send_replace(Some(head_block_info));

                        // For each log, attempt to construct a `SystemConfigLog`.
                        // Build the `SystemConfigUpdate` from the log.
                        // If the update is an Unsafe block signer update, send the address
                        // to the block signer sender.
                        let logs = self.fetch_logs(head_block_info.hash).await?;
                        let ecotone_active = self.config.is_ecotone_active(head_block_info.timestamp);
                        for log in logs {
                            if log.address() != self.config.l1_system_config_address {
                                continue; // Skip logs not related to the system config.
                            }

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
                },
                new_finalized = finalized_stream.next() => match new_finalized {
                    None => {
                        return Err(L1WatcherRpcError::StreamEnded);
                    }
                    Some(finalized_block_info) => {
                        self.finalized_sender.send(finalized_block_info)?;
                    }
                }
            }
        }
    }

    async fn process(&mut self, _msg: Self::InboundEvent) -> Result<(), Self::Error> {
        // The L1 watcher does not process any incoming messages.
        Ok(())
    }
}

/// A wrapper around a [`PollerBuilder`] that observes [`BlockId`] updates on a [`RootProvider`].
///
/// Note that this stream is not guaranteed to be contiguous. It may miss certain blocks, and
/// yielded items should only be considered to be the latest block matching the given
/// [`BlockNumberOrTag`].
struct BlockStream<'a> {
    /// The inner [`RootProvider`].
    l1_provider: &'a RootProvider,
    /// The block tag to poll for.
    tag: BlockNumberOrTag,
    /// The poll interval (in seconds).
    poll_interval: Duration,
}

impl<'a> BlockStream<'a> {
    /// Creates a new [`BlockStream`] instance.
    ///
    /// ## Panics
    /// Panics if the passed [`BlockNumberOrTag`] is of the [`BlockNumberOrTag::Number`] variant.
    fn new(l1_provider: &'a RootProvider, tag: BlockNumberOrTag, poll_interval: Duration) -> Self {
        if matches!(tag, BlockNumberOrTag::Number(_)) {
            panic!("Invalid BlockNumberOrTag variant - Must be a tag");
        }
        Self { l1_provider, tag, poll_interval }
    }

    /// Transforms the watcher into a [`Stream`].
    fn into_stream(self) -> impl Stream<Item = BlockInfo> + Unpin {
        let mut poll_stream = PollerBuilder::<_, Block>::new(
            self.l1_provider.weak_client(),
            "eth_getBlockByNumber",
            (self.tag, false),
        )
        .with_poll_interval(self.poll_interval)
        .into_stream();

        Box::pin(stream! {
            let mut last_block = None;
            while let Some(next) = poll_stream.next().await {
                let info: BlockInfo = next.into_consensus().into();

                if last_block.map(|b| b != info).unwrap_or(true) {
                    last_block = Some(info);
                    yield info;
                }
            }
        })
    }
}

/// The error type for the [`L1WatcherRpc`].
#[derive(Error, Debug)]
pub enum L1WatcherRpcError<T> {
    /// Error sending the head update event.
    #[error("Error sending the head update event: {0}")]
    SendError(#[from] SendError<T>),
    /// Error in the transport layer.
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
    /// The L1 block was not found.
    #[error("L1 block not found: {0}")]
    L1BlockNotFound(BlockId),
    /// Stream ended unexpectedly.
    #[error("Stream ended unexpectedly")]
    StreamEnded,
}
