use crate::{SupervisorError, reorg::task::ReorgTask, syncnode::ManagedNodeController};
use alloy_primitives::ChainId;
use alloy_rpc_client::RpcClient;
use derive_more::Constructor;
use futures::future;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{DbReader, StorageRewinder};
use std::{collections::HashMap, sync::Arc};
use tracing::{info, warn};

/// Handles L1 reorg operations for multiple chains
#[derive(Debug, Constructor)]
pub struct ReorgHandler<DB> {
    /// The Alloy RPC client for L1.
    rpc_client: RpcClient,
    /// Per chain dbs.
    chain_dbs: HashMap<ChainId, Arc<DB>>,
    /// Per chain managed nodes
    managed_nodes: HashMap<ChainId, Arc<dyn ManagedNodeController>>,
}

impl<DB> ReorgHandler<DB>
where
    DB: DbReader + StorageRewinder + Send + Sync + 'static,
{
    /// Processes a reorg for all chains when a new latest L1 block is received
    pub async fn handle_l1_reorg(&self, latest_block: BlockInfo) -> Result<(), SupervisorError> {
        info!(
            target: "supervisor::reorg_handler",
            l1_block_number = latest_block.number,
            "Reorg detected, processing..."
        );

        let mut handles = Vec::with_capacity(self.chain_dbs.len());

        for (chain_id, chain_db) in &self.chain_dbs {
            let managed_node = self.managed_nodes.get(chain_id).ok_or(
                SupervisorError::Initialise("no managed node found for chain".to_string()),
            )?;

            let reorg_task = ReorgTask::new(
                *chain_id,
                Arc::clone(chain_db),
                self.rpc_client.clone(),
                Arc::clone(managed_node),
            );

            let handle = tokio::spawn(async move { reorg_task.process_chain_reorg().await });
            handles.push(handle);
        }

        let results = future::join_all(handles).await;
        let failed_chains = results.into_iter().filter(|result| result.is_err()).count();

        if failed_chains > 0 {
            warn!(
                target: "supervisor::reorg_handler",
                no_of_failed_chains = %failed_chains,
                "Reorg processing completed with failed chains"
            );
        }

        Ok(())
    }
}
