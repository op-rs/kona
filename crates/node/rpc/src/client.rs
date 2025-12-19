use alloy_eips::BlockNumberOrTag;
use alloy_primitives::B256;
use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use kona_engine::EngineState;
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OutputRoot};
use rollup_boost::{GetExecutionModeResponse, SetExecutionModeRequest, SetExecutionModeResponse};
use std::fmt::Debug;
use thiserror::Error;
use tokio::sync::watch;

/// Client trait wrapping RPC implementation for the EngineActor.
#[async_trait]
pub trait EngineRpcClient: Debug + Send + Sync + Clone {
    /// Request the current rollup configuration.
    async fn get_config(&self) -> RpcResult<RollupConfig>;
    /// Request the current [`EngineState`] snapshot.
    async fn get_state(&self) -> RpcResult<EngineState>;
    /// Development API: Get the current number of pending tasks in the queue.
    async fn get_task_queue_length(&self) -> RpcResult<usize>;
    /// Request the L2 output root for a specific block.
    ///
    /// Returns a tuple of block info, output root, and engine state at the requested block.
    async fn output_at_block(
        &self,
        block: BlockNumberOrTag,
    ) -> RpcResult<(L2BlockInfo, OutputRoot, EngineState)>;
    /// Subscribes to engine queue length updates managed by the returned [`watch::Receiver`].
    async fn dev_subscribe_to_engine_queue_length(&self) -> RpcResult<watch::Receiver<usize>>;
    /// Subscribes to engine state updates managed by the returned [`watch::Receiver`].
    async fn dev_subscribe_to_engine_state(&self) -> RpcResult<watch::Receiver<EngineState>>;
}

/// Client trait wrapping RPC implementation for the rollup boost admin endpoints.
pub trait RollupBoostAdminClient: Send + Sync + Debug {
    /// Sets the execution mode for the rollup boost server.
    fn set_execution_mode(
        &self,
        request: SetExecutionModeRequest,
    ) -> impl Future<Output = RpcResult<SetExecutionModeResponse>> + Send;

    /// Gets the current execution mode from the rollup boost server.
    fn get_execution_mode(
        &self,
    ) -> impl Future<Output = RpcResult<GetExecutionModeResponse>> + Send;
}

/// Client trait wrapping RPC implementation for the Sequencer admin endpoints.
#[async_trait]
pub trait SequencerAdminAPIClient: Send + Sync + Debug {
    /// Check if the sequencer is active.
    async fn is_sequencer_active(&self) -> Result<bool, SequencerAdminAPIError>;

    /// Check if the conductor is enabled.
    async fn is_conductor_enabled(&self) -> Result<bool, SequencerAdminAPIError>;

    /// Check if in recovery mode.
    async fn is_recovery_mode(&self) -> Result<bool, SequencerAdminAPIError>;

    /// Start the sequencer.
    async fn start_sequencer(&self) -> Result<(), SequencerAdminAPIError>;

    /// Stop the sequencer.
    async fn stop_sequencer(&self) -> Result<B256, SequencerAdminAPIError>;

    /// Set recovery mode.
    async fn set_recovery_mode(&self, mode: bool) -> Result<(), SequencerAdminAPIError>;

    /// Override the leader.
    async fn override_leader(&self) -> Result<(), SequencerAdminAPIError>;

    /// Reset the derivation pipeline.
    async fn reset_derivation_pipeline(&self) -> Result<(), SequencerAdminAPIError>;
}

/// Errors that can occur when using the sequencer admin API.
#[derive(Debug, Error)]
pub enum SequencerAdminAPIError {
    /// Error sending request.
    #[error("Error sending request: {0}.")]
    RequestError(String),

    /// Error receiving response.
    #[error("Error receiving response: {0}.")]
    ResponseError(String),

    /// Error stopping sequencer.
    #[error("Error stopping sequencer: {0}.")]
    StopError(#[from] StopSequencerError),

    /// Error overriding leader.
    #[error("Error overriding leader: {0}.")]
    LeaderOverrideError(String),
}

/// Errors that can occur when using the sequencer admin API.
#[derive(Debug, Error)]
pub enum StopSequencerError {
    /// Sequencer stopped successfully, followed by some error.
    #[error("Sequencer stopped successfully, followed by error: {0}.")]
    ErrorAfterSequencerWasStopped(String),
}
