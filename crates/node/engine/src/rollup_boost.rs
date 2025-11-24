//! Rollup-boost abstraction used by the engine client.

use alloy_eips::BlockNumberOrTag;
use alloy_json_rpc::{ErrorPayload, RpcError};
use alloy_rpc_types_engine::{ForkchoiceState, ForkchoiceUpdated, PayloadId, PayloadStatus};
use alloy_rpc_types_eth::Block;
use alloy_transport::TransportErrorKind;
use futures::FutureExt;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use rollup_boost::{
    EngineApiExt, ExecutionMode, NewPayload, OpExecutionPayloadEnvelope, PayloadVersion,
    RpcClientError,
};
use std::{fmt::Debug, sync::Arc};

use rollup_boost::BlockSelectionPolicy;
use url::Url;

/// Configuration for the rollup-boost server.
#[derive(Clone, Debug)]
pub struct RollupBoostServerArgs {
    /// The initial execution mode of the rollup-boost server.
    pub initial_execution_mode: ExecutionMode,
    /// The block selection policy of the rollup-boost server.
    pub block_selection_policy: Option<BlockSelectionPolicy>,
    /// Whether to use the l2 client for computing state root.
    pub external_state_root: bool,
    /// Allow all engine API calls to builder even when marked as unhealthy
    /// This is default true assuming no builder CL set up
    pub ignore_unhealthy_builders: bool,
    /// Flashblocks configuration
    pub flashblocks: Option<FlashblocksClientArgs>,
}

/// Configuration for the Flashblocks client.
#[derive(Clone, Debug)]
pub struct FlashblocksClientArgs {
    /// Flashblocks Builder WebSocket URL
    pub flashblocks_builder_url: Url,

    /// Flashblocks WebSocket host for outbound connections
    pub flashblocks_host: String,

    /// Flashblocks WebSocket port for outbound connections
    pub flashblocks_port: u16,

    /// Websocket connection configuration
    pub flashblocks_ws_config: FlashblocksWebsocketConfig,
}

/// Configuration for the Flashblocks WebSocket connection.
#[derive(Debug, Clone, Copy)]
pub struct FlashblocksWebsocketConfig {
    /// Minimum time for exponential backoff for timeout if builder disconnected
    pub flashblock_builder_ws_initial_reconnect_ms: u64,

    /// Maximum time for exponential backoff for timeout if builder disconnected
    pub flashblock_builder_ws_max_reconnect_ms: u64,

    /// Interval in milliseconds between ping messages sent to upstream servers to detect
    /// unresponsive connections
    pub flashblock_builder_ws_ping_interval_ms: u64,

    /// Timeout in milliseconds to wait for pong responses from upstream servers before considering
    /// the connection dead
    pub flashblock_builder_ws_pong_timeout_ms: u64,
}

/// An error that occurred in the rollup-boost server.
#[derive(Debug, thiserror::Error)]
pub enum RollupBoostServerError {
    /// JSON-RPC error.
    #[error("Rollup boost server error: {0}")]
    Jsonrpsee(#[from] jsonrpsee_types::ErrorObjectOwned),
}

impl From<RollupBoostServerError> for RpcError<TransportErrorKind> {
    fn from(error: RollupBoostServerError) -> Self {
        match error {
            RollupBoostServerError::Jsonrpsee(error) => Self::ErrorResp(ErrorPayload {
                code: error.code().into(),
                message: error.message().to_string().into(),
                data: None,
            }),
        }
    }
}

#[derive(Debug)]
pub enum RollupBoostBackend {
    Flashblocks(Arc<rollup_boost::FlashblocksService>),
    Rpc(Arc<rollup_boost::RpcClient>),
}

pub(crate) type ClientResult<T> = Result<T, RpcClientError>;

impl EngineApiExt for RollupBoostBackend {
    #[allow(elided_named_lifetimes, clippy::type_complexity, clippy::type_repetition_in_bounds)]
    fn new_payload<'life0, 'async_trait>(
        &'life0 self,
        new_payload: NewPayload,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ClientResult<PayloadStatus>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        match self {
            RollupBoostBackend::Flashblocks(flashblocks) => {
                flashblocks.new_payload(new_payload).boxed()
            }
            RollupBoostBackend::Rpc(rpc) => rpc.new_payload(new_payload).boxed(),
        }
    }

    #[allow(elided_named_lifetimes, clippy::type_complexity, clippy::type_repetition_in_bounds)]
    fn get_payload<'life0, 'async_trait>(
        &'life0 self,
        payload_id: PayloadId,
        version: PayloadVersion,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ClientResult<OpExecutionPayloadEnvelope>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        match self {
            RollupBoostBackend::Flashblocks(flashblocks) => {
                flashblocks.get_payload(payload_id, version).boxed()
            }
            RollupBoostBackend::Rpc(rpc) => rpc.get_payload(payload_id, version).boxed(),
        }
    }

    #[allow(elided_named_lifetimes, clippy::type_complexity, clippy::type_repetition_in_bounds)]
    fn get_block_by_number<'life0, 'async_trait>(
        &'life0 self,
        number: BlockNumberOrTag,
        full: bool,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ClientResult<Block>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        match self {
            RollupBoostBackend::Flashblocks(flashblocks) => {
                flashblocks.get_block_by_number(number, full).boxed()
            }
            RollupBoostBackend::Rpc(rpc) => rpc.get_block_by_number(number, full).boxed(),
        }
    }

    #[allow(elided_named_lifetimes, clippy::type_complexity, clippy::type_repetition_in_bounds)]
    fn fork_choice_updated_v3<'life0, 'async_trait>(
        &'life0 self,
        fork_choice_state: ForkchoiceState,
        payload_attributes: Option<OpPayloadAttributes>,
    ) -> ::core::pin::Pin<
        Box<
            dyn ::core::future::Future<Output = ClientResult<ForkchoiceUpdated>>
                + ::core::marker::Send
                + 'async_trait,
        >,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        match self {
            RollupBoostBackend::Flashblocks(flashblocks) => {
                flashblocks.fork_choice_updated_v3(fork_choice_state, payload_attributes).boxed()
            }
            RollupBoostBackend::Rpc(rpc) => {
                rpc.fork_choice_updated_v3(fork_choice_state, payload_attributes).boxed()
            }
        }
    }
}
