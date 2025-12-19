//! The [`EngineActor`].

use crate::{
    EngineClientError, EngineClientResult, EngineError, L2Finalizer, NodeActor, NodeMode,
    actors::CancellableContext,
};
use alloy_provider::RootProvider;
use alloy_rpc_types_engine::{JwtSecret, PayloadId};
use async_trait::async_trait;
use futures::FutureExt;
use kona_derive::{ResetSignal, Signal};
use kona_engine::{
    BuildTask, ConsolidateTask, Engine, EngineClient, EngineClientBuilder,
    EngineClientBuilderError, EngineQueries, EngineState as InnerEngineState, EngineTask,
    EngineTaskError, EngineTaskErrorSeverity, InsertTask, OpEngineClient, RollupBoostServer,
    RollupBoostServerArgs, SealTask, SealTaskError,
};
use kona_genesis::RollupConfig;
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use kona_rpc::{RollupBoostAdminQuery, RollupBoostHealthQuery};
use op_alloy_network::Optimism;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::{fmt::Debug, sync::Arc, time::Duration};
use tokio::{
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
use tokio_util::{
    future::FutureExt as _,
    sync::{CancellationToken, WaitForCancellationFuture},
};
use url::Url;

/// Inbound requests that the [`EngineActor`] can process.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EngineActorRequest {
    /// Request to build.
    BuildRequest(BuildRequest),
    /// Request to consolidate based on the provided attributes.
    ConsolidateRequest(OpAttributesWithParent),
    /// Request to insert the provided unsafe block.
    InsertUnsafeBlockRequest(OpExecutionPayloadEnvelope),
    /// Request to reset engine forkchoice.
    ResetRequest(ResetRequest),
    /// Request for the engine to process the provided RPC request.
    RpcRequest(EngineRpcRequest),
    /// Request to seal the block with the provided details.
    SealRequest(SealRequest),
}

/// RPC Request for the engine to handle.
#[derive(Debug)]
pub enum EngineRpcRequest {
    /// Engine RPC query.
    EngineQuery(EngineQueries),
    /// Rollup boost admin request.
    RollupBoostAdminRequest(RollupBoostAdminQuery),
    /// Rollup boost health request.
    RollupBoostHealthRequest(RollupBoostHealthQuery),
}

/// A request to build a payload.
/// Contains the attributes to build and a channel to send back the resulting `PayloadId`.
#[derive(Debug)]
pub struct BuildRequest {
    /// The [`OpAttributesWithParent`] from which the block build should be started.
    pub attributes: OpAttributesWithParent,
    /// The channel on which the result, successful or not, will be sent.
    pub result_tx: mpsc::Sender<PayloadId>,
}

/// A request to reset the engine forkchoice.
/// Optionally contains a channel to send back the response if the caller would like to know that
/// the request was successfully processed.
#[derive(Debug)]
pub struct ResetRequest {
    /// response will be sent to this channel, if `Some`.
    pub result_tx: Option<mpsc::Sender<EngineClientResult<()>>>,
}

/// A request to seal and canonicalize a payload.
/// Contains the `PayloadId`, attributes, and a channel to send back the result.
#[derive(Debug)]
pub struct SealRequest {
    /// The `PayloadId` to seal and canonicalize.
    pub payload_id: PayloadId,
    /// The attributes necessary for the seal operation.
    pub attributes: OpAttributesWithParent,
    /// The channel on which the result, successful or not, will be sent.
    pub result_tx: mpsc::Sender<Result<OpExecutionPayloadEnvelope, SealTaskError>>,
}

/// The [`EngineActor`] is responsible for managing the operations sent to the execution layer's
/// Engine API. To accomplish this, it uses the [`Engine`] task queue to order Engine API
/// interactions based off of the [`Ord`] implementation of [`EngineTask`].
#[derive(Debug)]
pub struct EngineActor {
    inbound_request_rx: mpsc::Receiver<EngineActorRequest>,
    /// The [`EngineConfig`] used to build the actor.
    builder: EngineConfig,
    /// The [`L2Finalizer`], used to finalize L2 blocks.
    finalizer: L2Finalizer,

    /// A channel to use to relay the current unsafe head.
    /// ## Note
    /// This is `Some` when the node is in sequencer mode, and `None` when the node is in validator
    /// mode.
    unsafe_head_tx: Option<watch::Sender<L2BlockInfo>>,
}

/// The outbound data for the [`EngineActor`].
#[derive(Debug)]
pub struct EngineInboundData {
    /// A channel that sends new finalized L1 blocks intermittently.
    pub finalized_l1_block_tx: watch::Sender<Option<BlockInfo>>,
    /// A channel that sends requests to the EngineActor.
    pub inbound_request_tx: mpsc::Sender<EngineActorRequest>,
    /// A receiver to use to view the latest unsafe head [`L2BlockInfo`] and await its changes.
    ///
    /// This is `Some` when the node is in sequencer mode, and `None` when the node is in validator
    /// mode.
    pub unsafe_head_rx: Option<watch::Receiver<L2BlockInfo>>,
}

/// Configuration for the Engine Actor.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// The [`RollupConfig`].
    pub config: Arc<RollupConfig>,

    /// Builder url.
    pub builder_url: Url,
    /// Builder jwt secret.
    pub builder_jwt_secret: JwtSecret,
    /// Builder timeout.
    pub builder_timeout: Duration,

    /// The engine rpc url.
    pub l2_url: Url,
    /// The engine jwt secret.
    pub l2_jwt_secret: JwtSecret,
    /// The l2 timeout.
    pub l2_timeout: Duration,

    /// The L1 rpc url.
    pub l1_url: Url,

    /// The mode of operation for the node.
    /// When the node is in sequencer mode, the engine actor will receive requests to build blocks
    /// from the sequencer actor.
    pub mode: NodeMode,

    /// The rollup boost arguments.
    pub rollup_boost: RollupBoostServerArgs,
}

impl EngineConfig {
    /// Launches the [`Engine`]. Returns the [`Engine`] and a channel to receive engine state
    /// updates.
    fn build_state(
        self,
    ) -> Result<
        EngineActorState<OpEngineClient<RootProvider, RootProvider<Optimism>>>,
        EngineClientBuilderError,
    > {
        let client = EngineClientBuilder {
            builder: self.builder_url.clone(),
            builder_jwt: self.builder_jwt_secret,
            builder_timeout: self.builder_timeout,
            l2: self.l2_url.clone(),
            l2_jwt: self.l2_jwt_secret,
            l2_timeout: self.l2_timeout,
            l1_rpc: self.l1_url.clone(),
            cfg: self.config.clone(),
            rollup_boost: self.rollup_boost.clone(),
        }
        .build()?
        .into();

        let state = InnerEngineState::default();
        let (engine_state_send, _) = tokio::sync::watch::channel(state);
        let (engine_queue_length_send, _) = tokio::sync::watch::channel(0);

        Ok(EngineActorState {
            rollup: self.config,
            client,
            engine: Engine::new(state, engine_state_send, engine_queue_length_send),
        })
    }
}

/// The configuration for the [`EngineActor`].
#[derive(Debug)]
pub(super) struct EngineActorState<EngineClient_: EngineClient> {
    /// The [`RollupConfig`] used to build tasks.
    pub(super) rollup: Arc<RollupConfig>,
    /// An [`OpEngineClient`] used for creating engine tasks.
    pub(super) client: Arc<EngineClient_>,
    /// The [`Engine`] task queue.
    pub(super) engine: Engine<EngineClient_>,
}

/// The communication context used by the engine actor.
#[derive(Debug)]
pub struct EngineContext {
    /// The cancellation token, shared between all tasks.
    pub cancellation: CancellationToken,
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
    pub fn new(config: EngineConfig) -> (EngineInboundData, Self) {
        let (inbound_request_tx, inbound_request_rx) = mpsc::channel(1024);

        let (finalized_l1_block_tx, finalized_l1_block_rx) = watch::channel(None);

        let (unsafe_head_tx, unsafe_head_rx) = if config.mode.is_sequencer() {
            let (unsafe_head_tx, unsafe_head_rx) = watch::channel(L2BlockInfo::default());

            (Some(unsafe_head_tx), Some(unsafe_head_rx))
        } else {
            (None, None)
        };

        let actor = Self {
            builder: config,
            inbound_request_rx,
            unsafe_head_tx,
            finalizer: L2Finalizer::new(finalized_l1_block_rx),
        };

        let outbound_data =
            EngineInboundData { finalized_l1_block_tx, inbound_request_tx, unsafe_head_rx };

        (outbound_data, actor)
    }
}

impl<EngineClient_: EngineClient + 'static> EngineActorState<EngineClient_> {
    /// Starts a task to handle engine queries.
    fn start_query_task(
        &self,
        mut inbound_rpc_channel: mpsc::Receiver<EngineRpcRequest>,
        rollup_boost: Arc<RollupBoostServer>,
    ) -> JoinHandle<Result<(), EngineError>> {
        let state_recv = self.engine.state_subscribe();
        let queue_length_recv = self.engine.queue_length_subscribe();
        let engine_client = self.client.clone();
        let rollup_config = self.rollup.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    query = inbound_rpc_channel.recv(), if !inbound_rpc_channel.is_closed() => {
                        let Some(query) = query else {
                            error!(target: "engine", "Engine rpc request receiver closed unexpectedly");
                            return Err(EngineError::ChannelClosed);
                        };
                        match query {
                            EngineRpcRequest::EngineQuery(req) => {
                                trace!(target: "engine", ?req, "Received engine query.");

                                if let Err(e) = req
                                    .handle(&state_recv, &queue_length_recv, &engine_client, &rollup_config)
                                    .await
                                {
                                    warn!(target: "engine", err = ?e, "Failed to handle engine query.");
                                }
                            },
                            EngineRpcRequest::RollupBoostAdminRequest(admin_query) => {
                                trace!(target: "engine", ?admin_query, "Received rollup boost admin query.");

                                match admin_query {
                                    RollupBoostAdminQuery::SetExecutionMode { execution_mode, sender } => {
                                        rollup_boost.server.set_execution_mode(execution_mode);
                                        let _ = sender.send(()).map_err(|_| {
                                            warn!(target: "engine", "set execution mode response channel closed when trying to send");
                                        });
                                    }
                                    RollupBoostAdminQuery::GetExecutionMode { sender } => {
                                        let execution_mode = rollup_boost.server.get_execution_mode();
                                        let _ = sender.send(execution_mode).map_err(|_| {
                                            warn!(target: "engine", "get execution mode response channel closed when trying to send");
                                        });
                                    }
                                }
                            },
                            EngineRpcRequest::RollupBoostHealthRequest(health_query) => {
                                trace!(target: "engine", ?health_query, "Received rollup boost health query.");

                                let health = rollup_boost.get_health();
                                health_query.sender.send(health.into()).unwrap();
                            },

                        }
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

        // Attempt to update the safe head following the reset.
        // IMPORTANT NOTE: We need to update the safe head BEFORE sending the reset signal to the
        // derivation actor. Since the derivation actor receives the safe head via a watch
        // channel, updating the safe head after sending the reset signal may cause a race
        // condition where the derivation actor receives the pre-reset safe head.
        self.maybe_update_safe_head(engine_l2_safe_head_tx);

        // Signal the derivation actor to reset.
        let signal = ResetSignal { l2_safe_head, l1_origin, system_config: Some(system_config) };
        match derivation_signal_tx.send(signal.signal()).await {
            Ok(_) => info!(target: "engine", "Sent reset signal to derivation actor"),
            Err(err) => {
                error!(target: "engine", ?err, "Failed to send reset signal to the derivation actor");
                return Err(EngineError::ChannelClosed);
            }
        }

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
            Err(err) => {
                match err.severity() {
                    EngineTaskErrorSeverity::Critical => {
                        error!(target: "engine", ?err, "Critical error draining engine tasks");
                        return Err(err.into());
                    }
                    EngineTaskErrorSeverity::Reset => {
                        warn!(target: "engine", ?err, "Received reset request");
                        self.reset(derivation_signal_tx, engine_l2_safe_head_tx, finalizer).await?;
                    }
                    EngineTaskErrorSeverity::Flush => {
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
                    EngineTaskErrorSeverity::Temporary => {
                        trace!(target: "engine", ?err, "Temporary error draining engine tasks");
                    }
                }
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

            // Only reset the engine if the sync state does not already know about a finalized
            // block.
            if self.engine.state().sync_state.finalized_head() != L2BlockInfo::default() {
                return Ok(());
            }

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
        info!(target: "engine", safe_head = ?state_safe_head, ?sent, "Attempted L2 Safe Head Update");
    }
}

#[async_trait]
impl NodeActor for EngineActor {
    type Error = EngineError;
    type StartData = EngineContext;

    async fn start(
        mut self,
        EngineContext {
            cancellation,
            engine_l2_safe_head_tx,
            sync_complete_tx,
            derivation_signal_tx,
        }: Self::StartData,
    ) -> Result<(), Self::Error> {
        let mut state = self.builder.build_state()?;

        let (rpc_tx, rpc_rx) = mpsc::channel(1024);

        // Start the engine query server in a separate task to avoid blocking the main task.
        let handle = state
            .start_query_task(rpc_rx, state.client.rollup_boost.clone())
            .with_cancellation_token(&cancellation)
            .then(async |result| {
                cancellation.cancel();

                let Some(result) = result else {
                    warn!(target: "engine", "Engine query task cancelled");
                    return Ok(());
                };

                let Ok(result) = result else {
                    error!(target: "engine", ?result, "Engine query task panicked");
                    return Err(EngineError::ChannelClosed);
                };

                match result {
                    Ok(()) => {
                        info!(target: "engine", "Engine query task completed successfully");
                        Ok(())
                    }
                    Err(err) => {
                        error!(target: "engine", ?err, "Engine query task failed");
                        Err(err)
                    }
                }
            });

        // The sync complete tx is consumed after the first successful send. Hence we need to wrap
        // it in an `Option` to ensure we satisfy the borrow checker.
        let mut sync_complete_tx = Some(sync_complete_tx);

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    warn!(target: "engine", "EngineActor received shutdown signal. Aborting engine query task.");

                    handle.await?;

                    return Ok(());
                },

                drain_result = // Attempt to drain all outstanding tasks from the engine queue before adding new ones.
                    state
                        .drain(
                            &derivation_signal_tx,
                            &mut sync_complete_tx,
                            &engine_l2_safe_head_tx,
                            &mut self.finalizer,
                        )
                         => {
                        if let Err(err) = drain_result {
                            error!(target: "engine", ?err, "Failed to drain engine tasks");
                            cancellation.cancel();
                            return Err(err);
                        }

                        // If the unsafe head has updated, propagate it to the outbound channels.
                        if let Some(unsafe_head_tx) = self.unsafe_head_tx.as_mut() {
                            unsafe_head_tx.send_if_modified(|val| {
                                let new_head = state.engine.state().sync_state.unsafe_head();
                                (*val != new_head).then(|| *val = new_head).is_some()
                            });
                        }
                }
            }

            tokio::select! {
                biased;

                _ = cancellation.cancelled() => {
                    warn!(target: "engine", "EngineActor received shutdown signal. Aborting engine query task.");

                    return Ok(());
                }

                // TODO: pull finalize channel into inbound_request_rx, adding a EngineActorRequest for it.
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

                req = self.inbound_request_rx.recv() => {
                    let Some(request_type) = req else {
                        error!(target: "engine", "Engine inbound request receiver closed unexpectedly");
                        cancellation.cancel();
                        return Err(EngineError::ChannelClosed);
                    };

                    match request_type {
                        EngineActorRequest::ResetRequest(ResetRequest{result_tx}) => {
                            warn!(target: "engine", "Received reset request");

                            let reset_res = state
                                .reset(&derivation_signal_tx, &engine_l2_safe_head_tx, &mut self.finalizer)
                                .await;

                            // Send the result if there is a channel on which to do so.
                            if let Some(tx) = result_tx {
                                let response_payload = reset_res.as_ref().map(|_| ()).map_err(|e| EngineClientError::ResetForkchoiceError(e.to_string()));
                                if tx.send(response_payload).await.is_err() {
                                    warn!(target: "engine", "Sending reset response failed");
                                }
                            }

                            reset_res?;
                        },
                        EngineActorRequest::SealRequest(SealRequest{payload_id, attributes, result_tx}) => {
                            let task = EngineTask::Seal(Box::new(SealTask::new(
                                state.client.clone(),
                                state.rollup.clone(),
                                payload_id,
                                attributes,
                                // The payload is not derived in this case.
                                false,
                                Some(result_tx),
                            )));
                            state.engine.enqueue(task);
                        },
                        EngineActorRequest::BuildRequest(BuildRequest{attributes, result_tx}) => {
                            let task = EngineTask::Build(Box::new(BuildTask::new(
                                state.client.clone(),
                                state.rollup.clone(),
                                attributes,
                                Some(result_tx),
                            )));
                            state.engine.enqueue(task);
                        },
                        EngineActorRequest::ConsolidateRequest(attributes) => {
                            self.finalizer.enqueue_for_finalization(&attributes);

                            let task = EngineTask::Consolidate(Box::new(ConsolidateTask::new(
                                state.client.clone(),
                                state.rollup.clone(),
                                attributes,
                                true,
                            )));
                            state.engine.enqueue(task);
                        },
                        EngineActorRequest::InsertUnsafeBlockRequest(envelope) => {
                            let task = EngineTask::Insert(Box::new(InsertTask::new(
                                state.client.clone(),
                                state.rollup.clone(),
                                envelope,
                                false, // The payload is not derived in this case. This is an unsafe block.
                            )));
                            state.engine.enqueue(task);
                        },
                        EngineActorRequest::RpcRequest(req) => {
                            let _ = rpc_tx.send(req).await.map_err(|_| {
                                error!(target: "engine", "Engine RPC request handler channel closed unexpectedly");
                                cancellation.cancel();
                                EngineError::ChannelClosed
                            })?;
                        },
                    }
                }
            }
        }
    }
}
