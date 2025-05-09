//! Server-side implementation of the Supervisor RPC API.

use crate::supervisor::SupervisorService;
use alloy_eips::eip1898::BlockNumHash;
use alloy_primitives::{B256, ChainId};
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorObject, error::ErrorCode},
};
use kona_interop::{DerivedIdPair, ExecutingDescriptor, SafetyLevel, SuperRootResponse};
use kona_supervisor_rpc::SupervisorApiServer;
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
    async fn local_unsafe(&self, chain_id: ChainId) -> RpcResult<BlockNumHash> {
        trace!(target: "supervisor_rpc",
            %chain_id,
            "Received local_unsafe request"
        );
        // self.supervisor.local_unsafe()
        // .await
        // .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
        warn!(target: "supervisor_rpc", "local_unsafe method not yet implemented");
        Err(ErrorObject::from(ErrorCode::InternalError))
    }

    async fn cross_safe(&self, chain_id: ChainId) -> RpcResult<DerivedIdPair> {
        trace!(target: "supervisor_rpc",
            %chain_id,
            "Received cross_safe request"
        );
        // self.supervisor.cross_safe()
        // .await
        // .map_err(|_| ErrorObject::from(ErrorCode::InternalError))?;
        warn!(target: "supervisor_rpc", "cross_safe method not yet implemented");
        Err(ErrorObject::from(ErrorCode::InternalError))
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
}

impl<T> Clone for SupervisorRpc<T> {
    fn clone(&self) -> Self {
        Self { supervisor: self.supervisor.clone() }
    }
}
