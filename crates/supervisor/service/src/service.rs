//! Contains the main Supervisor service runner.

use alloy_primitives::ChainId;
use alloy_provider::{RootProvider, network::Ethereum};
use alloy_rpc_client::RpcClient;
use anyhow::Result;
use jsonrpsee::client_transport::ws::Url;
use kona_supervisor_core::{
    ChainProcessor, CrossSafetyCheckerJob, ReorgHandler, Supervisor,
    config::Config,
    event::ChainEvent,
    l1_watcher::L1Watcher,
    safety_checker::{CrossSafePromoter, CrossUnsafePromoter},
    syncnode::{Client, ManagedNode, ManagedNodeClient, ManagedNodeCommand},
};
use kona_supervisor_storage::{ChainDb, ChainDbFactory, DerivationStorageWriter, LogStorageWriter};
use std::{collections::HashMap, sync::Arc};
use tokio::{sync::mpsc, task::JoinSet, time::Duration};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::actors::{
    ChainProcessorActor, ManagedNodeActor, MetricWorker, SupervisorActor, SupervisorRpcActor,
};

/// The main service structure for the Kona
/// [`SupervisorService`](`kona_supervisor_core::SupervisorService`). Orchestrates the various
/// components of the supervisor.
#[derive(Debug)]
pub struct Service {
    config: Arc<Config>,

    database_factory: Arc<ChainDbFactory>,
    managed_nodes: HashMap<ChainId, Arc<ManagedNode<ChainDb, Client>>>,

    cancel_token: CancellationToken,

    join_set: JoinSet<Result<(), anyhow::Error>>,
}

impl Service {
    /// Creates a new Supervisor service instance.
    pub fn new(config: Config) -> Self {
        let database_factory = Arc::new(ChainDbFactory::new(config.datadir.clone()).with_metrics());

        Self {
            config: Arc::new(config),

            database_factory,
            managed_nodes: HashMap::new(),

            cancel_token: CancellationToken::new(),
            join_set: JoinSet::new(),
        }
    }

    /// Initialises the Supervisor service.
    pub async fn initialise(&mut self) -> Result<()> {
        let mut chain_event_receivers = HashMap::<ChainId, mpsc::Receiver<ChainEvent>>::new();
        let mut chain_event_senders = HashMap::<ChainId, mpsc::Sender<ChainEvent>>::new();
        let mut managed_node_receivers =
            HashMap::<ChainId, mpsc::Receiver<ManagedNodeCommand>>::new();
        let mut managed_node_senders = HashMap::<ChainId, mpsc::Sender<ManagedNodeCommand>>::new();

        // create sender and receiver channels for each chain
        for chain_id in self.config.rollup_config_set.rollups.keys() {
            let (chain_tx, chain_rx) = mpsc::channel::<ChainEvent>(1000);
            chain_event_senders.insert(*chain_id, chain_tx);
            chain_event_receivers.insert(*chain_id, chain_rx);

            let (managed_node_tx, managed_node_rx) = mpsc::channel::<ManagedNodeCommand>(1000);
            managed_node_senders.insert(*chain_id, managed_node_tx);
            managed_node_receivers.insert(*chain_id, managed_node_rx);
        }

        self.init_database().await?;
        self.init_managed_nodes(managed_node_receivers, &chain_event_senders).await?;
        self.init_chain_processor(chain_event_receivers, &managed_node_senders).await?;
        self.init_l1_watcher(&chain_event_senders)?;
        self.init_cross_safety_checker(&chain_event_senders).await?;

        // todo: run metric worker only if metrics are enabled
        self.init_rpc_server().await?;
        self.init_metric_reporter().await;
        Ok(())
    }

    async fn init_database(&self) -> Result<()> {
        info!(target: "supervisor::service", "Initialising databases for all chains...");

        for (chain_id, config) in self.config.rollup_config_set.rollups.iter() {
            // Initialise the database for each chain.
            let db = self.database_factory.get_or_create_db(*chain_id)?;
            let interop_time = config.interop_time;
            let derived_pair = config.genesis.get_derived_pair();
            if config.is_interop(derived_pair.derived.timestamp) {
                info!(target: "supervisor::service", chain_id, interop_time, %derived_pair, "Initialising database for interop activation block");
                db.initialise_log_storage(derived_pair.derived)?;
                db.initialise_derivation_storage(derived_pair)?;
            }
            info!(target: "supervisor::service", chain_id, "Database initialized successfully");
        }
        Ok(())
    }

    async fn init_managed_nodes(
        &mut self,
        mut managed_node_receivers: HashMap<ChainId, mpsc::Receiver<ManagedNodeCommand>>,
        chain_event_senders: &HashMap<ChainId, mpsc::Sender<ChainEvent>>,
    ) -> Result<()> {
        info!(target: "supervisor::service", "Initialising managed nodes for all chains...");

        for config in self.config.l2_consensus_nodes_config.iter() {
            let url = Url::parse(&self.config.l1_rpc).map_err(|err| {
                error!(target: "supervisor::service", %err, "Failed to parse L1 RPC URL");
                anyhow::anyhow!("failed to parse L1 RPC URL: {err}")
            })?;
            let provider = RootProvider::<Ethereum>::new_http(url);
            let client = Arc::new(Client::new(config.clone()));

            let chain_id = client.chain_id().await.map_err(|err| {
                error!(target: "supervisor::service", %err, "Failed to get chain ID from client");
                anyhow::anyhow!("failed to get chain ID from client: {err}")
            })?;
            let db = self.database_factory.get_db(chain_id)?;

            let chain_event_sender = chain_event_senders
                .get(&chain_id)
                .ok_or(anyhow::anyhow!("no chain event sender found for chain {chain_id}"))?
                .clone();

            let managed_node = ManagedNode::<ChainDb, Client>::new(
                client.clone(),
                db,
                provider,
                chain_event_sender,
            );

            if self.managed_nodes.contains_key(&chain_id) {
                warn!(target: "supervisor::service", %chain_id, "Managed node for chain already exists, skipping initialization");
                continue;
            }

            let managed_node = Arc::new(managed_node);
            self.managed_nodes.insert(chain_id, managed_node.clone());
            info!(target: "supervisor::service",
                 chain_id,
                "Managed node for chain initialized successfully",
            );

            // start managed node actor
            let managed_node_receiver = managed_node_receivers
                .remove(&chain_id)
                .ok_or(anyhow::anyhow!("no managed node receiver found for chain {chain_id}"))?;

            let cancel_token = self.cancel_token.clone();
            self.join_set.spawn(async move {
                if let Err(err) =
                    ManagedNodeActor::new(client, managed_node, managed_node_receiver, cancel_token)
                        .start()
                        .await
                {
                    Err(anyhow::anyhow!(err))
                } else {
                    Ok(())
                }
            });
        }
        Ok(())
    }

    async fn init_chain_processor(
        &mut self,
        mut chain_event_receivers: HashMap<ChainId, mpsc::Receiver<ChainEvent>>,
        managed_node_senders: &HashMap<ChainId, mpsc::Sender<ManagedNodeCommand>>,
    ) -> Result<()> {
        info!(target: "supervisor::service", "Initialising chain processors for all chains...");

        for (chain_id, _) in self.config.rollup_config_set.rollups.iter() {
            let db = self.database_factory.get_db(*chain_id)?;
            let managed_node = self
                .managed_nodes
                .get(chain_id)
                .ok_or(anyhow::anyhow!("no managed node found for chain {chain_id}"))?;

            let managed_node_sender = managed_node_senders
                .get(chain_id)
                .ok_or(anyhow::anyhow!("no managed node sender found for chain {chain_id}"))?
                .clone();

            // initialise chain processor for the chain.
            let mut processor = ChainProcessor::new(
                self.config.clone(),
                *chain_id,
                managed_node.clone(),
                db,
                managed_node_sender,
            );

            // todo: enable metrics only if configured
            processor = processor.with_metrics();

            // Start the chain processor actor.
            let chain_event_receiver = chain_event_receivers
                .remove(chain_id)
                .ok_or(anyhow::anyhow!("no chain event receiver found for chain {chain_id}"))?;

            let cancel_token = self.cancel_token.clone();
            self.join_set.spawn(async move {
                if let Err(err) =
                    ChainProcessorActor::new(processor, cancel_token, chain_event_receiver)
                        .start()
                        .await
                {
                    Err(anyhow::anyhow!(err))
                } else {
                    Ok(())
                }
            });
        }
        Ok(())
    }

    fn init_l1_watcher(
        &mut self,
        chain_event_senders: &HashMap<ChainId, mpsc::Sender<ChainEvent>>,
    ) -> Result<()> {
        info!(target: "supervisor::service", "Initialising L1 watcher...");

        let l1_rpc_url = Url::parse(&self.config.l1_rpc).map_err(|err| {
            error!(target: "supervisor::service", %err, "Failed to parse L1 RPC URL");
            anyhow::anyhow!("failed to parse L1 RPC URL: {err}")
        })?;
        let l1_rpc = RpcClient::new_http(l1_rpc_url);

        let chain_dbs_map: HashMap<ChainId, Arc<ChainDb>> = self
            .config
            .rollup_config_set
            .rollups
            .keys()
            .map(|chain_id| {
                self.database_factory.get_db(*chain_id)
                    .map(|db| (*chain_id, db)) // <-- FIX: remove Arc::new(db)
                    .map_err(|err| {
                        error!(target: "supervisor::service", %err, "Failed to get database for chain {chain_id}");
                        anyhow::anyhow!("failed to get database for chain {chain_id}: {err}")
                })
            })
            .collect::<Result<HashMap<ChainId, Arc<ChainDb>>>>()?;

        // Spawn a task that first performs a one-shot startup reorg across all chains,
        // (does nothing if the reorg is not detected) then starts the L1 watcher streaming loop.
        let database_factory = self.database_factory.clone();
        let cancel_token = self.cancel_token.clone();
        let event_senders = chain_event_senders.clone();
        let managed_nodes = self.managed_nodes.clone();
        self.join_set.spawn(async move {
            // Perform one-shot L1 consistency verification at startup to detect any
            // reorgs that occurred while the supervisor was offline, ensuring all
            // chains are in sync with the current canonical L1 state before processing.
            let reorg_handler =
                ReorgHandler::new(l1_rpc.clone(), chain_dbs_map.clone(), managed_nodes.clone())
                    .with_metrics();

            if let Err(err) = reorg_handler.verify_l1_consistency().await {
                warn!(target: "supervisor::service", %err, "Startup reorg check failed");
            } else {
                info!(target: "supervisor::service", "Startup reorg check completed");
            }

            // Start the L1 watcher streaming loop.
            let l1_watcher = L1Watcher::new(
                l1_rpc.clone(),
                database_factory,
                event_senders,
                cancel_token,
                reorg_handler,
            );

            l1_watcher.run().await;
            Ok(())
        });
        Ok(())
    }

    async fn init_cross_safety_checker(
        &mut self,
        chain_event_senders: &HashMap<ChainId, mpsc::Sender<ChainEvent>>,
    ) -> Result<()> {
        info!(target: "supervisor::service", "Initialising cross safety checker...");

        for (&chain_id, config) in &self.config.rollup_config_set.rollups {
            let db = Arc::clone(&self.database_factory);
            let cancel = self.cancel_token.clone();

            let chain_event_sender = chain_event_senders
                .get(&chain_id)
                .ok_or(anyhow::anyhow!("no chain event sender found for chain {chain_id}"))?
                .clone();

            let cross_safe_job = CrossSafetyCheckerJob::new(
                chain_id,
                db.clone(),
                cancel.clone(),
                Duration::from_secs(config.block_time),
                CrossSafePromoter,
                chain_event_sender.clone(),
                self.config.clone(),
            );

            self.join_set.spawn(async move {
                cross_safe_job.run().await;
                Ok(())
            });

            let cross_unsafe_job = CrossSafetyCheckerJob::new(
                chain_id,
                db,
                cancel,
                Duration::from_secs(config.block_time),
                CrossUnsafePromoter,
                chain_event_sender,
                self.config.clone(),
            );

            self.join_set.spawn(async move {
                cross_unsafe_job.run().await;
                Ok(())
            });
        }
        Ok(())
    }

    async fn init_metric_reporter(&mut self) {
        // Initialize the metric reporter actor.
        let database_factory = self.database_factory.clone();
        let cancel_token = self.cancel_token.clone();
        self.join_set.spawn(async move {
            if let Err(err) =
                MetricWorker::new(Duration::from_secs(30), vec![database_factory], cancel_token)
                    .start()
                    .await
            {
                Err(anyhow::anyhow!(err))
            } else {
                Ok(())
            }
        });
    }

    async fn init_rpc_server(&mut self) -> Result<()> {
        let supervisor = Arc::new(Supervisor::new(
            self.config.clone(),
            self.database_factory.clone(),
            self.managed_nodes.clone(),
        ));
        let rpc_addr = self.config.rpc_addr;
        let cancel_token = self.cancel_token.clone();
        self.join_set.spawn(async move {
            if let Err(err) =
                SupervisorRpcActor::new(rpc_addr, supervisor, cancel_token).start().await
            {
                Err(anyhow::anyhow!(err))
            } else {
                Ok(())
            }
        });
        Ok(())
    }

    /// Runs the Supervisor service.
    /// This function will typically run indefinitely until interrupted.
    pub async fn run(&mut self) -> Result<()> {
        self.initialise().await?;

        while let Some(res) = self.join_set.join_next().await {
            match res {
                Ok(Ok(_)) => {
                    info!(target: "supervisor::service", "Task completed successfully.");
                }
                Ok(Err(err)) => {
                    error!(target: "supervisor::service", %err, "A task encountered an error.");
                    // A task panicked, also trigger cancellation
                    self.cancel_token.cancel();
                    return Err(anyhow::anyhow!("A service task failed: {}", err));
                }
                Err(err) => {
                    error!(target: "supervisor::service", %err, "A task encountered an error.");
                    // A task panicked, also trigger cancellation
                    self.cancel_token.cancel();
                    return Err(anyhow::anyhow!("A service task failed: {}", err));
                }
            }
        }
        Ok(())
    }

    pub async fn shutdown(mut self) -> Result<()> {
        self.cancel_token.cancel(); // Signal cancellation to all tasks

        // Wait for all tasks to finish.
        while let Some(res) = self.join_set.join_next().await {
            match res {
                Ok(Ok(_)) => {
                    info!(target: "supervisor::service", "Task completed successfully during shutdown.");
                }
                Ok(Err(err)) => {
                    error!(target: "supervisor::service", %err, "A task encountered an error during shutdown.");
                }
                Err(err) => {
                    error!(target: "supervisor::service", %err, "A task encountered an error during shutdown.");
                }
            }
        }
        Ok(())
    }
}
