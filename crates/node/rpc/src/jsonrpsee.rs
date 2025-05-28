//! The Optimism RPC API using `jsonrpsee`

use crate::{OutputResponse, SafeHeadResponse};
use alloy_eips::BlockNumberOrTag;
use alloy_primitives::B256;
use core::net::IpAddr;
use jsonrpsee::{
    core::{RpcResult, SubscriptionResult},
    proc_macros::rpc,
};
use kona_genesis::RollupConfig;
use kona_interop::ExecutingDescriptor;
use kona_p2p::{PeerCount, PeerDump, PeerInfo, PeerStats};
use kona_protocol::SyncStatus;
use op_alloy_consensus::interop::SafetyLevel;

#[cfg_attr(all(target_arch = "wasm32", target_os = "unknown"), allow(unused_imports))]
use getrandom as _; // required for compiling wasm32-unknown-unknown

// Re-export apis defined in upstream `op-alloy-rpc-jsonrpsee`
pub use op_alloy_rpc_jsonrpsee::traits::{MinerApiExtServer, OpAdminApiServer};

/// Optimism specified rpc interface.
///
/// https://docs.optimism.io/builders/node-operators/json-rpc
/// https://github.com/ethereum-optimism/optimism/blob/8dd17a7b114a7c25505cd2e15ce4e3d0f7e3f7c1/op-node/node/api.go#L114
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "optimism"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "optimism"))]
pub trait RollupNodeApi {
    /// Get the output root at a specific block.
    #[method(name = "outputAtBlock")]
    async fn op_output_at_block(&self, block_number: BlockNumberOrTag)
    -> RpcResult<OutputResponse>;

    /// Gets the safe head at an L1 block height.
    #[method(name = "safeHeadAtL1Block")]
    async fn op_safe_head_at_l1_block(
        &self,
        block_number: BlockNumberOrTag,
    ) -> RpcResult<SafeHeadResponse>;

    /// Get the synchronization status.
    #[method(name = "syncStatus")]
    async fn op_sync_status(&self) -> RpcResult<SyncStatus>;

    /// Get the rollup configuration parameters.
    #[method(name = "rollupConfig")]
    async fn op_rollup_config(&self) -> RpcResult<RollupConfig>;

    /// Get the software version.
    #[method(name = "version")]
    async fn op_version(&self) -> RpcResult<String>;
}

/// The opp2p namespace handles peer interactions.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "opp2p"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "opp2p"))]
pub trait OpP2PApi {
    /// Returns information of node
    #[method(name = "self")]
    async fn opp2p_self(&self) -> RpcResult<PeerInfo>;

    /// Returns information of peers
    #[method(name = "peerCount")]
    async fn opp2p_peer_count(&self) -> RpcResult<PeerCount>;

    /// Returns information of peers. If `connected` is true, only returns connected peers.
    #[method(name = "peers")]
    async fn opp2p_peers(&self, connected: bool) -> RpcResult<PeerDump>;

    /// Returns statistics of peers
    #[method(name = "peerStats")]
    async fn opp2p_peer_stats(&self) -> RpcResult<PeerStats>;

    /// Returns the discovery table
    #[method(name = "discoveryTable")]
    async fn opp2p_discovery_table(&self) -> RpcResult<Vec<String>>;

    /// Blocks the given peer
    #[method(name = "blockPeer")]
    async fn opp2p_block_peer(&self, peer: String) -> RpcResult<()>;

    /// Lists blocked peers
    #[method(name = "listBlockedPeers")]
    async fn opp2p_list_blocked_peers(&self) -> RpcResult<Vec<String>>;

    /// Blocks the given address
    #[method(name = "blocAddr")]
    async fn opp2p_block_addr(&self, ip: IpAddr) -> RpcResult<()>;

    /// Unblocks the given address
    #[method(name = "unblockAddr")]
    async fn opp2p_unblock_addr(&self, ip: IpAddr) -> RpcResult<()>;

    /// Lists blocked addresses
    #[method(name = "listBlockedAddrs")]
    async fn opp2p_list_blocked_addrs(&self) -> RpcResult<Vec<IpAddr>>;

    // TODO: should be IPNet?
    /// Blocks the given subnet
    #[method(name = "blockSubnet")]
    async fn opp2p_block_subnet(&self, subnet: String) -> RpcResult<()>;

    // TODO: should be IPNet?
    /// Unblocks the given subnet
    #[method(name = "unblockSubnet")]
    async fn opp2p_unblock_subnet(&self, subnet: String) -> RpcResult<()>;

    // TODO: should be IPNet?
    /// Lists blocked subnets
    #[method(name = "listBlockedSubnets")]
    async fn opp2p_list_blocked_subnets(&self) -> RpcResult<Vec<String>>;

    /// Protects the given peer
    #[method(name = "protectPeer")]
    async fn opp2p_protect_peer(&self, peer: String) -> RpcResult<()>;

    /// Unprotects the given peer
    #[method(name = "unprotectPeer")]
    async fn opp2p_unprotect_peer(&self, peer: String) -> RpcResult<()>;

    /// Connects to the given peer
    #[method(name = "connectPeer")]
    async fn opp2p_connect_peer(&self, peer: String) -> RpcResult<()>;

    /// Disconnects from the given peer
    #[method(name = "disconnectPeer")]
    async fn opp2p_disconnect_peer(&self, peer: String) -> RpcResult<()>;
}

/// Websockets API for the node.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "ws"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "ws"))]
#[async_trait]
pub trait Ws {
    /// Subscribes to the stream of finalized head updates.
    #[subscription(name = "subscribe_finalized_head", item = kona_protocol::L2BlockInfo)]
    async fn ws_finalized_head_updates(&self) -> SubscriptionResult;

    /// Subscribes to the stream of safe head updates.
    #[subscription(name = "subscribe_safe_head", item = kona_protocol::L2BlockInfo)]
    async fn ws_safe_head_updates(&self) -> SubscriptionResult;

    /// Subscribes to the stream of unsafe head updates.
    #[subscription(name = "subscribe_unsafe_head", item = kona_protocol::L2BlockInfo)]
    async fn ws_unsafe_head_updates(&self) -> SubscriptionResult;
}

/// Supervisor API for interop.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "supervisor"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "supervisor"))]
pub trait SupervisorApi {
    /// Checks if the given inbox entries meet the given minimum safety level.
    #[method(name = "checkAccessList")]
    async fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> RpcResult<()>;
}
