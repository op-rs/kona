use std::sync::Arc;

use alloy_eips::BlockNumberOrTag;
use alloy_provider::Provider;
use alloy_transport::{RpcError, TransportErrorKind};
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OutputRoot, Predeploys};
use tokio::sync::oneshot::Sender;

use crate::{EngineClient, EngineClientError, EngineState};

/// The type of data that can be requested from the engine.
pub type EngineQuerySender = tokio::sync::mpsc::Sender<EngineQueries>;

/// Returns the full engine state.
#[derive(Debug)]
pub enum EngineQueries {
    /// Returns the rollup config.
    Config(Sender<RollupConfig>),
    /// Returns current L2 engine state information.
    State(Sender<EngineState>),
    /// Returns the L2 output at the specified block with a tuple of the block info and associated
    /// engine state.
    OutputAtBlock {
        /// The block number or tag of the block to retrieve the output for.
        block: BlockNumberOrTag,
        /// A channel to send back the output and engine state.
        sender: Sender<(L2BlockInfo, OutputRoot, EngineState)>,
    },
    /// Returns a subscription to the updates of the engine state.
    StateReceiver(Sender<tokio::sync::watch::Receiver<EngineState>>),
}

/// An error that can occur when querying the engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineQueriesError {
    /// The output channel was closed unexpectedly. Impossible to send query response.
    #[error("Output channel closed unexpectedly. Impossible to send query response")]
    OutputChannelClosed,
    /// Failed to retrieve the L2 block by label.
    #[error("Failed to retrieve L2 block by label: {0}")]
    BlockRetrievalFailed(#[from] EngineClientError),
    /// No block withdrawals root while Isthmus is active.
    #[error("No block withdrawals root while Isthmus is active")]
    NoWithdrawalsRoot,
    /// No L2 block found for block number or tag.
    #[error("No L2 block found for block number or tag: {0}")]
    NoL2BlockFound(BlockNumberOrTag),
    /// Impossible to retrieve L2 withdrawals root from state.
    #[error("Impossible to retrieve L2 withdrawals root from state. {0}")]
    FailedToRetrieveWithdrawalsRoot(#[from] RpcError<TransportErrorKind>),
}

impl EngineQueries {
    /// Handles the engine query request.
    pub async fn handle(
        self,
        state_recv: &tokio::sync::watch::Receiver<EngineState>,
        client: &Arc<EngineClient>,
        rollup_config: &Arc<RollupConfig>,
    ) -> Result<(), EngineQueriesError> {
        let state = *state_recv.borrow();
        match self {
            Self::Config(sender) => sender
                .send((**rollup_config).clone())
                .map_err(|_| EngineQueriesError::OutputChannelClosed),
            Self::State(sender) => {
                sender.send(state).map_err(|_| EngineQueriesError::OutputChannelClosed)
            }
            Self::OutputAtBlock { block, sender } => {
                let output_block = client.l2_block_by_label(block).await?;
                let output_block = output_block.ok_or(EngineQueriesError::NoL2BlockFound(block))?;
                // Cloning the l2 block below is cheaper than sending a network request to get the
                // l2 block info. Querying the `L2BlockInfo` from the client ends up
                // fetching the full l2 block again.
                let consensus_block = output_block.clone().into_consensus();
                let output_block_info =
                    L2BlockInfo::from_block_and_genesis::<op_alloy_consensus::OpTxEnvelope>(
                        &consensus_block.map_transactions(|tx| tx.inner.inner.into_inner()),
                        &rollup_config.genesis,
                    )
                    .map_err(|_| EngineQueriesError::NoL2BlockFound(block))?;

                let state_root = output_block.header.state_root;

                let message_passer_storage_root =
                    if rollup_config.is_isthmus_active(output_block.header.timestamp) {
                        output_block
                            .header
                            .withdrawals_root
                            .ok_or(EngineQueriesError::NoWithdrawalsRoot)?
                    } else {
                        // Fetch the storage root for the L2 head block.
                        let l2_to_l1_message_passer = client
                            .get_proof(Predeploys::L2_TO_L1_MESSAGE_PASSER, Default::default())
                            .block_id(block.into())
                            .await?;

                        l2_to_l1_message_passer.storage_hash
                    };

                let output_response_v0 = OutputRoot::from_parts(
                    state_root,
                    message_passer_storage_root,
                    output_block.header.hash,
                );

                sender
                    .send((output_block_info, output_response_v0, state))
                    .map_err(|_| EngineQueriesError::OutputChannelClosed)
            }
            Self::StateReceiver(subscription) => subscription
                .send(state_recv.clone())
                .map_err(|_| EngineQueriesError::OutputChannelClosed),
        }
    }
}
