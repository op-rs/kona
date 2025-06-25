//! The [`EngineActor`].

use super::{EngineError, L2Finalizer};
use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use kona_derive::{ResetSignal, Signal};
use kona_engine::{
    ConsolidateTask, Engine, EngineClient, EngineQueries, EngineState, EngineTask, EngineTaskError,
    InsertUnsafeTask,
};
use kona_genesis::RollupConfig;
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use kona_sources::RuntimeConfig;
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::NodeActor;

/// The [`EngineActor`] is responsible for managing the operations sent to the execution layer's
/// Engine API. To accomplish this, it uses the [`Engine`] task queue to order Engine API
/// interactions based off of the [`Ord`] implementation of [`EngineTask`].
#[derive(Debug)]
pub struct EngineActor {
    /// The [`RollupConfig`] used to build tasks.
    config: Arc<RollupConfig>,
    /// An [`EngineClient`] used for creating engine tasks.
    client: Arc<EngineClient>,
    /// The [`Engine`] task queue.
    engine: Engine,
    /// The [`L2Finalizer`], used to finalize L2 blocks.
    finalizer: L2Finalizer,

    /// The channel to send the l2 safe head to the derivation actor.
    engine_l2_safe_head_tx: watch::Sender<L2BlockInfo>,
    /// A channel to send a signal that EL sync has completed. Informs the derivation actor to
    /// start. Because the EL sync state machine within [`EngineState`] can only complete once,
    /// this channel is consumed after the first successful send. Future cases where EL sync is
    /// re-triggered can occur, but we will not block derivation on it.
    sync_complete_tx: Option<oneshot::Sender<()>>,
    /// A way for the engine actor to send a [`Signal`] back to the derivation actor.
    derivation_signal_tx: mpsc::Sender<Signal>,

    /// Handler for inbound queries to the engine.
    inbound_queries: Option<mpsc::Receiver<EngineQueries>>,
    /// A channel to receive [`RuntimeConfig`] from the runtime actor.
    runtime_config_rx: mpsc::Receiver<RuntimeConfig>,
    /// A channel to receive [`OpAttributesWithParent`] from the derivation actor.
    attributes_rx: mpsc::Receiver<OpAttributesWithParent>,
    /// A channel to receive [`OpExecutionPayloadEnvelope`] from the network actor.
    unsafe_block_rx: mpsc::Receiver<OpExecutionPayloadEnvelope>,
    /// A channel to receive reset requests.
    reset_request_rx: mpsc::Receiver<()>,
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
        engine_l2_safe_head_tx: watch::Sender<L2BlockInfo>,
        sync_complete_tx: oneshot::Sender<()>,
        derivation_signal_tx: mpsc::Sender<Signal>,
        runtime_config_rx: mpsc::Receiver<RuntimeConfig>,
        attributes_rx: mpsc::Receiver<OpAttributesWithParent>,
        unsafe_block_rx: mpsc::Receiver<OpExecutionPayloadEnvelope>,
        reset_request_rx: mpsc::Receiver<()>,
        finalized_block_rx: watch::Receiver<Option<BlockInfo>>,
        inbound_queries: Option<mpsc::Receiver<EngineQueries>>,
        cancellation: CancellationToken,
    ) -> Self {
        let client = Arc::new(client);
        Self {
            config,
            client: Arc::clone(&client),
            engine,
            finalizer: L2Finalizer::new(finalized_block_rx, client),
            engine_l2_safe_head_tx,
            sync_complete_tx: Some(sync_complete_tx),
            derivation_signal_tx,
            runtime_config_rx,
            attributes_rx,
            unsafe_block_rx,
            reset_request_rx,
            inbound_queries,
            cancellation,
        }
    }

    /// Resets the inner [`Engine`] and propagates the reset to the derivation actor.
    pub async fn reset(&mut self) -> Result<(), EngineError> {
        // Reset the engine.
        let (l2_safe_head, l1_origin, system_config) =
            self.engine.reset(self.client.clone(), &self.config).await?;

        // Signal the derivation actor to reset.
        let signal = ResetSignal { l2_safe_head, l1_origin, system_config: Some(system_config) };
        match self.derivation_signal_tx.send(signal.signal()).await {
            Ok(_) => debug!(target: "engine", "Sent reset signal to derivation actor"),
            Err(err) => {
                error!(target: "engine", ?err, "Failed to send reset signal to the derivation actor");
                self.cancellation.cancel();
                return Err(EngineError::ChannelClosed);
            }
        }

        // Attempt to update the safe head following the reset.
        self.maybe_update_safe_head();

        // Clear the queue of L2 blocks awaiting finalization.
        self.finalizer.clear();

        Ok(())
    }

    /// Drains the inner [`Engine`] task queue and attempts to update the safe head.
    async fn drain(&mut self) -> Result<(), EngineError> {
        match self.engine.drain().await {
            Ok(_) => {
                trace!(target: "engine", "[ENGINE] tasks drained");
            }
            Err(EngineTaskError::Reset(err)) => {
                warn!(target: "engine", ?err, "Received reset request");
                self.reset().await?;
            }
            Err(EngineTaskError::Flush(err)) => {
                // This error is encountered when the payload is marked INVALID
                // by the engine api. Post-holocene, the payload is replaced by
                // a "deposits-only" block and re-executed. At the same time,
                // the channel and any remaining buffered batches are flushed.
                warn!(target: "engine", ?err, "Invalid payload, Flushing derivation pipeline.");
                match self.derivation_signal_tx.send(Signal::FlushChannel).await {
                    Ok(_) => {
                        debug!(target: "engine", "Sent flush signal to derivation actor")
                    }
                    Err(err) => {
                        error!(target: "engine", ?err, "Failed to send flush signal to the derivation actor.");
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
        self.check_el_sync().await?;

        Ok(())
    }

    /// Checks if the EL has finished syncing, notifying the derivation actor if it has.
    async fn check_el_sync(&mut self) -> Result<(), EngineError> {
        if self.engine.state().el_sync_finished {
            let Some(sender) = self.sync_complete_tx.take() else {
                return Ok(());
            };

            // If the sync status is finished, we can reset the engine and start derivation.
            info!(target: "engine", "Performing initial engine reset");
            self.reset().await?;
            sender.send(()).ok();
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

    async fn process(&mut self, msg: InboundEngineMessage) -> Result<(), EngineError> {
        match msg {
            InboundEngineMessage::ResetRequest => {
                warn!(target: "engine", "Received reset request");
                self.reset().await?;
            }
            InboundEngineMessage::UnsafeBlockReceived(envelope) => {
                let task = EngineTask::InsertUnsafe(InsertUnsafeTask::new(
                    Arc::clone(&self.client),
                    Arc::clone(&self.config),
                    *envelope,
                ));
                self.engine.enqueue(task);
            }
            InboundEngineMessage::DerivedAttributesReceived(attributes) => {
                self.finalizer.enqueue_for_finalization(&attributes);

                let task = EngineTask::Consolidate(ConsolidateTask::new(
                    Arc::clone(&self.client),
                    Arc::clone(&self.config),
                    *attributes,
                    true,
                ));
                self.engine.enqueue(task);
            }
            InboundEngineMessage::RuntimeConfigUpdate(config) => {
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
            InboundEngineMessage::NewFinalizedL1Block => {
                // Attempt to finalize any L2 blocks that are contained within the finalized L1
                // chain.
                self.finalizer.try_finalize_next(&mut self.engine).await;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl NodeActor for EngineActor {
    type Error = EngineError;

    async fn start(mut self) -> Result<(), Self::Error> {
        // Start the engine query server in a separate task to avoid blocking the main task.
        let handle = std::mem::take(&mut self.inbound_queries)
            .map(|inbound_query_channel| self.start_query_task(inbound_query_channel));

        loop {
            // Attempt to drain all outstanding tasks from the engine queue before adding new ones.
            self.drain().await?;

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
                    if reset.is_none() {
                        error!(target: "engine", "Reset request receiver closed unexpectedly");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    }
                    self.process(InboundEngineMessage::ResetRequest).await?;
                }
                unsafe_block = self.unsafe_block_rx.recv() => {
                    let Some(envelope) = unsafe_block else {
                        error!(target: "engine", "Unsafe block receiver closed unexpectedly");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    self.process(InboundEngineMessage::UnsafeBlockReceived(envelope.into())).await?;
                }
                attributes = self.attributes_rx.recv() => {
                    let Some(attributes) = attributes else {
                        error!(target: "engine", "Attributes receiver closed unexpectedly");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    self.process(InboundEngineMessage::DerivedAttributesReceived(attributes.into())).await?;
                }
                config = self.runtime_config_rx.recv() => {
                    let Some(config) = config else {
                        error!(target: "engine", "Runtime config receiver closed unexpectedly");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    self.process(InboundEngineMessage::RuntimeConfigUpdate(config.into())).await?;
                }
                msg = self.finalizer.new_finalized_block() => {
                    if let Err(err) = msg {
                        error!(target: "engine", ?err, "L1 finalized block receiver closed unexpectedly");
                        self.cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    }
                    self.process(InboundEngineMessage::NewFinalizedL1Block).await?;
                }
            }
        }
    }
}

/// An event that is received by the [`EngineActor`] for processing.
#[derive(Debug)]
pub enum InboundEngineMessage {
    /// Engine reset requested.
    ResetRequest,
    /// Received a new unsafe [`OpExecutionPayloadEnvelope`].
    UnsafeBlockReceived(Box<OpExecutionPayloadEnvelope>),
    /// Received new derived [`OpAttributesWithParent`].
    DerivedAttributesReceived(Box<OpAttributesWithParent>),
    /// Received an update to the runtime configuration.
    RuntimeConfigUpdate(Box<RuntimeConfig>),
    /// Received a new finalized L1 block
    NewFinalizedL1Block,
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
