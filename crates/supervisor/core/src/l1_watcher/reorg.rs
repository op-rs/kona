use crate::{SupervisorError, rewinder::ChainRewinder};
use alloy_eips::BlockNumHash;
use alloy_primitives::{B256, ChainId};
use alloy_rpc_client::RpcClient;
use alloy_rpc_types_eth::Block;
use derive_more::Constructor;
use kona_interop::DependencySet;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{
    ChainDb, ChainDbFactory, DerivationStorageReader, HeadRefStorageReader,
};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Handles L1 reorg operations for multiple chains
#[derive(Debug, Constructor)]
pub struct ReorgHandler {
    /// The Alloy RPC client for L1.
    rpc_client: RpcClient,
    /// The superchain dependency set.
    dependency_set: Arc<DependencySet>,
    /// The database factory.
    db_factory: Arc<ChainDbFactory>,
}

impl ReorgHandler {
    /// Processes a reorg for all chains when a new latest L1 block is received
    pub async fn handle_l1_reorg(&self, latest_block: BlockInfo) -> Result<(), SupervisorError> {
        info!(
            target: "reorg_handler",
            l1_block_number = latest_block.number,
            "Reorg detected, processing..."
        );

        let mut failed_chains = Vec::new();

        for chain_id in self.dependency_set.dependencies.keys() {
            if let Err(err) = self.process_chain_reorg(chain_id, latest_block).await {
                error!(
                    target: "reorg_handler",
                    chain_id = %chain_id,
                    %err,
                    "Failed to process reorg for chain"
                );
                failed_chains.push(chain_id);
                // Continue processing other chains even if one fails
            }
        }

        if !failed_chains.is_empty() {
            warn!(
                target: "reorg_handler",
                no_of_failed_chains = %failed_chains.len(),
                "Reorg processing completed with failed chains"
            );
        }

        Ok(())
    }

    /// Processes reorg for a single chain
    async fn process_chain_reorg(
        &self,
        chain_id: &u64,
        _latest_block: BlockInfo,
    ) -> Result<(), SupervisorError> {
        let chain_db = self.db_factory.get_db(*chain_id)?;

        // Find last valid source block for this chain
        let rewind_target_source =
            match self.find_rewind_target(*chain_id, chain_db.clone()).await? {
                Some(source) => source,
                None => {
                    // No need to re-org for this chain
                    return Ok(());
                }
            };

        // Get the derived block at the target source block
        let rewind_target_derived =
            chain_db.latest_derived_block_at_source(rewind_target_source)?;

        // Call the rewinder to handle the DB rewinding
        let rewinder = ChainRewinder::new(*chain_id, chain_db);
        rewinder.handle_l1_reorg(rewind_target_derived.id())?;

        Ok(())
    }

    /// Finds the rewind target for a chain during a reorg
    async fn find_rewind_target(
        &self,
        chain_id: ChainId,
        db: Arc<ChainDb>,
    ) -> Result<Option<BlockNumHash>, SupervisorError> {
        info!(
            target: "reorg_handler",
            chain_id = %chain_id,
            "Finding rewind target..."
        );

        let latest_state = db.latest_derivation_state()?;

        // Check if the latest source block is still canonical
        if self.is_block_canonical(latest_state.source.number, latest_state.source.hash).await? {
            info!(
                target: "reorg_handler",
                chain_id = %chain_id,
                block_number = latest_state.source.number,
                "Latest source block is still canonical, no reorg needed"
            );
            return Ok(None);
        }

        // Get finalized block and derive common ancestor
        let finalized_block = db.get_safety_head_ref(kona_interop::SafetyLevel::Finalized)?;
        let mut common_ancestor = db.derived_to_source(finalized_block.id())?.id();
        let mut current_source = latest_state.source.id();

        while current_source.number >= common_ancestor.number {
            // If the current source block is canonical, we found the rewind target
            if self.is_block_canonical(current_source.number, current_source.hash).await? {
                info!(
                    target: "reorg_handler",
                    chain_id = %chain_id,
                    block_number = current_source.number,
                    "Found canonical block as rewind target"
                );
                common_ancestor = current_source;
                break;
            }

            // Otherwise, walk back to the previous source block
            current_source = db.get_source_block(current_source.number - 1)?.id();
        }

        Ok(Some(common_ancestor))
    }

    /// Checks if a block is canonical on L1
    async fn is_block_canonical(
        &self,
        block_number: u64,
        expected_hash: B256,
    ) -> Result<bool, SupervisorError> {
        match self
            .rpc_client
            .request::<_, Block>("eth_getBlockByNumber", (block_number, false))
            .await
        {
            Ok(canonical_l1) => Ok(canonical_l1.hash() == expected_hash),
            Err(err) => {
                warn!(
                    target: "reorg_handler",
                    block_number,
                    %err,
                    "Failed to fetch canonical L1 block"
                );
                Ok(false)
            }
        }
    }
}
