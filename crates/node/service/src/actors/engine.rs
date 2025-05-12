//! The Engine Actor

use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use kona_derive::types::Signal;
use kona_engine::{
    ConsolidateTask, Engine, EngineClient, EngineQueries, EngineStateBuilder,
    EngineStateBuilderError, EngineTask, EngineTaskError, InsertUnsafeTask,
};
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OpAttributesWithParent};
use kona_sources::RuntimeConfig;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::sync::Arc;
use tokio::{
    sync::{
        mpsc::{Receiver, UnboundedReceiver, UnboundedSender},
        watch::Sender as WatchSender,
    },
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::NodeActor;

/// The [`EngineActor`] for the engine api sub-routine.
///
/// The engine actor is essentially just a wrapper over two things.
/// - [`kona_engine::EngineState`]
/// - The Engine API
#[derive(Debug)]
pub struct EngineActor {
    /// The [`RollupConfig`] used to build tasks.
    pub config: Arc<RollupConfig>,
    /// An [`EngineClient`] used for creating engine tasks.
    pub client: Arc<EngineClient>,
    /// The [`Engine`].
    pub engine: Engine,
    /// The channel to send the l2 safe head to the derivation actor.
    engine_l2_safe_head_tx: WatchSender<L2BlockInfo>,
    /// Handler for inbound queries to the engine.
    inbound_queries: Option<tokio::sync::mpsc::Receiver<EngineQueries>>,
    /// A channel to send a signal that syncing is complete.
    /// Informs the derivation actor to start.
    sync_complete_tx: UnboundedSender<()>,
    /// A way for the engine actor to signal back to the derivation actor
    /// if a block building task produced an `INVALID` response.
    derivation_signal_tx: UnboundedSender<Signal>,
    /// A channel to receive [`RuntimeConfig`] from the runtime actor.
    runtime_config_rx: UnboundedReceiver<RuntimeConfig>,
    /// A channel to receive [`OpAttributesWithParent`] from the derivation actor.
    attributes_rx: UnboundedReceiver<OpAttributesWithParent>,
    /// A channel to receive [`OpNetworkPayloadEnvelope`] from the network actor.
    unsafe_block_rx: UnboundedReceiver<OpNetworkPayloadEnvelope>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl EngineActor {
    /// Constructs a new [`EngineActor`] from the params.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        config: Arc<RollupConfig>,
        client: EngineClient,
        engine: Engine,
        engine_l2_safe_head_tx: WatchSender<L2BlockInfo>,
        sync_complete_tx: UnboundedSender<()>,
        derivation_signal_tx: UnboundedSender<Signal>,
        runtime_config_rx: UnboundedReceiver<RuntimeConfig>,
        attributes_rx: UnboundedReceiver<OpAttributesWithParent>,
        unsafe_block_rx: UnboundedReceiver<OpNetworkPayloadEnvelope>,
        inbound_queries: Option<Receiver<EngineQueries>>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            config,
            client: Arc::new(client),
            sync_complete_tx,
            derivation_signal_tx,
            engine,
            engine_l2_safe_head_tx,
            runtime_config_rx,
            inbound_queries,
            attributes_rx,
            unsafe_block_rx,
            cancellation,
        }
    }

    /// Checks if the engine is syncing, notifying the derivation actor if necessary.
    pub fn check_sync(&self) {
        // If the channel is closed, the receiver already marked engine ready.
        if self.sync_complete_tx.is_closed() {
            return;
        }
        let client = Arc::clone(&self.client);
        let channel = self.sync_complete_tx.clone();
        tokio::task::spawn(async move {
            if let Ok(sync_status) = client.syncing().await {
                // If the sync status is not `None`, continue syncing.
                if !matches!(sync_status, alloy_rpc_types_eth::SyncStatus::None) {
                    trace!(target: "engine", ?sync_status, "SYNCING");
                    return;
                }
                // If the sync status is `None`, begin derivation.
                trace!(target: "engine", "Sending signal to start derivation");
                channel.send(()).ok();
            }
        });
    }

    /// Starts a task to handle engine queries.
    fn start_query_task(
        &self,
        mut inbound_query_channel: tokio::sync::mpsc::Receiver<EngineQueries>,
    ) -> JoinHandle<()> {
        let state_recv = self.engine.subscribe();
        let engine_client = self.client.clone();
        let rollup_config = self.config.clone();

        tokio::spawn(async move {
            while let Some(req) = inbound_query_channel.recv().await {
                {
                    trace!(target: "engine", ?req, "Received engine query request.");

                    if let Err(e) = req.handle(&state_recv, &engine_client, &rollup_config).await {
                        warn!(target: "engine", err = ?e, "Failed to handle engine query request.");
                    }
                }
            }
        })
    }
}

/// Configuration for the Engine Actor.
#[derive(Debug, Clone)]
pub struct EngineLauncher {
    /// The [`RollupConfig`].
    pub config: Arc<RollupConfig>,
    /// The engine rpc url.
    pub engine_url: Url,
    /// The l2 rpc url.
    pub l2_rpc_url: Url,
    /// The engine jwt secret.
    pub jwt_secret: JwtSecret,
}

impl EngineLauncher {
    /// Launches the [`Engine`]. Returns the [`Engine`] and a channel to receive engine state
    /// updates.
    pub async fn launch(self) -> Result<Engine, EngineStateBuilderError> {
        let state = self.state_builder().build().await?;
        let (engine_state_send, _) = tokio::sync::watch::channel(state);

        Ok(Engine::new(state, engine_state_send))
    }

    /// Returns the [`EngineClient`].
    pub fn client(&self) -> EngineClient {
        EngineClient::new_http(
            self.engine_url.clone(),
            self.l2_rpc_url.clone(),
            self.config.clone(),
            self.jwt_secret,
        )
    }

    /// Returns an [`EngineStateBuilder`].
    pub fn state_builder(&self) -> EngineStateBuilder {
        EngineStateBuilder::new(self.client())
    }
}

#[async_trait]
impl NodeActor for EngineActor {
    type InboundEvent = ();
    type Error = EngineError;

    async fn start(mut self) -> Result<(), Self::Error> {
        // Start the engine query server in a separate task to avoid blocking the main task.
        let handle = std::mem::take(&mut self.inbound_queries)
            .map(|inbound_query_channel| self.start_query_task(inbound_query_channel));

        loop {
            tokio::select! {
                _ = self.cancellation.cancelled() => {
                    warn!(target: "engine", "EngineActor received shutdown signal.");

                    if let Some(handle) = handle {
                        warn!(target: "engine", "Shutting down engine query task.");
                        handle.abort();
                    }

                    return Ok(());
                }
                res = self.engine.drain() => {
                    match res {
                        Ok(_) => {
                          trace!(target: "engine", "[ENGINE] tasks drained");
                          // Update the l2 safe head if needed.
                          let state_safe_head = self.engine.safe_head();
                           let update = |head: &mut L2BlockInfo| {
                              if head != &state_safe_head {
                                  *head = state_safe_head;
                                  return true;
                              }
                              false
                          };
                          let sent = self.engine_l2_safe_head_tx.send_if_modified(update);
                          trace!(target: "engine", ?sent, "Attempted L2 Safe Head Update");
                        }
                        Err(EngineTaskError::Flush(e)) => {
                            // This error is encountered when the payload is marked INVALID
                            // by the engine api. Post-holocene, the payload is replaced by
                            // a "deposits-only" block and re-executed. At the same time,
                            // the channel and any remaining buffered batches are flushed.
                            warn!(target: "engine", ?e, "[HOLOCENE] Invalid payload, Flushing derivation pipeline.");
                            match self.derivation_signal_tx.send(Signal::FlushChannel) {
                                Ok(_) => debug!(target: "engine", "[SENT] flush signal to derivation actor"),
                                Err(e) => {
                                    error!(target: "engine", ?e, "[ENGINE] Failed to send flush signal to the derivation actor.");
                                    self.cancellation.cancel();
                                    return Err(EngineError::ChannelClosed);
                                }
                            }
                        }
                        Err(e) => warn!(target: "engine", ?e, "Error draining engine tasks"),
                    }
                }
                attributes = self.attributes_rx.recv() => {
                    let Some(attributes) = attributes else {
                        error!(target: "engine", "Attributes receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    let task = ConsolidateTask::new(
                        Arc::clone(&self.client),
                        Arc::clone(&self.config),
                        attributes,
                        true,
                    );
                    let task = EngineTask::Consolidate(task);
                    self.engine.enqueue(task);
                    debug!(target: "engine", "Enqueued attributes consolidation task.");
                }
                unsafe_block = self.unsafe_block_rx.recv() => {
                    let Some(envelope) = unsafe_block else {
                        error!(target: "engine", "Unsafe block receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    let hash = envelope.payload_hash;
                    let task = InsertUnsafeTask::new(
                        Arc::clone(&self.client),
                        Arc::clone(&self.config),
                        envelope,
                    );
                    let task = EngineTask::InsertUnsafe(task);
                    self.engine.enqueue(task);
                    debug!(target: "engine", ?hash, "Enqueued unsafe block task.");
                    self.check_sync();
                }
                Some(config) = self.runtime_config_rx.recv() => {
                    let client = Arc::clone(&self.client);
                    tokio::task::spawn(async move {
                        debug!(target: "engine", config = ?config, "Received runtime config");
                        let recommended: op_alloy_rpc_types_engine::ProtocolVersion = config.recommended_protocol_version.into();
                        let required: op_alloy_rpc_types_engine::ProtocolVersion = config.required_protocol_version.into();
                        match client.signal_superchain_v1(recommended, required).await {
                            Ok(v) => info!(target: "engine", ?v, "[SUPERCHAIN::SIGNAL]"),
                            Err(e) => {
                                // Since the `engine_signalSuperchainV1` endpoint is OPTIONAL,
                                // a warning is logged instead of an error.
                                warn!(target: "engine", ?e, "Failed to send superchain signal (OPTIONAL)");
                            }
                        }
                    });
                }
            }
        }
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// An error from the [`EngineActor`].
#[derive(thiserror::Error, Debug)]
pub enum EngineError {
    /// Closed channel error.
    #[error("closed channel error")]
    ChannelClosed,
}
