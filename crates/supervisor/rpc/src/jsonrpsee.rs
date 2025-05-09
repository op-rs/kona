//! The Optimism Supervisor RPC API using `jsonrpsee`

use alloy_eips::eip1898::BlockNumHash;
use alloy_primitives::{B256, ChainId};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use kona_interop::{DerivedIdPair, ExecutingDescriptor, SafetyLevel, SuperRootResponse};

/// Optimism specified rpc interface.
///
/// https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#methods
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

    /// Gets the super root state at a specified timestamp, which represents the global state across
    /// all monitored chains.
    #[method(name = "superRootAtTimestamp")]
    async fn super_root_at_timestamp(&self, timestamp: u64) -> RpcResult<SuperRootResponse>;

    /// Verifies if an access-list references only valid messages
    #[method(name = "checkAccessList")]
    async fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> RpcResult<()>;
}
