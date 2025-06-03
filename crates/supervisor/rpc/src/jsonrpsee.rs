//! The Optimism Supervisor RPC API using `jsonrpsee`

pub use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    types::{ErrorCode, ErrorObjectOwned},
};

use alloy_eips::{BlockId, BlockNumHash};
use alloy_primitives::{B256, BlockHash, ChainId};
use jsonrpsee::proc_macros::rpc;
use kona_interop::{
    DerivedIdPair, DerivedRefPair, ExecutingDescriptor, SafetyLevel, SuperRootResponse,
};
use kona_protocol::BlockInfo;
use kona_supervisor_types::{BlockSeal, L2BlockRef, ManagedEvent, OutputV0, Receipts};

/// Supervisor API for interop.
///
/// See spec <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#methods>.
// TODO:: add all the methods
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "supervisor"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "supervisor"))]
pub trait SupervisorApi {
    /// Gets the localUnsafe BlockId
    #[method(name = "localUnsafe")]
    async fn local_unsafe(&self, chain_id: ChainId) -> RpcResult<BlockNumHash>;

    /// Gets the crossSafe DerivedIdPair
    #[method(name = "crossSafe")]
    async fn cross_safe(&self, chain_id: ChainId) -> RpcResult<DerivedIdPair>;

    /// Gets the finalized BlockId
    #[method(name = "finalized")]
    async fn finalized(&self, chain_id: ChainId) -> RpcResult<BlockNumHash>;

    /// Gets the super root state at a specified timestamp, which represents the global state
    /// across all monitored chains.
    #[method(name = "superRootAtTimestamp")]
    async fn super_root_at_timestamp(&self, timestamp: u64) -> RpcResult<SuperRootResponse>;

    /// Verifies if an access-list references only valid messages w.r.t. locally configured minimum
    /// [`SafetyLevel`].
    #[method(name = "checkAccessList")]
    async fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> RpcResult<()>;
}

/// ManagedModeApi to send control signals to a managed node from supervisor
/// And get info for syncing the state with the given L2.
///
/// See spec <https://specs.optimism.io/interop/managed-mode.html>
/// Using the proc_macro to generate the client and server code.
/// Default namespace separator is `_`.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "supervisor"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "supervisor"))]
pub trait ManagedModeApi {
    /// Subscribe to the events from the managed node.
    #[subscription(name = "events", item = Option<ManagedEvent>, unsubscribe = "unsubscribeEvents")]
    async fn subscribe_events(&self) -> SubscriptionResult;

    /// Pull an event from the managed node.
    #[method(name = "pullEvent")]
    async fn pull_event(&self) -> RpcResult<ManagedEvent>;

    /// Control signals sent to the managed node from supervisor
    /// Update the cross unsafe block head
    #[method(name = "updateCrossUnsafe")]
    async fn update_cross_unsafe(&self, id: BlockId) -> RpcResult<()>;

    /// Update the cross safe block head
    #[method(name = "updateCrossSafe")]
    async fn update_cross_safe(&self, derived: BlockId, source: BlockId) -> RpcResult<()>;

    /// Update the finalized block head
    #[method(name = "updateFinalized")]
    async fn update_finalized(&self, id: BlockId) -> RpcResult<()>;

    /// Invalidate a block
    #[method(name = "invalidateBlock")]
    async fn invalidate_block(&self, seal: BlockSeal) -> RpcResult<()>;

    /// Send the next L1 block
    #[method(name = "provideL1")]
    async fn provide_l1(&self, next_l1: BlockInfo) -> RpcResult<()>;

    /// Get the genesis block ref for l1 and l2; Soon to be deprecated!
    #[method(name = "anchorPoint")]
    async fn anchor_point(&self) -> RpcResult<DerivedRefPair>;

    /// Reset the managed node to the specified block heads
    #[method(name = "reset")]
    async fn reset(
        &self,
        local_unsafe: BlockId,
        cross_unsafe: BlockId,
        local_safe: BlockId,
        cross_safe: BlockId,
        finalized: BlockId,
    ) -> RpcResult<()>;

    /// Sync methods that supervisor uses to sync with the managed node
    /// Fetch all receipts for a give block
    #[method(name = "fetchReceipts")]
    async fn fetch_receipts(&self, block_hash: BlockHash) -> RpcResult<Receipts>;

    /// Get block infor for a given block number
    #[method(name = "blockRefByNumber")]
    async fn block_ref_by_number(&self, number: u64) -> RpcResult<BlockInfo>;

    /// Get the chain id
    #[method(name = "chainID")]
    async fn chain_id(&self) -> RpcResult<ChainId>;

    /// Get the state_root, message_parser_storage_root, and block_hash at a given timestamp
    #[method(name = "outputV0AtTimestamp")]
    async fn output_v0_at_timestamp(&self, timestamp: u64) -> RpcResult<OutputV0>;

    /// Get the pending state_root, message_parser_storage_root, and block_hash at a given timestamp
    #[method(name = "pendingOutputV0AtTimestamp")]
    async fn pending_output_v0_at_timestamp(&self, timestamp: u64) -> RpcResult<OutputV0>;

    /// Get the l2 block ref for a given timestamp
    #[method(name = "l2BlockRefByTimestamp")]
    async fn l2_block_ref_by_timestamp(&self, timestamp: u64) -> RpcResult<L2BlockRef>;
}
