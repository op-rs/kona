use core::fmt::Debug;

use alloy_primitives::{B256, ChainId};
use async_trait::async_trait;
use jsonrpsee::types::{ErrorCode, ErrorObjectOwned};
use kona_interop::{ExecutingDescriptor, SafetyLevel};
use kona_supervisor_storage::{ChainDb, ChainDbFactory, StorageError};
use kona_supervisor_types::SuperHead;
use op_alloy_rpc_types::InvalidInboxEntry;
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::{
    chain_processor::{ChainProcessor, ChainProcessorError},
    config::Config,
    syncnode::{ManagedNode, ManagedNodeError},
};

/// Custom error type for the Supervisor core logic.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SupervisorError {
    /// Indicates that a feature or method is not yet implemented.
    #[error("functionality not implemented")]
    Unimplemented,
    /// No chains are configured for supervision.
    #[error("empty dependency set")]
    EmptyDependencySet,
    /// Data availability errors.
    ///
    /// Spec <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#protocol-specific-error-codes>.
    #[error(transparent)]
    InvalidInboxEntry(#[from] InvalidInboxEntry),

    /// Indicates that the supervisor was unable to initialise due to an error.
    #[error("unable to initialize the supervisor: {0}")]
    Initialise(String),

    /// Indicates that error occurred while interacting with the storage layer.
    #[error(transparent)]
    StorageError(#[from] StorageError),

    /// Indicates the error occured while interacting with the managed node.
    #[error(transparent)]
    ManagedNodeError(#[from] ManagedNodeError),

    /// Indicates the error occured while processing the chain.
    #[error(transparent)]
    ChainProcessorError(#[from] ChainProcessorError),
}

impl From<SupervisorError> for ErrorObjectOwned {
    fn from(err: SupervisorError) -> Self {
        match err {
            // todo: handle these errors more gracefully
            SupervisorError::Unimplemented |
            SupervisorError::EmptyDependencySet |
            SupervisorError::Initialise(_) |
            SupervisorError::StorageError(_) |
            SupervisorError::ManagedNodeError(_) |
            SupervisorError::ChainProcessorError(_) => {
                ErrorObjectOwned::from(ErrorCode::InternalError)
            }
            SupervisorError::InvalidInboxEntry(err) => ErrorObjectOwned::owned(
                (err as i64).try_into().expect("should fit i32"),
                err.to_string(),
                None::<()>,
            ),
        }
    }
}

/// Defines the service for the Supervisor core logic.
#[async_trait]
#[auto_impl::auto_impl(&, &mut, Arc, Box)]
pub trait SupervisorService: Debug + Send + Sync {
    /// Returns list of supervised [`ChainId`]s.
    fn chain_ids(&self) -> impl Iterator<Item = ChainId>;

    /// Returns [`SuperHead`] of given supervised chain.
    fn super_head(&self, chain: ChainId) -> Result<SuperHead, SupervisorError>;

    /// Verifies if an access-list references only valid messages
    async fn check_access_list(
        &self,
        _inbox_entries: Vec<B256>,
        _min_safety: SafetyLevel,
        _executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        Err(SupervisorError::Unimplemented)
    }
}

/// The core Supervisor component responsible for monitoring and coordinating chain states.
#[derive(Debug)]
pub struct Supervisor {
    config: Config,
    database_factory: Arc<ChainDbFactory>,

    // As of now supervisor only supports a single managed node per chain.
    // This is a limitation of the current implementation, but it will be extended in the future.
    managed_nodes: HashMap<ChainId, Arc<ManagedNode>>,
    chain_processors: HashMap<ChainId, ChainProcessor<ManagedNode, ChainDb>>,

    cancel_token: CancellationToken,
}

impl Supervisor {
    /// Creates a new [`Supervisor`] instance.
    #[allow(clippy::new_without_default, clippy::missing_const_for_fn)]
    pub fn new(
        config: Config,
        database_factory: Arc<ChainDbFactory>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            config,
            database_factory,
            managed_nodes: HashMap::new(),
            chain_processors: HashMap::new(),
            cancel_token,
        }
    }

    /// Initialises the Supervisor service.
    pub async fn initialise(&mut self) -> Result<(), SupervisorError> {
        self.init_managed_nodes().await?;
        self.init_chain_processor().await
    }

    async fn init_chain_processor(&mut self) -> Result<(), SupervisorError> {
        // Initialise the service components, such as database connections or other resources.

        for (chain_id, config) in self.config.rollup_config_set.rollups.iter() {
            // initialise the chain database for each chain.
            let db = self.database_factory.get_or_create_db(*chain_id)?;
            db.initialise(config.genesis.get_anchor())?;

            let managed_node =
                self.managed_nodes.get(chain_id).ok_or(SupervisorError::Initialise(format!(
                    "no managed node found for chain {}",
                    chain_id
                )))?;

            // initialise chain processor for the chain.
            let processor =
                ChainProcessor::new(*chain_id, managed_node.clone(), db, self.cancel_token.clone());

            // Start the chain processors.
            // Each chain processor will start its own managed nodes and begin processing messages.
            processor.start().await?;
            self.chain_processors.insert(*chain_id, processor);
        }
        Ok(())
    }

    async fn init_managed_nodes(&mut self) -> Result<(), SupervisorError> {
        for config in self.config.l2_consensus_nodes_config.iter() {
            let managed_node =
                ManagedNode::new(Arc::new(config.clone()), self.cancel_token.clone());

            let chain_id = managed_node.chain_id().await?;
            if !self.managed_nodes.contains_key(&chain_id) {
                warn!(target: "supervisor_service", "Managed node for chain {chain_id} already exists, skipping initialization");
                continue;
            }
            self.managed_nodes.insert(chain_id, Arc::new(managed_node));
        }
        Ok(())
    }
}

#[async_trait]
impl SupervisorService for Supervisor {
    fn chain_ids(&self) -> impl Iterator<Item = ChainId> {
        self.config.dependency_set.dependencies.keys().copied()
    }

    fn super_head(&self, _chain: ChainId) -> Result<SuperHead, SupervisorError> {
        todo!("implement call to ChainDbFactory")
    }

    async fn check_access_list(
        &self,
        _inbox_entries: Vec<B256>,
        _min_safety: SafetyLevel,
        _executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        Err(SupervisorError::Unimplemented)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rpc_error_conversion() {
        let err = InvalidInboxEntry::UnknownChain;
        let rpc_err = ErrorObjectOwned::owned(err as i32, err.to_string(), None::<()>);

        assert_eq!(ErrorObjectOwned::from(SupervisorError::InvalidInboxEntry(err)), rpc_err);
    }
}
