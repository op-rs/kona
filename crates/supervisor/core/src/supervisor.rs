use core::fmt::Debug;

use alloy_primitives::{B256, ChainId};
use async_trait::async_trait;
use jsonrpsee::types::ErrorObjectOwned;
use kona_interop::{ExecutingDescriptor, SafetyLevel};
use kona_supervisor_rpc::SupervisorApiServer;
use kona_supervisor_storage::{ChainDbFactory, StorageError};
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
    /// Data availability errors.
    ///
    /// Spec <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#protocol-specific-error-codes>.
    #[error(transparent)]
    InvalidInboxEntry(#[from] InvalidInboxEntry),

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

impl From<ErrorObjectOwned> for SupervisorError {
    fn from(err: ErrorObjectOwned) -> Self {
        let Ok(err) = (err.code() as i64).try_into() else {
            return Self::Unimplemented;
        };
        Self::InvalidInboxEntry(err)
    }
}

/// Defines the service for the Supervisor core logic.
#[async_trait]
pub trait SupervisorService: Debug + Send + Sync {
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
    chain_processors: HashMap<ChainId, ChainProcessor>,

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
        Self { config, database_factory, chain_processors: HashMap::new(), cancel_token }
    }

    /// Initialises the Supervisor service.
    pub async fn initialise(&mut self) -> Result<(), SupervisorError> {
        self.init_chain_processor()?;
        self.init_manged_nodes().await?;

        // Start the chain processors.
        // This will start the chain processors for each chain in the rollup config.
        // Each chain processor will start its own managed nodes and begin processing messages.
        for processor in self.chain_processors.values_mut() {
            processor.start().await?;
        }
        Ok(())
    }

    fn init_chain_processor(&mut self) -> Result<(), SupervisorError> {
        // Initialise the service components, such as database connections or other resources.

        for (chain_id, config) in self.config.rollup_config_set.rollups.iter() {
            // initialise the chain database for each chain.
            let db = self.database_factory.get_or_create_db(*chain_id)?;
            db.initialise(config.genesis.get_anchor())?;

            // initialise chain processor for the chain.
            let processor = ChainProcessor::new(*chain_id, self.cancel_token.clone());
            self.chain_processors.insert(*chain_id, processor);
        }
        Ok(())
    }

    async fn init_manged_nodes(&mut self) -> Result<(), SupervisorError> {
        for config in self.config.l2_consensus_nodes_config.iter() {
            let managed_node =
                ManagedNode::new(Arc::new(config.clone()), self.cancel_token.clone());

            let chain_id = managed_node.chain_id().await?;
            if let Some(processor) = self.chain_processors.get_mut(&chain_id) {
                processor.add_managed_node(managed_node).await?;
            } else {
                warn!(target: "supervisor",
                    chain_id = chain_id,
                    "No ChainProcessor found for ManagedNode with chain_id"
                );
            }
        }
        Ok(())
    }
}

#[async_trait]
impl SupervisorService for Supervisor {
    async fn check_access_list(
        &self,
        _inbox_entries: Vec<B256>,
        _min_safety: SafetyLevel,
        _executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        Err(SupervisorError::Unimplemented)
    }
}

#[async_trait]
impl<T> SupervisorService for T
where
    T: SupervisorApiServer + Debug,
{
    async fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        // codecov:ignore-start
        Ok(T::check_access_list(self, inbox_entries, min_safety, executing_descriptor).await?)
        // codecov:ignore-end
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rpc_error_conversion() {
        let err_code = InvalidInboxEntry::UnknownChain;
        let err = ErrorObjectOwned::owned(err_code as i32, "", None::<()>);

        assert_eq!(SupervisorError::InvalidInboxEntry(err_code), err.into());

        let err = ErrorObjectOwned::owned(i32::MAX, "", None::<()>);

        assert_eq!(SupervisorError::Unimplemented, err.into())
    }
}
