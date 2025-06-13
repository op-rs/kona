//! Server-side implementation of the Supervisor RPC API.

use crate::{SupervisorError, SupervisorService};
use alloy_eips::eip1898::BlockNumHash;
use alloy_primitives::{B256, ChainId, map::HashMap};
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorObject, error::ErrorCode},
};
use kona_interop::{DerivedIdPair, ExecutingDescriptor, SafetyLevel, SuperRootResponse};
use kona_protocol::BlockInfo;
use kona_supervisor_rpc::{SupervisorApiServer, SupervisorChainSyncStatus, SupervisorSyncStatus};
use kona_supervisor_types::SuperHead;
use std::sync::Arc;
use tracing::{trace, warn};

/// The server-side implementation struct for the [`SupervisorApiServer`].
/// It holds a reference to the core Supervisor logic.
#[derive(Debug)]
pub struct SupervisorRpc<T> {
    /// Reference to the core Supervisor logic.
    /// Using Arc allows sharing the Supervisor instance if needed,
    supervisor: Arc<T>,
}

impl<T> SupervisorRpc<T> {
    /// Creates a new [`SupervisorRpc`] instance.
    pub fn new(supervisor: Arc<T>) -> Self {
        super::Metrics::init();
        trace!(target: "supervisor_rpc", "Creating new SupervisorRpc handler");
        Self { supervisor }
    }
}

#[async_trait]
impl<T> SupervisorApiServer for SupervisorRpc<T>
where
    T: SupervisorService + 'static,
{
    async fn cross_derived_to_source(
        &self,
        chain_id: ChainId,
        derived: BlockNumHash,
    ) -> RpcResult<BlockInfo> {
        trace!(
            target: "supervisor_rpc",
            %chain_id,
            ?derived,
            "Received cross_derived_to_source request"
        );

        let source_block =
            self.supervisor.derived_to_source_block(chain_id, derived).map_err(|err| {
                warn!(
                    target: "supervisor_rpc",
                    %chain_id,
                    ?derived,
                    %err,
                    "Failed to get source block for derived block"
                );
                ErrorObject::from(err)
            })?;

        Ok(source_block)
    }

    async fn local_unsafe(&self, chain_id: ChainId) -> RpcResult<BlockNumHash> {
        trace!(target: "supervisor_rpc",
            %chain_id,
            "Received local_unsafe request"
        );

        Ok(self.supervisor.local_unsafe(chain_id)?.id())
    }

    async fn cross_safe(&self, chain_id: ChainId) -> RpcResult<DerivedIdPair> {
        trace!(target: "supervisor_rpc",
            %chain_id,
            "Received cross_safe request"
        );

        let derived = self.supervisor.cross_safe(chain_id)?.id();
        let source = self.supervisor.derived_to_source_block(chain_id, derived)?.id();

        Ok(DerivedIdPair { source, derived })
    }

    async fn finalized(&self, chain_id: ChainId) -> RpcResult<BlockNumHash> {
        trace!(target: "supervisor_rpc",
            %chain_id,
            "Received finalized request"
        );
        // self.supervisor.finalized()
        // .await
        // .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
        warn!(target: "supervisor_rpc", "finalized method not yet implemented");
        Err(ErrorObject::from(ErrorCode::InternalError))
    }

    async fn super_root_at_timestamp(&self, timestamp: u64) -> RpcResult<SuperRootResponse> {
        trace!(target: "supervisor_rpc",
            %timestamp,
            "Received super_root_at_timestamp request"
        );
        // self.supervisor.super_root_at_timestamp()
        // .await
        // .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
        warn!(target: "supervisor_rpc", "super_root_at_timestamp method not yet implemented");
        Err(ErrorObject::from(ErrorCode::InternalError))
    }

    async fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> RpcResult<()> {
        // TODO:: refcator, maybe build proc macro to record metrics
        crate::observe_rpc_call!("check_access_list", async {
            trace!(target: "supervisor_rpc", 
                num_inbox_entries = inbox_entries.len(),
                ?min_safety,
                ?executing_descriptor,
                "Received check_access_list request",
            );
            self.supervisor
                .check_access_list(inbox_entries, min_safety, executing_descriptor)
                .await
                .map_err(|e| {
                    warn!(target: "supervisor_rpc", "Error from core supervisor check_access_list: {:?}", e);
                    ErrorObject::from(ErrorCode::InternalError)
                })
        }.await)
    }

    async fn sync_status(&self) -> RpcResult<SupervisorSyncStatus> {
        trace!(target: "supervisor_rpc", "Received sync_status request");

        let mut chains = self
            .supervisor
            .chain_ids()
            .map(|id| (id, Default::default()))
            .collect::<HashMap<_, SupervisorChainSyncStatus>>();

        if chains.is_empty() {
            // return error if no chains configured
            //
            // <https://github.com/ethereum-optimism/optimism/blob/fac40575a8bcefd325c50a52e12b0e93254ac3f8/op-supervisor/supervisor/backend/status/status.go#L100-L104>
            //
            // todo: add to spec
            Err(SupervisorError::EmptyDependencySet)?;
        }

        let mut min_synced_l1 = BlockInfo { number: u64::MAX, ..Default::default() };
        let mut cross_safe_timestamp = u64::MAX;
        let mut finalized_timestamp = u64::MAX;

        for (id, status) in chains.iter_mut() {
            let head = self.supervisor.super_head(*id)?;

            // uses lowest safe and finalized timestamps, as well as l1 block, of all l2s
            //
            // <https://github.com/ethereum-optimism/optimism/blob/fac40575a8bcefd325c50a52e12b0e93254ac3f8/op-supervisor/supervisor/backend/status/status.go#L117-L131>
            //
            // todo: add to spec
            let SuperHead { l1_source, cross_safe, finalized, .. } = &head;
            if l1_source.number < min_synced_l1.number {
                min_synced_l1 = *l1_source;
            }
            if cross_safe.timestamp < cross_safe_timestamp {
                cross_safe_timestamp = cross_safe.timestamp;
            }
            if finalized.timestamp < finalized_timestamp {
                finalized_timestamp = finalized.timestamp;
            }

            *status = head.into()
        }

        Ok(SupervisorSyncStatus {
            min_synced_l1,
            cross_safe_timestamp,
            finalized_timestamp,
            chains,
        })
    }

    async fn all_safe_derived_at(
        &self,
        derived_from: BlockNumHash,
    ) -> RpcResult<HashMap<ChainId, BlockNumHash>> {
        trace!(target: "supervisor_rpc",
            ?derived_from,
            "Received all_safe_derived_at request"
        );

        let mut chains = self
            .supervisor
            .chain_ids()
            .map(|id| (id, Default::default()))
            .collect::<HashMap<_, BlockNumHash>>();

        for (id, block) in chains.iter_mut() {
            *block = self.supervisor.latest_block_from(derived_from, *id)?.id();
        }

        Ok(chains)
    }
}

impl<T> Clone for SupervisorRpc<T> {
    fn clone(&self) -> Self {
        Self { supervisor: self.supervisor.clone() }
    }
}
