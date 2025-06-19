//! Server-side implementation of the Supervisor RPC API.

use crate::{SupervisorError, SupervisorService};
use alloy_eips::eip1898::BlockNumHash;
use alloy_primitives::{B256, ChainId, map::HashMap};
use async_trait::async_trait;
use jsonrpsee::{
    core::RpcResult,
    types::{ErrorObject, error::ErrorCode},
};
use kona_interop::{
    DependencySet, DerivedIdPair, ExecutingDescriptor, SafetyLevel, SuperRootResponse,
};
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
        crate::observe_rpc_call!(
            "cross_derived_to_source",
            async {
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
            .await
        )
    }

    async fn local_unsafe(&self, chain_id: ChainId) -> RpcResult<BlockNumHash> {
        crate::observe_rpc_call!(
            "local_unsafe",
            async {
                trace!(target: "supervisor_rpc",
                    %chain_id,
                    "Received local_unsafe request"
                );

                Ok(self.supervisor.local_unsafe(chain_id)?.id())
            }
            .await
        )
    }

    async fn dependency_set_v1(&self) -> RpcResult<DependencySet> {
        crate::observe_rpc_call!(
            "dependency_set",
            async {
                trace!(target: "supervisor_rpc",
                    "Received the dependency set"
                );

                Ok(self.supervisor.dependency_set().to_owned())
            }
            .await
        )
    }

    async fn cross_safe(&self, chain_id: ChainId) -> RpcResult<DerivedIdPair> {
        crate::observe_rpc_call!(
            "cross_safe",
            async {
                trace!(target: "supervisor_rpc",
                    %chain_id,
                    "Received cross_safe request"
                );

                let derived = self.supervisor.cross_safe(chain_id)?.id();
                let source = self.supervisor.derived_to_source_block(chain_id, derived)?.id();

                Ok(DerivedIdPair { source, derived })
            }
            .await
        )
    }

    async fn finalized(&self, chain_id: ChainId) -> RpcResult<BlockNumHash> {
        crate::observe_rpc_call!(
            "finalized",
            async {
                trace!(target: "supervisor_rpc",
                    %chain_id,
                    "Received finalized request"
                );

                Ok(self.supervisor.finalized(chain_id)?.id())
            }
            .await
        )
    }

    async fn finalized_l1(&self) -> RpcResult<BlockInfo> {
        crate::observe_rpc_call!(
            "finalized_l1",
            async {
                trace!(target: "supervisor_rpc", "Received finalized_l1 request");
                Ok(self.supervisor.finalized_l1()?)
            }
            .await
        )
    }

    async fn super_root_at_timestamp(&self, timestamp: u64) -> RpcResult<SuperRootResponse> {
        crate::observe_rpc_call!(
            "super_root_at_timestamp",
            async {
        trace!(target: "supervisor_rpc",
            %timestamp,
            "Received super_root_at_timestamp request"
        );
        warn!(target: "supervisor_rpc", "super_root_at_timestamp method not yet implemented");
        Err(ErrorObject::from(ErrorCode::InternalError))
        }.await)
    }

    async fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> RpcResult<()> {
        // TODO:: refcator, maybe build proc macro to record metrics
        crate::observe_rpc_call!(
            "check_access_list", 
            async {
            trace!(target: "supervisor_rpc", 
                num_inbox_entries = inbox_entries.len(),
                ?min_safety,
                ?executing_descriptor,
                "Received check_access_list request",
            );
            self.supervisor
                .check_access_list(inbox_entries, min_safety, executing_descriptor)
                .map_err(|err| {
                    warn!(target: "supervisor_rpc", %err, "Error from core supervisor check_access_list");
                    ErrorObject::from(err)
                })
        }.await)
    }

    async fn sync_status(&self) -> RpcResult<SupervisorSyncStatus> {
        crate::observe_rpc_call!(
            "sync_status",
            async {
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
            .await
        )
    }

    async fn all_safe_derived_at(
        &self,
        derived_from: BlockNumHash,
    ) -> RpcResult<HashMap<ChainId, BlockNumHash>> {
        crate::observe_rpc_call!(
            "all_safe_derived_at",
            async {
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
            .await
        )
    }
}

impl<T> Clone for SupervisorRpc<T> {
    fn clone(&self) -> Self {
        Self { supervisor: self.supervisor.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::ChainId;
    use kona_interop::ChainDependency;
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::StorageError;
    use std::sync::Arc;

    #[derive(Debug)]
    struct MockSupervisorService {
        pub chain_ids: Vec<ChainId>,
        pub super_head_map: std::collections::HashMap<ChainId, SuperHead>,
        pub dependency_set: DependencySet,
    }

    impl SupervisorService for MockSupervisorService {
        fn chain_ids(&self) -> impl Iterator<Item = ChainId> {
            self.chain_ids.clone().into_iter()
        }

        fn dependency_set(&self) -> &DependencySet {
            &self.dependency_set
        }

        fn super_head(&self, chain: ChainId) -> Result<SuperHead, SupervisorError> {
            self.super_head_map.get(&chain).cloned().ok_or_else(|| {
                SupervisorError::StorageError(StorageError::EntryNotFound(
                    "superhead not found".to_string(),
                ))
            })
        }

        fn latest_block_from(
            &self,
            _l1_block: BlockNumHash,
            _chain: ChainId,
        ) -> Result<BlockInfo, SupervisorError> {
            unimplemented!()
        }

        fn derived_to_source_block(
            &self,
            _chain: ChainId,
            _derived: BlockNumHash,
        ) -> Result<BlockInfo, SupervisorError> {
            unimplemented!()
        }

        fn local_unsafe(&self, _chain: ChainId) -> Result<BlockInfo, SupervisorError> {
            unimplemented!()
        }

        fn cross_safe(&self, _chain: ChainId) -> Result<BlockInfo, SupervisorError> {
            unimplemented!()
        }

        fn finalized(&self, _chain: ChainId) -> Result<BlockInfo, SupervisorError> {
            unimplemented!()
        }

        fn finalized_l1(&self) -> Result<BlockInfo, SupervisorError> {
            unimplemented!()
        }

        fn check_access_list(
            &self,
            _inbox_entries: Vec<B256>,
            _min_safety: SafetyLevel,
            _executing_descriptor: ExecutingDescriptor,
        ) -> Result<(), SupervisorError> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_sync_status_empty_chains() {
        let mut deps = HashMap::default();
        deps.insert(1, ChainDependency {});
        let ds = DependencySet { dependencies: deps, override_message_expiry_window: Some(0) };

        let mock_service = MockSupervisorService {
            chain_ids: vec![],
            super_head_map: std::collections::HashMap::new(),
            dependency_set: ds,
        };

        let rpc = SupervisorRpc::new(Arc::new(mock_service));
        let result = rpc.sync_status().await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ErrorObject::from(SupervisorError::EmptyDependencySet));
    }

    #[tokio::test]
    async fn test_sync_status_single_chain() {
        let mut deps = HashMap::default();
        deps.insert(1, ChainDependency {});
        let ds = DependencySet { dependencies: deps, override_message_expiry_window: Some(0) };
        let chain_id = ChainId::from(1u64);

        let block_info = BlockInfo { number: 42, ..Default::default() };
        let super_head = SuperHead {
            l1_source: block_info,
            cross_safe: BlockInfo { timestamp: 100, ..Default::default() },
            finalized: BlockInfo { timestamp: 50, ..Default::default() },
            ..Default::default()
        };

        let mut super_head_map = std::collections::HashMap::new();
        super_head_map.insert(chain_id, super_head);

        let mock_service =
            MockSupervisorService { chain_ids: vec![chain_id], super_head_map, dependency_set: ds };

        assert_eq!(mock_service.dependency_set.dependencies.len(), 1);

        let rpc = SupervisorRpc::new(Arc::new(mock_service));
        let result = rpc.sync_status().await.unwrap();

        assert_eq!(result.min_synced_l1.number, 42);
        assert_eq!(result.cross_safe_timestamp, 100);
        assert_eq!(result.finalized_timestamp, 50);
        assert_eq!(result.chains.len(), 1);
    }

    #[tokio::test]
    async fn test_sync_status_missing_super_head() {
        let mut deps = HashMap::default();
        deps.insert(1, ChainDependency {});
        deps.insert(2, ChainDependency {});
        let ds = DependencySet { dependencies: deps, override_message_expiry_window: Some(0) };
        let chain_id_1 = ChainId::from(1u64);
        let chain_id_2 = ChainId::from(2u64);

        // Only chain_id_1 has a SuperHead, chain_id_2 is missing
        let block_info = BlockInfo { number: 42, ..Default::default() };
        let super_head = SuperHead {
            l1_source: block_info,
            cross_safe: BlockInfo { timestamp: 100, ..Default::default() },
            finalized: BlockInfo { timestamp: 50, ..Default::default() },
            ..Default::default()
        };

        let mut super_head_map = std::collections::HashMap::new();
        super_head_map.insert(chain_id_1, super_head);

        let mock_service = MockSupervisorService {
            chain_ids: vec![chain_id_1, chain_id_2],
            super_head_map,
            dependency_set: ds,
        };

        let rpc = SupervisorRpc::new(Arc::new(mock_service));
        let result = rpc.sync_status().await;

        assert!(result.is_err());
    }
}
