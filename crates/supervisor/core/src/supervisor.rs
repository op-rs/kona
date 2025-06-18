use core::fmt::Debug;

use alloy_eips::BlockNumHash;
use alloy_network::Ethereum;
use alloy_primitives::{B256, ChainId};
use alloy_provider::RootProvider;
use alloy_rpc_client::RpcClient;
use async_trait::async_trait;
use kona_interop::{DependencySet, ExecutingDescriptor, SafetyLevel};
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{
    ChainDb, ChainDbFactory, DerivationStorageReader, FinalizedL1Storage, HeadRefStorageReader,
};
use kona_supervisor_types::SuperHead;
use reqwest::Url;
use std::{collections::HashMap, sync::Arc};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::{
    ChainProcessor, SupervisorError, config::Config, l1_watcher::L1Watcher, syncnode::ManagedNode,
};

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

    async fn check_access_list(
        &self,
        _inbox_entries: Vec<B256>,
        _min_safety: SafetyLevel,
        _executing_descriptor: ExecutingDescriptor,
    ) -> Result<(), SupervisorError> {
        Err(SupervisorError::Unimplemented)
    }
}
