use core::fmt::Debug;
use op_alloy_rpc_types::InvalidInboxEntry;

use alloy_eips::BlockNumHash;
use alloy_primitives::{B256, ChainId};
use async_trait::async_trait;
use jsonrpsee::types::{ErrorCode, ErrorObjectOwned};
use kona_interop::{ExecutingDescriptor, SafetyLevel};
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{
    ChainDb, ChainDbFactory, DerivationStorageReader, HeadRefStorageReader, LogStorageReader,
    StorageError,
};
use kona_supervisor_types::{AccessListError, SuperHead, parse_access_list};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

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

    /// Indicates the error occurred while parsing the access_list
    #[error(transparent)]
    AccessListError(#[from] AccessListError),
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
            SupervisorError::AccessListError(_) |
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

    /// Returns latest block derived from given L1 block, for given chain.
    fn latest_block_from(
        &self,
        l1_block: BlockNumHash,
        chain: ChainId,
    ) -> Result<BlockInfo, SupervisorError>;

    /// Returns the L1 source block that the given L2 derived block was based on, for the specified
    /// chain.
    fn derived_to_source_block(
        &self,
        chain: ChainId,
        derived: BlockNumHash,
    ) -> Result<BlockInfo, SupervisorError>;

    /// Returns the
    /// Verifies if an access-list references only valid messages
    fn check_access_list(
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
    managed_nodes: HashMap<ChainId, Arc<ManagedNode<ChainDb>>>,
    chain_processors: HashMap<ChainId, ChainProcessor<ManagedNode<ChainDb>, ChainDb>>,

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
        self.init_database().await?;
        self.init_managed_nodes().await?;
        self.init_chain_processor().await
    }

    async fn init_database(&self) -> Result<(), SupervisorError> {
        for (chain_id, config) in self.config.rollup_config_set.rollups.iter() {
            // Initialise the database for each chain.
            let db = self.database_factory.get_or_create_db(*chain_id)?;
            db.initialise(config.genesis.get_anchor())?;
            info!(target: "supervisor_service", chain_id, "Database initialized successfully");
        }
        Ok(())
    }

    async fn init_chain_processor(&mut self) -> Result<(), SupervisorError> {
        // Initialise the service components, such as database connections or other resources.

        for (chain_id, _) in self.config.rollup_config_set.rollups.iter() {
            let db = self.database_factory.get_db(*chain_id)?;
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
            let mut managed_node =
                ManagedNode::<ChainDb>::new(Arc::new(config.clone()), self.cancel_token.clone());

            let chain_id = managed_node.chain_id().await?;
            let db = self.database_factory.get_db(chain_id)?;
            managed_node.set_db_provider(db);

            if self.managed_nodes.contains_key(&chain_id) {
                warn!(target: "supervisor_service", "Managed node for chain {chain_id} already exists, skipping initialization");
                continue;
            }
            self.managed_nodes.insert(chain_id, Arc::new(managed_node));
            info!(target: "supervisor_service",
                 chain_id,
                "Managed node for chain initialized successfully",
            );
        }
        Ok(())
    }

    fn is_interop_enabled(&self, chain_id: ChainId, timestamp: u64) -> bool {
        self.config
            .rollup_config_set
            .get(chain_id)
            .map(|cfg| cfg.is_post_interop(timestamp))
            .unwrap_or(false) // if config not found, return false
    }

    fn verify_safety_level(
        &self,
        chain_id: ChainId,
        block: &BlockInfo,
        safety: SafetyLevel,
    ) -> Result<(), SupervisorError> {
        let derived = self
            .database_factory
            .get_db(chain_id)?
            .derived_to_source(BlockNumHash { number: block.number, hash: block.hash })?;

        if derived.hash != block.hash {
            return Err(SupervisorError::from(InvalidInboxEntry::ConflictingData));
        }

        let head_ref = self.database_factory.get_db(chain_id)?.get_safety_head_ref(safety)?;

        if head_ref.number < block.number {
            return Err(SupervisorError::from(InvalidInboxEntry::ConflictingData));
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

    fn latest_block_from(
        &self,
        l1_block: BlockNumHash,
        chain: ChainId,
    ) -> Result<BlockInfo, SupervisorError> {
        Ok(self.database_factory.get_db(chain)?.latest_derived_block_at_source(l1_block)?)
    }

    fn derived_to_source_block(
        &self,
        chain: ChainId,
        derived: BlockNumHash,
    ) -> Result<BlockInfo, SupervisorError> {
        Ok(self.database_factory.get_db(chain)?.derived_to_source(derived)?)
    }

    fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        let access_list = parse_access_list(inbox_entries)?;
        let message_expiry_window = self.config.dependency_set.get_message_expiry_window();
        let timeout = executing_descriptor.timeout.unwrap_or(0);
        let executing_ts_with_duration = executing_descriptor.timestamp.saturating_add(timeout);

        for access in &access_list {
            // Check all the invariants for each message
            // Ref: https://github.com/ethereum-optimism/specs/blob/main/specs/interop/derivation.md#invariants

            // TODO: support 32 bytes chain id and convert to u64 via dependency set to be usable
            // across services
            let initiating_chain_id =
                u64::from_be_bytes(access.chain_id[24..32].try_into().unwrap());

            // TODO: Extend the `ExecutingDescriptor` to accept chain_id.
            // And set executing_chain_id as initiating_chain_id only for backward compat.
            let executing_chain_id = initiating_chain_id;

            // Message must be valid at the time of execution.
            access.validate_message_lifetime(
                executing_descriptor.timestamp,
                executing_ts_with_duration,
                message_expiry_window,
            )?;

            // The interop fork must be active for both the executing and
            // initiating chains at the respective timestamps.
            if !self.is_interop_enabled(initiating_chain_id, executing_descriptor.timestamp) ||
                !self.is_interop_enabled(executing_chain_id, access.timestamp)
            {
                return Err(SupervisorError::from(InvalidInboxEntry::ConflictingData));
            }

            // Verify the initiating message exists and valid for corresponding executing message.
            let block = self
                .database_factory
                .get_db(initiating_chain_id)?
                .get_block(access.block_number)?;
            if block.timestamp != access.timestamp {
                return Err(SupervisorError::from(InvalidInboxEntry::ConflictingData))
            }
            let log = self
                .database_factory
                .get_db(initiating_chain_id)?
                .get_log(access.block_number, access.log_index)?;
            access.verify_checksum(&log.hash)?;

            // The message must be included in a block that is at least as safe as required
            // by the `min_safety` level
            if min_safety != SafetyLevel::Unsafe {
                // The block is already unsafe as it is found in log db
                self.verify_safety_level(initiating_chain_id, &block, min_safety)?;
            }
        }

        Ok(())
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
