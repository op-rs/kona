//! The Engine Actor

use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use kona_derive::types::{ResetSignal, Signal};
use kona_engine::{
    ConsolidateTask, Engine, EngineClient, EngineQueries, EngineResetError, EngineState,
    EngineTask, EngineTaskError, FinalizeTask, InsertUnsafeTask,
};
use kona_genesis::RollupConfig;
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use kona_sources::RuntimeConfig;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::OpNetworkPayloadEnvelope;
use std::{collections::BTreeMap, sync::Arc};
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
    /// A channel to receive reset requests.
    reset_request_rx: UnboundedReceiver<()>,
    /// A channel to receive finalized block updates.
    finalized_block_rx: UnboundedReceiver<BlockInfo>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,

    /// A map of L1 block number -> highest derived L2 block number within the L1 epoch, used to
    /// track derived attributes awaiting finalization. When a new finalized L1 block is
    /// received, the highest L2 block whose inputs are contained within the finalized L1 chain
    /// is finalized.
    awaiting_finalization: BTreeMap<u64, u64>,
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
        reset_request_rx: UnboundedReceiver<()>,
        finalized_block_rx: UnboundedReceiver<BlockInfo>,
        inbound_queries: Option<Receiver<EngineQueries>>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            config,
            client: Arc::new(client),
            engine,
            engine_l2_safe_head_tx,
            sync_complete_tx,
            derivation_signal_tx,
            runtime_config_rx,
            attributes_rx,
            unsafe_block_rx,
            reset_request_rx,
            finalized_block_rx,
            inbound_queries,
            cancellation,
            awaiting_finalization: BTreeMap::new(),
        }
    }

    /// Resets the inner [`Engine`] and propagates the reset to the derivation actor.
    pub async fn reset(&mut self) -> Result<(), EngineError> {
        // Reset the engine.
        let (l2_safe_head, l1_origin, system_config) =
            self.engine.reset(self.client.clone(), &self.config).await?;

        // Signal the derivation actor to reset.
        let signal = ResetSignal { l2_safe_head, l1_origin, system_config: Some(system_config) };
        match self.derivation_signal_tx.send(signal.signal()) {
            Ok(_) => debug!(target: "engine", "Sent reset signal to derivation actor"),
            Err(e) => {
                error!(target: "engine", ?e, "Failed to send reset signal to the derivation actor");
                self.cancellation.cancel();
                return Err(EngineError::ChannelClosed);
            }
        }

        // Clear the queue of attributes awaiting finalization. It will be re-saturated following
        // derivation.
        self.awaiting_finalization.clear();

        // Attempt to update the safe head following the reset.
        self.maybe_update_safe_head();

        Ok(())
    }

    /// Checks if the engine is syncing, notifying the derivation actor if necessary.
    async fn check_sync(&mut self) -> Result<(), EngineError> {
        // If the channel is closed, the receiver already marked engine ready.
        if self.sync_complete_tx.is_closed() {
            return Ok(());
        }

        if self.engine.state().el_sync_finished {
            // If the sync status is finished, we can reset the engine and start derivation.
            info!(target: "engine", "Performing initial engine reset");
            self.reset().await?;
            self.sync_complete_tx.send(()).ok();
        }

        Ok(())
    }

    /// Attempts to update the safe head via the watch channel.
    fn maybe_update_safe_head(&self) {
        let state_safe_head = self.engine.state().safe_head();
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

#[async_trait]
impl NodeActor for EngineActor {
    type InboundEvent = ();
    type Error = EngineError;

    async fn start(mut self) -> Result<(), Self::Error> {
        // Start the engine query server in a separate task to avoid blocking the main task.
        let handle = std::mem::take(&mut self.inbound_queries)
            .map(|inbound_query_channel| self.start_query_task(inbound_query_channel));

        loop {
            match self.engine.drain().await {
                Ok(_) => {
                    trace!(target: "engine", "[ENGINE] tasks drained");
                }
                Err(EngineTaskError::Reset(e)) => {
                    warn!(target: "engine", err = ?e, "Received reset request");
                    self.reset().await?;
                }
                Err(EngineTaskError::Flush(e)) => {
                    // This error is encountered when the payload is marked INVALID
                    // by the engine api. Post-holocene, the payload is replaced by
                    // a "deposits-only" block and re-executed. At the same time,
                    // the channel and any remaining buffered batches are flushed.
                    warn!(target: "engine", err = ?e, "[HOLOCENE] Invalid payload, Flushing derivation pipeline.");
                    match self.derivation_signal_tx.send(Signal::FlushChannel) {
                        Ok(_) => {
                            debug!(target: "engine", "[SENT] flush signal to derivation actor")
                        }
                        Err(e) => {
                            error!(target: "engine", ?e, "[ENGINE] Failed to send flush signal to the derivation actor.");
                            self.cancellation.cancel();
                            return Err(EngineError::ChannelClosed);
                        }
                    }
                }
                Err(err @ EngineTaskError::Critical(_)) => {
                    error!(target: "engine", ?err, "Critical error draining engine tasks");
                    self.cancellation.cancel();
                    return Err(err.into());
                }
                Err(EngineTaskError::Temporary(err)) => {
                    trace!(target: "engine", ?err, "Temporary error draining engine tasks");
                }
            }

            self.maybe_update_safe_head();

            tokio::select! {
                biased;

                _ = self.cancellation.cancelled() => {
                    warn!(target: "engine", "EngineActor received shutdown signal.");

                    if let Some(handle) = handle {
                        warn!(target: "engine", "Shutting down engine query task.");
                        handle.abort();
                    }

                    return Ok(());
                }
                reset = self.reset_request_rx.recv() => {
                    let Some(_) = reset else {
                        error!(target: "engine", "Reset request receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };

                    warn!(target: "engine", "Received reset request");
                    self.reset().await?;
                }
                unsafe_block = self.unsafe_block_rx.recv() => {
                    let Some(envelope) = unsafe_block else {
                        error!(target: "engine", "Unsafe block receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    let hash = envelope.payload_hash;
                    let task = EngineTask::InsertUnsafe(InsertUnsafeTask::new(
                        Arc::clone(&self.client),
                        Arc::clone(&self.config),
                        envelope,
                    ));
                    self.engine.enqueue(task);
                    debug!(target: "engine", ?hash, "Enqueued unsafe block task.");
                    self.check_sync().await?;
                }
                attributes = self.attributes_rx.recv() => {
                    let Some(attributes) = attributes else {
                        error!(target: "engine", "Attributes receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };

                    // Optimistically enqueue attributes for finalization.
                    self.awaiting_finalization
                        .entry(attributes.l1_origin.number)
                        .and_modify(|n| *n = (*n).max(attributes.parent.block_info.number + 1))
                        .or_insert(attributes.parent.block_info.number + 1);

                    let task = EngineTask::Consolidate(ConsolidateTask::new(
                        Arc::clone(&self.client),
                        Arc::clone(&self.config),
                        attributes,
                        true,
                    ));
                    self.engine.enqueue(task);
                    debug!(target: "engine", "Enqueued attributes consolidation task.");
                }
                config = self.runtime_config_rx.recv() => {
                    let Some(config) = config else {
                        error!(target: "engine", "Runtime config receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };

                    let client = Arc::clone(&self.client);
                    tokio::task::spawn(async move {
                        debug!(target: "engine", config = ?config, "Received runtime config");
                        let recommended = config.recommended_protocol_version;
                        let required = config.required_protocol_version;
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
                new_finalized_l1 = self.finalized_block_rx.recv() => {
                    let Some(new_finalized_l1) = new_finalized_l1 else {
                        error!(target: "engine", "Finalized block receiver closed unexpectedly, exiting node");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };

                    // Find the highest safe L2 block that's contained in the finalized chain, that we know about.
                    let highest_safe = self.awaiting_finalization.range(..=new_finalized_l1.number).next_back();

                    if let Some((_, highest_safe_number)) = highest_safe {
                        // Enqueue a finalize task.
                        let task = EngineTask::Finalize(FinalizeTask::new(self.client.clone(), *highest_safe_number));
                        self.engine.enqueue(task);

                        // Drain the map of all L2 blocks that were derived prior or within the finalized L1 block.
                        self.awaiting_finalization.retain(|&number, _| number > new_finalized_l1.number);
                    }
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
    /// Engine reset error.
    #[error(transparent)]
    EngineReset(#[from] EngineResetError),
    /// Engine task error.
    #[error(transparent)]
    EngineTask(#[from] EngineTaskError),
}

/// Configuration for the Engine Actor.
#[derive(Debug, Clone)]
pub struct EngineLauncher {
    /// The [`RollupConfig`].
    pub config: Arc<RollupConfig>,
    /// The engine rpc url.
    pub engine_url: Url,
    /// The L2 rpc url.
    pub l2_rpc_url: Url,
    /// The L1 rpc url.
    pub l1_rpc_url: Url,
    /// The engine jwt secret.
    pub jwt_secret: JwtSecret,
}

impl EngineLauncher {
    /// Launches the [`Engine`]. Returns the [`Engine`] and a channel to receive engine state
    /// updates.
    pub fn launch(self) -> Engine {
        let state = EngineState::default();
        let (engine_state_send, _) = tokio::sync::watch::channel(state);
        Engine::new(state, engine_state_send)
    }

    /// Returns the [`EngineClient`].
    pub fn client(&self) -> EngineClient {
        EngineClient::new_http(
            self.engine_url.clone(),
            self.l2_rpc_url.clone(),
            self.l1_rpc_url.clone(),
            self.config.clone(),
            self.jwt_secret,
        )
    }
}
