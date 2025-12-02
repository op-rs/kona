//! A task for finalizing an L2 block.

use crate::{
    EngineClient, EngineState, EngineTaskExt, FinalizeTaskError, SynchronizeTask,
    state::EngineSyncStateUpdate,
};
use alloy_network::Ethereum;
use alloy_provider::Provider;
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::L2BlockInfo;
use op_alloy_network::Optimism;
use std::{marker::PhantomData, sync::Arc, time::Instant};

/// The [`FinalizeTask`] fetches the [`L2BlockInfo`] at `block_number`, updates the [`EngineState`],
/// and dispatches a forkchoice update to finalize the block.
#[derive(Debug, Clone)]
pub struct FinalizeTask<L1Provider, L2Provider, EngineClient_>
where
    L1Provider: Provider<Ethereum>,
    L2Provider: Provider<Optimism>,
    EngineClient_: EngineClient<L1Provider, L2Provider>,
{
    /// The engine client.
    pub client: Arc<EngineClient_>,
    /// The rollup config.
    pub cfg: Arc<RollupConfig>,
    /// The number of the L2 block to finalize.
    pub block_number: u64,

    phantom_l1_provider: PhantomData<L1Provider>,
    phantom_l2_provider: PhantomData<L2Provider>,
}

impl<L1Provider, L2Provider, EngineClient_> FinalizeTask<L1Provider, L2Provider, EngineClient_>
where
    L1Provider: Provider<Ethereum>,
    L2Provider: Provider<Optimism>,
    EngineClient_: EngineClient<L1Provider, L2Provider>,
{
    /// Creates a new [`SynchronizeTask`].
    pub const fn new(
        client: Arc<EngineClient_>,
        cfg: Arc<RollupConfig>,
        block_number: u64,
    ) -> Self {
        Self {
            client,
            cfg,
            block_number,
            phantom_l1_provider: PhantomData,
            phantom_l2_provider: PhantomData,
        }
    }
}

#[async_trait]
impl<L1Provider, L2Provider, EngineClient_> EngineTaskExt
    for FinalizeTask<L1Provider, L2Provider, EngineClient_>
where
    L1Provider: Provider<Ethereum>,
    L2Provider: Provider<Optimism>,
    EngineClient_: EngineClient<L1Provider, L2Provider>,
{
    type Output = ();

    type Error = FinalizeTaskError;

    async fn execute(&self, state: &mut EngineState) -> Result<(), FinalizeTaskError> {
        // Sanity check that the block that is being finalized is at least safe.
        if state.sync_state.safe_head().block_info.number < self.block_number {
            return Err(FinalizeTaskError::BlockNotSafe);
        }

        let block_fetch_start = Instant::now();
        let block = self
            .client
            .l2_engine()
            .get_block(self.block_number.into())
            .full()
            .await
            .map_err(FinalizeTaskError::TransportError)?
            .ok_or(FinalizeTaskError::BlockNotFound(self.block_number))?
            .into_consensus();
        let block_info = L2BlockInfo::from_block_and_genesis(&block, &self.client.cfg().genesis)
            .map_err(FinalizeTaskError::FromBlock)?;
        let block_fetch_duration = block_fetch_start.elapsed();

        // Dispatch a forkchoice update.
        let fcu_start = Instant::now();
        SynchronizeTask::new(
            self.client.clone(),
            self.cfg.clone(),
            EngineSyncStateUpdate { finalized_head: Some(block_info), ..Default::default() },
        )
        .execute(state)
        .await?;
        let fcu_duration = fcu_start.elapsed();

        info!(
            target: "engine",
            hash = %block_info.block_info.hash,
            number = block_info.block_info.number,
            ?block_fetch_duration,
            ?fcu_duration,
            "Updated finalized head"
        );

        Ok(())
    }
}
