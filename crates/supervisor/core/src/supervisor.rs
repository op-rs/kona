use crate::{
    ChainProcessor, SupervisorError, config::Config, l1_watcher::L1Watcher, syncnode::ManagedNode,
};
use alloy_eips::BlockNumHash;
use alloy_network::Ethereum;
use alloy_primitives::{B256, ChainId};
use alloy_provider::RootProvider;
use alloy_rpc_client::RpcClient;
use async_trait::async_trait;
use core::fmt::Debug;
use kona_interop::{DependencySet, ExecutingDescriptor, SafetyLevel};
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{
    ChainDb, ChainDbFactory, DerivationStorageReader, FinalizedL1Storage, HeadRefStorageReader,
    LogStorageReader,
};
use kona_supervisor_types::{SuperHead, parse_access_list};
use op_alloy_rpc_types::SuperchainDAError;
use reqwest::Url;
use std::{collections::HashMap, sync::Arc};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Defines the service for the Supervisor core logic.
#[async_trait]
#[auto_impl::auto_impl(&, &mut, Arc, Box)]
pub trait SupervisorService: Debug + Send + Sync {
    /// Returns list of supervised [`ChainId`]s.
    fn chain_ids(&self) -> impl Iterator<Item = ChainId>;

    /// Returns mapping of supervised [`ChainId`]s to their [`ChainDependency`] config.
    ///
    /// [`ChainDependency`]: kona_interop::ChainDependency
    fn dependency_set(&self) -> &DependencySet;

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

    /// Returns [`LocalUnsafe`] block for the given chain.
    ///
    /// [`LocalUnsafe`]: SafetyLevel::LocalUnsafe
    fn local_unsafe(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError>;

    /// Returns [`CrossSafe`] block for the given chain.
    ///
    /// [`CrossSafe`]: SafetyLevel::CrossSafe
    fn cross_safe(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError>;

    /// Returns [`Finalized`] block for the given chain.
    ///
    /// [`Finalized`]: SafetyLevel::Finalized
    fn finalized(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError>;

    /// Returns the finalized L1 block that the supervisor is synced to.
    fn finalized_l1(&self) -> Result<BlockInfo, SupervisorError>;

    /// Verifies if an access-list references only valid messages
    fn check_access_list(
        &self,
        inbox_entries: Vec<B256>,
        min_safety: SafetyLevel,
        executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError>;
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
        self.init_chain_processor().await?;
        self.init_l1_watcher()?;
        Ok(())
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
            let url = Url::parse(&self.config.l1_rpc).map_err(|e| {
                error!(target: "supervisor_service", %e, "Failed to parse L1 RPC URL");
                SupervisorError::Initialise("invalid l1 rpc url".to_string())
            })?;
            let provider = RootProvider::<Ethereum>::new_http(url);
            let mut managed_node = ManagedNode::<ChainDb>::new(
                Arc::new(config.clone()),
                self.cancel_token.clone(),
                provider,
            );

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

    fn init_l1_watcher(&self) -> Result<(), SupervisorError> {
        let l1_rpc = RpcClient::new_http(self.config.l1_rpc.parse().unwrap());
        let l1_watcher =
            L1Watcher::new(l1_rpc, self.database_factory.clone(), self.cancel_token.clone());

        tokio::spawn(async move {
            l1_watcher.run().await;
        });
        Ok(())
    }

    fn verify_safety_level(
        &self,
        chain_id: ChainId,
        block: &BlockInfo,
        safety: SafetyLevel,
    ) -> Result<(), SupervisorError> {
        // check block exists on the derivation storage
        self.database_factory
            .get_db(chain_id)?
            .derived_to_source(BlockNumHash { number: block.number, hash: block.hash })?;

        let head_ref = self.database_factory.get_db(chain_id)?.get_safety_head_ref(safety)?;

        if head_ref.number < block.number {
            return Err(SupervisorError::from(SuperchainDAError::ConflictingData));
        }

        Ok(())
    }
}

#[async_trait]
impl SupervisorService for Supervisor {
    fn chain_ids(&self) -> impl Iterator<Item = ChainId> {
        self.config.dependency_set.dependencies.keys().copied()
    }

    fn dependency_set(&self) -> &DependencySet {
        &self.config.dependency_set
    }

    fn super_head(&self, chain: ChainId) -> Result<SuperHead, SupervisorError> {
        let db = self.database_factory.get_db(chain)?;
        Ok(db.get_super_head()?)
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

    fn local_unsafe(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError> {
        Ok(self.database_factory.get_db(chain)?.get_safety_head_ref(SafetyLevel::LocalUnsafe)?)
    }

    fn cross_safe(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError> {
        Ok(self.database_factory.get_db(chain)?.get_safety_head_ref(SafetyLevel::CrossSafe)?)
    }

    fn finalized(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError> {
        Ok(self.database_factory.get_db(chain)?.get_safety_head_ref(SafetyLevel::Finalized)?)
    }

    fn finalized_l1(&self) -> Result<BlockInfo, SupervisorError> {
        Ok(self.database_factory.get_finalized_l1()?)
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
            let rollup_config = &self.config.rollup_config_set;
            if !rollup_config
                .is_interop_enabled(initiating_chain_id, executing_descriptor.timestamp) ||
                !rollup_config.is_interop_enabled(executing_chain_id, access.timestamp)
            {
                return Err(SupervisorError::from(SuperchainDAError::ConflictingData)); // todo: replace with better error
            }

            // Verify the initiating message exists and valid for corresponding executing message.
            let db = self.database_factory.get_db(initiating_chain_id)?;

            let block = db.get_block(access.block_number)?;
            if block.timestamp != access.timestamp {
                return Err(SupervisorError::from(SuperchainDAError::ConflictingData))
            }

            let log = db.get_log(access.block_number, access.log_index)?;
            access.verify_checksum(&log.hash)?;

            // The message must be included in a block that is at least as safe as required
            // by the `min_safety` level
            if min_safety != SafetyLevel::LocalUnsafe {
                // The block is already unsafe as it is found in log db
                self.verify_safety_level(initiating_chain_id, &block, min_safety)?;
            }
        }

        Ok(())
    }
}
