use alloy_eips::BlockNumHash;
use alloy_network::Ethereum;
use alloy_primitives::{B256, Bytes, ChainId, keccak256};
use alloy_provider::RootProvider;
use alloy_rpc_client::RpcClient;
use async_trait::async_trait;
use core::fmt::Debug;
use kona_interop::{
    DependencySet, ExecutingDescriptor, OutputRootWithChain, SUPER_ROOT_VERSION, SafetyLevel,
    SuperRoot,
};
use kona_protocol::BlockInfo;
use kona_supervisor_rpc::{ChainRootInfoRpc, SuperRootOutputRpc};
use kona_supervisor_storage::{
    ChainDb, ChainDbFactory, DerivationStorageReader, DerivationStorageWriter, FinalizedL1Storage,
    HeadRefStorageReader, LogStorageReader, LogStorageWriter,
};
use kona_supervisor_types::{SuperHead, parse_access_list};
use op_alloy_rpc_types::SuperchainDAError;
use reqwest::Url;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::{
    ChainProcessor, CrossSafetyCheckerJob, SpecError, SupervisorError,
    config::Config,
    event::ChainEvent,
    l1_watcher::L1Watcher,
    safety_checker::{CrossSafePromoter, CrossUnsafePromoter},
    syncnode::{Client, ManagedNode, ManagedNodeClient, ManagedNodeDataProvider},
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

    /// Returns the [`SuperRootOutput`] at a specified timestamp, which represents the global
    /// state across all monitored chains.
    ///
    /// [`SuperRootOutput`]: kona_interop::SuperRootOutput
    async fn super_root_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<SuperRootOutputRpc, SupervisorError>;

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
    managed_nodes: HashMap<ChainId, Arc<ManagedNode<ChainDb, Client>>>,
    chain_processors: HashMap<ChainId, ChainProcessor<ManagedNode<ChainDb, Client>, ChainDb>>,

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
        self.init_cross_safety_checker().await?;
        Ok(())
    }

    async fn init_database(&self) -> Result<(), SupervisorError> {
        for (chain_id, config) in self.config.rollup_config_set.rollups.iter() {
            // Initialise the database for each chain.
            let db = self.database_factory.get_or_create_db(*chain_id)?;
            let anchor = config.genesis.get_anchor();
            db.initialise_log_storage(anchor.derived)?;
            db.initialise_derivation_storage(anchor)?;
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

            let rollup_config =
                self.config.rollup_config_set.get(*chain_id).ok_or(SupervisorError::Initialise(
                    format!("no rollup config found for chain {}", chain_id),
                ))?;

            // initialise chain processor for the chain.
            let mut processor = ChainProcessor::new(
                rollup_config.clone(),
                *chain_id,
                managed_node.clone(),
                db,
                self.cancel_token.clone(),
            );

            // todo: enable metrics only if configured
            processor = processor.with_metrics();

            // Start the chain processors.
            // Each chain processor will start its own managed nodes and begin processing messages.
            processor.start().await?;
            self.chain_processors.insert(*chain_id, processor);
        }
        Ok(())
    }
    async fn init_cross_safety_checker(&self) -> Result<(), SupervisorError> {
        for (&chain_id, config) in &self.config.rollup_config_set.rollups {
            let db = Arc::clone(&self.database_factory);
            let cancel = self.cancel_token.clone();

            // todo: remove dependency from chain processors to get event txs
            // initialize event txs independently and pass at the time of initialization
            let processor = self.chain_processors.get(&chain_id).ok_or_else(|| {
                error!(target: "supervisor_service", %chain_id, "processor not initialized");
                SupervisorError::Initialise("processor not initialized".into())
            })?;

            let event_tx = processor.event_sender().ok_or_else(|| {
                error!(target: "supervisor_service", %chain_id, "no event tx found in chain processor");
                SupervisorError::Initialise("event sender not found".into())
            })?;

            let cross_safe_job = CrossSafetyCheckerJob::new(
                chain_id,
                db.clone(),
                cancel.clone(),
                Duration::from_secs(config.block_time),
                CrossSafePromoter,
                event_tx.clone(),
            );

            tokio::spawn(async move {
                cross_safe_job.run().await;
            });

            let cross_unsafe_job = CrossSafetyCheckerJob::new(
                chain_id,
                db,
                cancel,
                Duration::from_secs(config.block_time),
                CrossUnsafePromoter,
                event_tx,
            );

            tokio::spawn(async move {
                cross_unsafe_job.run().await;
            });
        }

        Ok(())
    }

    async fn init_managed_nodes(&mut self) -> Result<(), SupervisorError> {
        for config in self.config.l2_consensus_nodes_config.iter() {
            let url = Url::parse(&self.config.l1_rpc).map_err(|err| {
                error!(target: "supervisor_service", %err, "Failed to parse L1 RPC URL");
                SupervisorError::Initialise("invalid l1 rpc url".to_string())
            })?;
            let provider = RootProvider::<Ethereum>::new_http(url);
            let client = Arc::new(Client::new(config.clone()));

            let chain_id = client.chain_id().await.map_err(|err| {
                error!(target: "supervisor_service", %err, "Failed to get chain ID from client");
                SupervisorError::Initialise("failed to get chain id from client".to_string())
            })?;
            let db = self.database_factory.get_db(chain_id)?;

            let managed_node = ManagedNode::<ChainDb, Client>::new(
                client,
                db,
                self.cancel_token.clone(),
                provider,
            );

            if self.managed_nodes.contains_key(&chain_id) {
                warn!(target: "supervisor_service", %chain_id, "Managed node for chain already exists, skipping initialization");
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

        let mut senders = HashMap::<ChainId, mpsc::Sender<ChainEvent>>::new();
        for (chain_id, chain_processor) in &self.chain_processors {
            if let Some(sender) = chain_processor.event_sender() {
                senders.insert(*chain_id, sender);
            } else {
                error!(target: "supervisor_service", chain_id, "No sender found for chain processor");
                return Err(SupervisorError::Initialise(format!(
                    "no sender found for chain processor for chain {}",
                    chain_id
                )));
            }
        }

        let l1_watcher = L1Watcher::new(
            l1_rpc,
            self.database_factory.clone(),
            senders,
            self.cancel_token.clone(),
        );

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
        let head_ref = self.database_factory.get_db(chain_id)?.get_safety_head_ref(safety)?;

        if head_ref.number < block.number {
            return Err(SpecError::SuperchainDAError(SuperchainDAError::ConflictingData).into());
        }

        Ok(())
    }

    fn get_db(&self, chain: ChainId) -> Result<Arc<ChainDb>, SupervisorError> {
        self.database_factory.get_db(chain).map_err(|err| {
            error!(target: "supervisor_service", %chain, %err, "Failed to get database for chain");
            SpecError::from(err).into()
        })
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
        Ok(self.get_db(chain)?.get_super_head().map_err(|err| {
            error!(target: "supervisor_service", %chain, %err, "Failed to get super head for chain");
            SpecError::from(err)
        })?)
    }

    fn latest_block_from(
        &self,
        l1_block: BlockNumHash,
        chain: ChainId,
    ) -> Result<BlockInfo, SupervisorError> {
        Ok(self
            .get_db(chain)?
            .latest_derived_block_at_source(l1_block)
            .map_err(|err| {
                error!(target: "supervisor_service", %chain, %err, "Failed to get latest derived block at source for chain");
                SpecError::from(err)
            })?
        )
    }

    fn derived_to_source_block(
        &self,
        chain: ChainId,
        derived: BlockNumHash,
    ) -> Result<BlockInfo, SupervisorError> {
        Ok(self.get_db(chain)?.derived_to_source(derived).map_err(|err| {
            error!(target: "supervisor_service", %chain, %err, "Failed to get derived to source block for chain");
            SpecError::from(err)
        })?)
    }

    fn local_unsafe(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError> {
        Ok(self.get_db(chain)?.get_safety_head_ref(SafetyLevel::LocalUnsafe).map_err(|err| {
            error!(target: "supervisor_service", %chain, %err, "Failed to get local unsafe head ref for chain");
            SpecError::from(err)
        })?)
    }

    fn cross_safe(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError> {
        Ok(self.get_db(chain)?.get_safety_head_ref(SafetyLevel::CrossSafe).map_err(|err| {
            error!(target: "supervisor_service", %chain, %err, "Failed to get cross safe head ref for chain");
            SpecError::from(err)
        })?)
    }

    fn finalized(&self, chain: ChainId) -> Result<BlockInfo, SupervisorError> {
        Ok(self.get_db(chain)?.get_safety_head_ref(SafetyLevel::Finalized).map_err(|err| {
            error!(target: "supervisor_service", %chain, %err, "Failed to get finalized head ref for chain");
            SpecError::from(err)
        })?)
    }

    fn finalized_l1(&self) -> Result<BlockInfo, SupervisorError> {
        Ok(self.database_factory.get_finalized_l1().map_err(|err| {
            error!(target: "supervisor_service", %err, "Failed to get finalized L1");
            SpecError::from(err)
        })?)
    }

    async fn super_root_at_timestamp(
        &self,
        timestamp: u64,
    ) -> Result<SuperRootOutputRpc, SupervisorError> {
        let mut chain_ids = self.config.dependency_set.dependencies.keys().collect::<Vec<_>>();
        // Sorting chain ids for deterministic super root hash
        chain_ids.sort();

        let mut chain_infos = Vec::<ChainRootInfoRpc>::with_capacity(chain_ids.len());
        let mut super_root_chains = Vec::<OutputRootWithChain>::with_capacity(chain_ids.len());
        let mut cross_safe_source = BlockNumHash::default();

        for id in chain_ids {
            let managed_node = self.managed_nodes.get(id).unwrap();
            let output_v0 = managed_node.output_v0_at_timestamp(timestamp).await?;
            let output_v0_string = serde_json::to_string(&output_v0).unwrap();
            let canonical_root = keccak256(output_v0_string.as_bytes());

            let pending_output_v0 = managed_node.pending_output_v0_at_timestamp(timestamp).await?;
            let pending_output_v0_bytes = Bytes::copy_from_slice(
                serde_json::to_string(&pending_output_v0).unwrap().as_bytes(),
            );

            chain_infos.push(ChainRootInfoRpc {
                chain_id: *id,
                canonical: canonical_root,
                pending: pending_output_v0_bytes,
            });

            super_root_chains
                .push(OutputRootWithChain { chain_id: *id, output_root: canonical_root });

            let l2_block = managed_node.l2_block_ref_by_timestamp(timestamp).await?;
            let source = self
                .derived_to_source_block(*id, l2_block.id())
                .inspect_err(|err| {
                    error!(target: "supervisor_service", %id, %err, "Failed to get derived to source block for chain");
                })?;

            if cross_safe_source.number == 0 || cross_safe_source.number < source.number {
                cross_safe_source = source.id();
            }
        }

        let super_root = SuperRoot { timestamp, output_roots: super_root_chains };
        let super_root_hash = super_root.hash();

        Ok(SuperRootOutputRpc {
            cross_safe_derived_from: cross_safe_source,
            timestamp,
            super_root: super_root_hash,
            chains: chain_infos,
            version: SUPER_ROOT_VERSION,
        })
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

            let executing_chain_id = executing_descriptor.chain_id.unwrap_or(initiating_chain_id);

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
                return Err(SupervisorError::InteropNotEnabled);
            }

            // Verify the initiating message exists and valid for corresponding executing message.
            let db = self.get_db(initiating_chain_id)?;

            let block = db.get_block(access.block_number).map_err(|err| {
                error!(target: "supervisor_service", %initiating_chain_id, %err, "Failed to get block for chain");
                SpecError::from(err)
            })?;
            if block.timestamp != access.timestamp {
                return Err(SupervisorError::from(SpecError::SuperchainDAError(
                    SuperchainDAError::ConflictingData,
                )))
            }

            let log = db.get_log(access.block_number, access.log_index).map_err(|err| {
                error!(target: "supervisor_service", %initiating_chain_id, %err, "Failed to get log for chain");
                SpecError::from(err)
            })?;
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
