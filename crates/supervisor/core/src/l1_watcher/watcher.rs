use crate::event::ChainEvent;
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::ChainId;
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_eth::{Block, Header};
use futures::StreamExt;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::FinalizedL1Storage;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// A watcher that polls the L1 chain for finalized blocks.
#[derive(Debug)]
pub struct L1Watcher<F> {
    /// The Alloy RPC client for L1.
    rpc_client: RpcClient,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
    finalized_l1_storage: Arc<F>,

    event_txs: HashMap<ChainId, mpsc::Sender<ChainEvent>>,
}

impl<F> L1Watcher<F>
where
    F: FinalizedL1Storage + 'static,
{
    /// Creates a new [`L1Watcher`] instance.
    pub const fn new(
        rpc_client: RpcClient,
        finalized_l1_storage: Arc<F>,
        event_txs: HashMap<ChainId, mpsc::Sender<ChainEvent>>,
        cancellation: CancellationToken,
    ) -> Self {
        Self { rpc_client, finalized_l1_storage, event_txs, cancellation }
    }

    /// Starts polling for finalized blocks and processes them.
    pub async fn run(&self) {
        let finalized_head_poller = self
            .rpc_client
            .prepare_static_poller::<_, Block>(
                "eth_getBlockByNumber",
                (BlockNumberOrTag::Finalized, false),
            )
            .with_poll_interval(Duration::from_secs(5));
        let mut finalized_head_stream = finalized_head_poller.into_stream();

        // Track the last seen finalized block number
        let mut last_finalized_number = 0;

        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    info!(target: "l1_watcher", "L1Watcher cancellation requested, stopping...");
                    break;
                }
                finalized_block = finalized_head_stream.next() => {
                    if let Some(block) = finalized_block {
                        let block_number = block.header.number;
                        if block_number != last_finalized_number {
                            let Header { hash, inner: alloy_consensus::Header { number, parent_hash, timestamp, .. }, .. } = block.header;
                            let finalized_source_block = BlockInfo::new(hash, number, parent_hash, timestamp);
                            info!(target: "l1_watcher", block_number = finalized_source_block.number, "New finalized L1 block received");
                            if let Err(err) = self.finalized_l1_storage.update_finalized_l1(finalized_source_block) {
                                error!(target: "l1_watcher", %err, "Failed to update finalized L1 block");
                                continue
                            }

                            for (chain_id, sender) in &self.event_txs {
                                if let Err(err) = sender
                                    .send(ChainEvent::FinalizedSourceUpdate { finalized_source_block })
                                    .await {
                                    error!(
                                        target: "l1_watcher",
                                        chain_id = %chain_id,
                                        %err, "Failed to send finalized L1 update event",
                                    );
                                }
                            }
                            last_finalized_number = block_number;

                        }
                    }
                }
            }
        }
    }
}
