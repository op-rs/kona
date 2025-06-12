//! The Optimism Supervisor RPC API using `jsonrpsee`

pub use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    types::{ErrorCode, ErrorObjectOwned},
};

use crate::SupervisorSyncStatus;
use alloy_eips::BlockNumHash;
use alloy_primitives::{B256, BlockHash, ChainId, map::HashMap};
use jsonrpsee::proc_macros::rpc;
use kona_interop::{
    DerivedIdPair, DerivedRefPair, ExecutingDescriptor, SafetyLevel, SuperRootResponse,
};
use kona_protocol::BlockInfo;
use kona_supervisor_types::{
    BlockSeal, L2BlockRef, ManagedEvent, OutputV0, Receipts, SubscriptionEvent,
};
use serde::{Deserialize, Serialize};

/// Supervisor API for interop.
///
/// See spec <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#methods>.
// TODO:: add all the methods
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "supervisor"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "supervisor"))]
pub trait SupervisorApi {
    /// Gets the source block for a given derived block
    #[method(name = "crossDerivedToSource")]
    async fn cross_derived_to_source(
        &self,
        chain_id: ChainId,
        block_id: BlockNumHash,
    ) -> RpcResult<BlockInfo>;

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

    /// Describes superchain sync status.
    ///
    /// Spec: <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#supervisor_syncstatus>
    #[method(name = "syncStatus")]
    async fn sync_status(&self) -> RpcResult<SupervisorSyncStatus>;

    /// Returns the last derived block, aka the [`LocalSafe`] block, for each chain, from the given
    /// L1 block.
    ///
    /// Spec: <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#supervisor_allsafederivedat>
    ///
    /// [`LocalSafe`]: SafetyLevel::LocalSafe
    #[method(name = "allSafeDerivedAt")]
    async fn all_safe_derived_at(
        &self,
        derived_from: BlockNumHash,
    ) -> RpcResult<HashMap<ChainId, BlockNumHash>>;
}

/// Represents the topics for subscriptions in the Managed Mode API.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubscriptionTopic {
    /// The topic for events from the managed node.
    Events,
}

/// ManagedModeApi to send control signals to a managed node from supervisor
/// And get info for syncing the state with the given L2.
///
/// See spec <https://specs.optimism.io/interop/managed-mode.html>
/// Using the proc_macro to generate the client and server code.
/// Default namespace separator is `_`.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "interop"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "interop"))]
pub trait ManagedModeApi {
    /// Subscribe to the events from the managed node.
    /// Op-node provides the `interop-subscribe` method for subscribing to the events topic.
    /// Subscription notifications are then sent via the `interop-subscription` method as
    /// [`SubscriptionEvent`]s.
    // Currently, the `events` topic must be explicitly passed as a parameter to the subscription
    // request, even though this function is specifically intended to subscribe to the `events`
    // topic. todo: Find a way to eliminate the need to pass the topic explicitly.
    #[subscription(name = "subscribe" => "subscription", item = SubscriptionEvent, unsubscribe = "unsubscribe")]
    async fn subscribe_events(&self, topic: SubscriptionTopic) -> SubscriptionResult;

    /// Pull an event from the managed node.
    #[method(name = "pullEvent")]
    async fn pull_event(&self) -> RpcResult<ManagedEvent>;

    /// Control signals sent to the managed node from supervisor
    /// Update the cross unsafe block head
    #[method(name = "updateCrossUnsafe")]
    async fn update_cross_unsafe(&self, id: BlockNumHash) -> RpcResult<()>;

    /// Update the cross safe block head
    #[method(name = "updateCrossSafe")]
    async fn update_cross_safe(&self, derived: BlockNumHash, source: BlockNumHash)
    -> RpcResult<()>;

    /// Update the finalized block head
    #[method(name = "updateFinalized")]
    async fn update_finalized(&self, id: BlockNumHash) -> RpcResult<()>;

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
        local_unsafe: BlockNumHash,
        cross_unsafe: BlockNumHash,
        local_safe: BlockNumHash,
        cross_safe: BlockNumHash,
        finalized: BlockNumHash,
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
    async fn chain_id(&self) -> RpcResult<String>;

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
