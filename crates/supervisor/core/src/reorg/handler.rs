use super::metrics::Metrics;
use crate::{reorg::task::ReorgTask, syncnode::ManagedNodeController};
use alloy_primitives::ChainId;
use alloy_rpc_client::RpcClient;
use derive_more::Constructor;
use futures::future;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DbReader, StorageRewinder};
use std::{collections::HashMap, sync::Arc, time::Instant};
use thiserror::Error;
use tracing::{info, warn};

/// Error type for reorg handling
#[derive(Debug, Error)]
pub enum ReorgHandlerError {
    /// Indicates managed node not found for the chain.
    #[error("managed node not found for chain: {0}")]
    ManagedNodeMissing(u64),
}

/// Handles L1 reorg operations for multiple chains
#[derive(Debug, Constructor)]
pub struct ReorgHandler<C, DB> {
    /// The Alloy RPC client for L1.
    rpc_client: RpcClient,
    /// Per chain dbs.
    chain_dbs: HashMap<ChainId, Arc<DB>>,
    /// Per chain managed nodes
    managed_nodes: HashMap<ChainId, Arc<C>>,
}

impl<C, DB> ReorgHandler<C, DB>
where
    C: ManagedNodeController + Send + Sync + 'static,
    DB: DbReader + StorageRewinder + Send + Sync + 'static,
{
    /// Processes a reorg for all chains when a new latest L1 block is received
    pub async fn handle_l1_reorg(&self, latest_block: BlockInfo) -> Result<(), ReorgHandlerError> {
        info!(
            target: "supervisor::reorg_handler",
            l1_block_number = latest_block.number,
            "Reorg detected, processing..."
        );

        let mut handles = Vec::with_capacity(self.chain_dbs.len());

        for (chain_id, chain_db) in &self.chain_dbs {
            let managed_node = self
                .managed_nodes
                .get(chain_id)
                .ok_or(ReorgHandlerError::ManagedNodeMissing(*chain_id))?;

            let reorg_task = ReorgTask::new(
                *chain_id,
                Arc::clone(chain_db),
                self.rpc_client.clone(),
                managed_node.clone(),
            );

            let chain_id = *chain_id;

            let handle = tokio::spawn(async move {
                let reorg_task = reorg_task.with_metrics();
                let start_time = Instant::now();
                let result = reorg_task.process_chain_reorg().await;
                Metrics::record_l1_reorg_processing(chain_id, start_time, &result);
                result
            });
            handles.push(handle);
        }

        let results = future::join_all(handles).await;
        for result in results {
            if let Err(err) = result {
                warn!(target: "supervisor::reorg_handler", %err, "Reorg task failed");
            }
        }

        Ok(())
    }
}
