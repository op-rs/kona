//! The [`EngineActor`].

use super::{EngineError, L2Finalizer};
use alloy_rpc_types_engine::JwtSecret;
use async_trait::async_trait;
use futures::future::OptionFuture;
use kona_derive::{ResetSignal, Signal};
use kona_engine::{
    BuildTask, ConsolidateTask, Engine, EngineClient, EngineQueries,
    EngineState as InnerEngineState, EngineTask, EngineTaskError, InsertUnsafeTask,
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
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};
use url::Url;

use crate::{NodeActor, NodeMode, actors::CancellableContext};

/// The [`EngineActor`] is responsible for managing the operations sent to the execution layer's
/// Engine API. To accomplish this, it uses the [`Engine`] task queue to order Engine API
/// interactions based off of the [`Ord`] implementation of [`EngineTask`].
#[derive(Debug)]
pub struct EngineActor {
    /// The [`EngineActorState`] used to build the actor.
    builder: EngineBuilder,
    /// A channel to receive [`OpAttributesWithParent`] from the derivation actor.
    attributes_rx: mpsc::Receiver<OpAttributesWithParent>,
    /// A channel to receive [`OpExecutionPayloadEnvelope`] from the network actor.
    unsafe_block_rx: mpsc::Receiver<OpExecutionPayloadEnvelope>,
    /// A channel to receive reset requests.
    reset_request_rx: mpsc::Receiver<()>,
    /// Handler for inbound queries to the engine.
    inbound_queries: mpsc::Receiver<EngineQueries>,
    /// A channel to receive [`RuntimeConfig`] from the runtime actor.
    runtime_config_rx: mpsc::Receiver<RuntimeConfig>,
    /// A channel to receive build requests from the sequencer actor.
    ///
    /// ## Note
    /// This is `Some` when the node is in sequencer mode, and `None` when the node is in validator
    /// mode.
    build_request_rx:
        Option<mpsc::Receiver<(OpAttributesWithParent, mpsc::Sender<OpExecutionPayloadEnvelope>)>>,
    /// The [`L2Finalizer`], used to finalize L2 blocks.
    finalizer: L2Finalizer,
}

/// The outbound data for the [`EngineActor`].
#[derive(Debug)]
pub struct EngineInboundData {
    /// The channel used by the sequencer actor to send build requests to the engine actor.
    ///
    /// ## Note
    /// This is `Some` when the node is in sequencer mode, and `None` when the node is in validator
    /// mode.
    pub build_request_tx:
        Option<mpsc::Sender<(OpAttributesWithParent, mpsc::Sender<OpExecutionPayloadEnvelope>)>>,
    /// A channel to send [`OpAttributesWithParent`] to the engine actor.
    pub attributes_tx: mpsc::Sender<OpAttributesWithParent>,
    /// A channel to send [`OpExecutionPayloadEnvelope`] to the engine actor.
    ///
    /// ## Note
    /// The sequencer actor should not need to send [`OpExecutionPayloadEnvelope`]s to the engine
    /// actor through that channel. Instead, it should use the `build_request_tx` channel to
    /// trigger [`BuildTask`] tasks which should insert the block newly built to the engine
    /// state upon completion.
    pub unsafe_block_tx: mpsc::Sender<OpExecutionPayloadEnvelope>,
    /// A channel to send reset requests.
    pub reset_request_tx: mpsc::Sender<()>,
    /// Handler to send inbound queries to the engine.
    pub inbound_queries_tx: mpsc::Sender<EngineQueries>,
    /// A channel to send [`RuntimeConfig`] to the engine actor.
    pub runtime_config_tx: mpsc::Sender<RuntimeConfig>,
    /// A channel that sends new finalized L1 blocks intermittently.
    pub finalized_l1_block_tx: watch::Sender<Option<BlockInfo>>,
}

/// Configuration for the Engine Actor.
#[derive(Debug, Clone)]
pub struct EngineBuilder {
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
    /// The mode of operation for the node.
    /// When the node is in sequencer mode, the engine actor will receive requests to build blocks
    /// from the sequencer actor.
    pub mode: NodeMode,
}

impl EngineBuilder {
    /// Launches the [`Engine`]. Returns the [`Engine`] and a channel to receive engine state
    /// updates.
    fn build_state(self) -> EngineActorState {
        let client = self.client();
        let state = InnerEngineState::default();
        let (engine_state_send, _) = tokio::sync::watch::channel(state);
        EngineActorState {
            rollup: self.config,
            client,
            engine: Engine::new(state, engine_state_send),
        }
    }

    /// Returns the [`EngineClient`].
    pub fn client(&self) -> Arc<EngineClient> {
        EngineClient::new_http(
            self.engine_url.clone(),
            self.l2_rpc_url.clone(),
            self.l1_rpc_url.clone(),
            self.config.clone(),
            self.jwt_secret,
        )
        .into()
    }
}

/// The configuration for the [`EngineActor`].
#[derive(Debug)]
pub(super) struct EngineActorState {
    /// The [`RollupConfig`] used to build tasks.
    pub(super) rollup: Arc<RollupConfig>,
    /// An [`EngineClient`] used for creating engine tasks.
    pub(super) client: Arc<EngineClient>,
    /// The [`Engine`] task queue.
    pub(super) engine: Engine,
}

/// The communication context used by the engine actor.
#[derive(Debug)]
pub struct EngineContext {
    /// The cancellation token, shared between all tasks.
    pub cancellation: CancellationToken,
    /// A sender for L2 unsafe head update notifications.
    /// Is optional because it is only used in sequencer mode.
    pub engine_unsafe_head_tx: Option<watch::Sender<L2BlockInfo>>,
    /// The sender for L2 safe head update notifications.
    pub engine_l2_safe_head_tx: watch::Sender<L2BlockInfo>,
    /// A channel to send a signal that EL sync has completed. Informs the derivation actor to
    /// start. Because the EL sync state machine within [`InnerEngineState`] can only complete
    /// once, this channel is consumed after the first successful send. Future cases where EL
    /// sync is re-triggered can occur, but we will not block derivation on it.
    pub sync_complete_tx: oneshot::Sender<()>,
    /// A way for the engine actor to send a [`Signal`] back to the derivation actor.
    pub derivation_signal_tx: mpsc::Sender<Signal>,
}

impl CancellableContext for EngineContext {
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}

impl EngineActor {
    /// Constructs a new [`EngineActor`] from the params.
    pub fn new(config: EngineBuilder) -> (EngineInboundData, Self) {
        let (finalized_l1_block_tx, finalized_l1_block_rx) = watch::channel(None);
        let (inbound_queries_tx, inbound_queries_rx) = mpsc::channel(1024);
        let (runtime_config_tx, runtime_config_rx) = mpsc::channel(1024);
        let (attributes_tx, attributes_rx) = mpsc::channel(1024);
        let (unsafe_block_tx, unsafe_block_rx) = mpsc::channel(1024);
        let (reset_request_tx, reset_request_rx) = mpsc::channel(1024);

        let (build_request_tx, build_request_rx) = if config.mode.is_sequencer() {
            let (tx, rx) = mpsc::channel(1024);
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };

        let actor = Self {
            builder: config,
            attributes_rx,
            unsafe_block_rx,
            reset_request_rx,
            inbound_queries: inbound_queries_rx,
            runtime_config_rx,
            build_request_rx,
            finalizer: L2Finalizer::new(finalized_l1_block_rx),
        };

        let outbound_data = EngineInboundData {
            build_request_tx,
            finalized_l1_block_tx,
            inbound_queries_tx,
            runtime_config_tx,
            attributes_tx,
            unsafe_block_tx,
            reset_request_tx,
        };

        (outbound_data, actor)
    }
}

impl EngineActorState {
    /// Starts a task to handle engine queries.
    fn start_query_task(
        &self,
        mut inbound_query_channel: tokio::sync::mpsc::Receiver<EngineQueries>,
    ) -> JoinHandle<()> {
        let state_recv = self.engine.subscribe();
        let engine_client = self.client.clone();
        let rollup_config = self.rollup.clone();

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

    /// Resets the inner [`Engine`] and propagates the reset to the derivation actor.
    pub(super) async fn reset(
        &mut self,
        derivation_signal_tx: &mpsc::Sender<Signal>,
        engine_l2_safe_head_tx: &watch::Sender<L2BlockInfo>,
        finalizer: &mut L2Finalizer,
    ) -> Result<(), EngineError> {
        // Reset the engine.
        let (l2_safe_head, l1_origin, system_config) =
            self.engine.reset(self.client.clone(), self.rollup.clone()).await?;

        // Signal the derivation actor to reset.
        let signal = ResetSignal { l2_safe_head, l1_origin, system_config: Some(system_config) };
        match derivation_signal_tx.send(signal.signal()).await {
            Ok(_) => debug!(target: "engine", "Sent reset signal to derivation actor"),
            Err(err) => {
                error!(target: "engine", ?err, "Failed to send reset signal to the derivation actor");
                return Err(EngineError::ChannelClosed);
            }
        }

        // Attempt to update the safe head following the reset.
        self.maybe_update_safe_head(engine_l2_safe_head_tx);

        // Clear the queue of L2 blocks awaiting finalization.
        finalizer.clear();

        Ok(())
    }

    /// Drains the inner [`Engine`] task queue and attempts to update the safe head.
    async fn drain(
        &mut self,
        derivation_signal_tx: &mpsc::Sender<Signal>,
        sync_complete_tx: &mut Option<oneshot::Sender<()>>,
        engine_l2_safe_head_tx: &watch::Sender<L2BlockInfo>,
        finalizer: &mut L2Finalizer,
    ) -> Result<(), EngineError> {
        match self.engine.drain().await {
            Ok(_) => {
                trace!(target: "engine", "[ENGINE] tasks drained");
            }
            Err(EngineTaskError::Reset(err)) => {
                warn!(target: "engine", ?err, "Received reset request");
                self.reset(derivation_signal_tx, engine_l2_safe_head_tx, finalizer).await?;
            }
            Err(EngineTaskError::Flush(err)) => {
                // This error is encountered when the payload is marked INVALID
                // by the engine api. Post-holocene, the payload is replaced by
                // a "deposits-only" block and re-executed. At the same time,
                // the channel and any remaining buffered batches are flushed.
                warn!(target: "engine", ?err, "Invalid payload, Flushing derivation pipeline.");
                match derivation_signal_tx.send(Signal::FlushChannel).await {
                    Ok(_) => {
                        debug!(target: "engine", "Sent flush signal to derivation actor")
                    }
                    Err(err) => {
                        error!(target: "engine", ?err, "Failed to send flush signal to the derivation actor.");
                        return Err(EngineError::ChannelClosed);
                    }
                }
            }
            Err(err @ EngineTaskError::Critical(_)) => {
                error!(target: "engine", ?err, "Critical error draining engine tasks");
                return Err(err.into());
            }
            Err(EngineTaskError::Temporary(err)) => {
                trace!(target: "engine", ?err, "Temporary error draining engine tasks");
            }
        }

        self.maybe_update_safe_head(engine_l2_safe_head_tx);
        self.check_el_sync(
            derivation_signal_tx,
            engine_l2_safe_head_tx,
            sync_complete_tx,
            finalizer,
        )
        .await?;

        Ok(())
    }

    /// Checks if the EL has finished syncing, notifying the derivation actor if it has.
    async fn check_el_sync(
        &mut self,
        derivation_signal_tx: &mpsc::Sender<Signal>,
        engine_l2_safe_head_tx: &watch::Sender<L2BlockInfo>,
        sync_complete_tx: &mut Option<oneshot::Sender<()>>,
        finalizer: &mut L2Finalizer,
    ) -> Result<(), EngineError> {
        if self.engine.state().el_sync_finished {
            let Some(sync_complete_tx) = std::mem::take(sync_complete_tx) else {
                return Ok(());
            };

            // If the sync status is finished, we can reset the engine and start derivation.
            info!(target: "engine", "Performing initial engine reset");
            self.reset(derivation_signal_tx, engine_l2_safe_head_tx, finalizer).await?;
            sync_complete_tx.send(()).ok();
        }

        Ok(())
    }

    /// Attempts to update the safe head via the watch channel.
    fn maybe_update_safe_head(&self, engine_l2_safe_head_tx: &watch::Sender<L2BlockInfo>) {
        let state_safe_head = self.engine.state().sync_state.safe_head();
        let update = |head: &mut L2BlockInfo| {
            if head != &state_safe_head {
                *head = state_safe_head;
                return true;
            }
            false
        };
        let sent = engine_l2_safe_head_tx.send_if_modified(update);
        trace!(target: "engine", ?sent, "Attempted L2 Safe Head Update");
    }

    fn runtime_config_update(&mut self, config: RuntimeConfig) {
        let client = self.client.clone();
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
}

#[async_trait]
impl NodeActor for EngineActor {
    type Error = EngineError;
    type OutboundData = EngineContext;
    type InboundData = EngineInboundData;
    type Builder = EngineBuilder;

    fn build(config: Self::Builder) -> (Self::InboundData, Self) {
        Self::new(config)
    }

    async fn start(
        mut self,
        EngineContext {
            cancellation,
            engine_l2_safe_head_tx,
            sync_complete_tx,
            derivation_signal_tx,
            mut engine_unsafe_head_tx,
        }: Self::OutboundData,
    ) -> Result<(), Self::Error> {
        let mut state = self.builder.build_state();

        // Start the engine query server in a separate task to avoid blocking the main task.
        let handle = state.start_query_task(self.inbound_queries);

        // The sync complete tx is consumed after the first successful send. Hence we need to wrap
        // it in an `Option` to ensure we satisfy the borrow checker.
        let mut sync_complete_tx = Some(sync_complete_tx);

        loop {
            // Attempt to drain all outstanding tasks from the engine queue before adding new ones.
            state
                .drain(
                    &derivation_signal_tx,
                    &mut sync_complete_tx,
                    &engine_l2_safe_head_tx,
                    &mut self.finalizer,
                )
                .await?;

            // If the unsafe head has updated, propagate it to the outbound channels.
            if let Some(unsafe_head_tx) = engine_unsafe_head_tx.as_mut() {
                unsafe_head_tx.send_if_modified(|val| {
                    let new_head = state.engine.state().sync_state.unsafe_head();
                    (*val != new_head).then(|| *val = new_head).is_some()
                });
            }

            tokio::select! {
                biased;

                _ = cancellation.cancelled() => {
                    warn!(target: "engine", "EngineActor received shutdown signal. Aborting engine query task.");

                    handle.abort();

                    return Ok(());
                }
                reset = self.reset_request_rx.recv() => {
                    if reset.is_none() {
                        error!(target: "engine", "Reset request receiver closed unexpectedly");
                        cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    }
                    warn!(target: "engine", "Received reset request");
                    state
                        .reset(&derivation_signal_tx, &engine_l2_safe_head_tx, &mut self.finalizer)
                        .await?;
                }
                Some(res) = OptionFuture::from(self.build_request_rx.as_mut().map(|rx| rx.recv())), if self.build_request_rx.is_some() => {
                    let Some((attributes, response_tx)) = res else {
                        error!(target: "engine", "Build request receiver closed unexpectedly while in sequencer mode");
                        cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };

                    let task = EngineTask::BuildBlock(BuildTask::new(
                        state.client.clone(),
                        state.rollup.clone(),
                        attributes,
                        // The payload is not derived in this case.
                        false,
                        Some(response_tx),
                    ));
                    state.engine.enqueue(task);
                }
                unsafe_block = self.unsafe_block_rx.recv() => {
                    let Some(envelope) = unsafe_block else {
                        error!(target: "engine", "Unsafe block receiver closed unexpectedly");
                        cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    let task = EngineTask::InsertUnsafe(InsertUnsafeTask::new(
                        state.client.clone(),
                        state.rollup.clone(),
                        envelope,
                    ));
                    state.engine.enqueue(task);
                }
                attributes = self.attributes_rx.recv() => {
                    let Some(attributes) = attributes else {
                        error!(target: "engine", "Attributes receiver closed unexpectedly");
                        cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };
                    self.finalizer.enqueue_for_finalization(&attributes);

                    let task = EngineTask::Consolidate(ConsolidateTask::new(
                        state.client.clone(),
                        state.rollup.clone(),
                        attributes,
                        true,
                    ));
                    state.engine.enqueue(task);
                }
                // Since the runtime actor is optional, we need to check if the channel is closed to ensure we're not immediately resolving the future.
                Some(config) = self.runtime_config_rx.recv(), if !self.runtime_config_rx.is_closed() => {
                    state.runtime_config_update(config);
                }
                msg = self.finalizer.new_finalized_block() => {
                    if let Err(err) = msg {
                        error!(target: "engine", ?err, "L1 finalized block receiver closed unexpectedly");
                        cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    }
                    // Attempt to finalize any L2 blocks that are contained within the finalized L1
                    // chain.
                    self.finalizer.try_finalize_next(&mut state).await;
                }
            }
        }
    }
}
