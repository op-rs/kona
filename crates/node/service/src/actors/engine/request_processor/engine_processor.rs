use crate::{
    BuildRequest, EngineClientError, EngineDerivationClient, EngineError, ResetRequest, SealRequest,
};
use kona_derive::{ResetSignal, Signal};
use kona_engine::{
    BuildTask, ConsolidateTask, Engine, EngineClient, EngineTask, EngineTaskError,
    EngineTaskErrorSeverity, FinalizeTask, InsertTask, SealTask,
};
use kona_genesis::RollupConfig;
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::{collections::BTreeMap, sync::Arc};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};

/// An internal type alias for L1 block numbers.
type L1BlockNumber = u64;
/// An internal type alias for L2 block numbers.
type L2BlockNumber = u64;

/// A request to process engine tasks.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EngineProcessingRequest {
    /// Request to start building a block.
    Build(Box<BuildRequest>),
    /// Request to process a derived L2 safe head.
    ProcessDerivedL2Attributes(Box<OpAttributesWithParent>),
    /// Request to process a finalized L1 block.
    ProcessFinalizedL1Block(Box<BlockInfo>),
    /// Request to process a received unsafe L2 block.
    ProcessUnsafeL2Block(Box<OpExecutionPayloadEnvelope>),
    /// Request to reset the forkchoice.
    Reset(Box<ResetRequest>),
    /// Request to seal a block.
    Seal(Box<SealRequest>),
}

/// Responsible for managing the operations sent to the execution layer's Engine API. To accomplish
/// this, it uses the [`Engine`] task queue to order Engine API  interactions based off of
/// the [`Ord`] implementation of [`EngineTask`].
#[derive(Debug)]
pub struct EngineProcessor<EngineClient_, DerivationClient>
where
    EngineClient_: EngineClient,
    DerivationClient: EngineDerivationClient,
{
    /// A map of `L1 block number -> highest derived L2 block number` within the L1 epoch, used to
    /// track derived [`OpAttributesWithParent`] awaiting finalization. When a new finalized L1
    /// block is received, the highest L2 block whose inputs are contained within the finalized
    /// L1 chain is finalized.
    awaiting_finalization: BTreeMap<L1BlockNumber, L2BlockNumber>,
    /// The client used to send messages to the [`crate::DerivationActor`].
    derivation_client: DerivationClient,
    /// Whether the EL sync is complete. This should only ever go from false to true.
    el_sync_complete: bool,
    /// The last safe head update sent.
    last_safe_head_sent: L2BlockInfo,
    /// The [`RollupConfig`] .
    /// A channel to use to relay the current unsafe head.
    /// ## Note
    /// This is `Some` when the node is in sequencer mode, and `None` when the node is in validator
    /// mode.
    unsafe_head_tx: Option<watch::Sender<L2BlockInfo>>,

    /// The [`RollupConfig`] used to build tasks.
    rollup: Arc<RollupConfig>,
    /// An [`EngineClient`] used for creating engine tasks.
    client: Arc<EngineClient_>,
    /// The [`Engine`] task queue.
    engine: Engine<EngineClient_>,
}

impl<EngineClient_, DerivationClient> EngineProcessor<EngineClient_, DerivationClient>
where
    EngineClient_: EngineClient + 'static,
    DerivationClient: EngineDerivationClient + 'static,
{
    /// Constructs a new [`EngineProcessor`] from the params.
    pub fn new(
        client: Arc<EngineClient_>,
        config: Arc<RollupConfig>,
        derivation_client: DerivationClient,
        engine: Engine<EngineClient_>,
        unsafe_head_tx: Option<watch::Sender<L2BlockInfo>>,
    ) -> Self {
        Self {
            awaiting_finalization: BTreeMap::new(),
            client,
            derivation_client,
            el_sync_complete: false,
            engine,
            last_safe_head_sent: L2BlockInfo::default(),
            rollup: config,
            unsafe_head_tx,
        }
    }

    /// Starts a task to handle engine processing requests.
    pub fn start(
        mut self,
        mut request_channel: mpsc::Receiver<EngineProcessingRequest>,
    ) -> JoinHandle<Result<(), EngineError>> {
        tokio::spawn(async move {
            loop {
                // Attempt to drain all outstanding tasks from the engine queue before adding new
                // ones.
                let drain_result = self.drain().await;

                if let Err(err) = drain_result {
                    error!(target: "engine", ?err, "Failed to drain engine tasks");
                    return Err(err);
                }

                // If the unsafe head has updated, propagate it to the outbound channels.
                if let Some(unsafe_head_tx) = self.unsafe_head_tx.as_ref() {
                    unsafe_head_tx.send_if_modified(|val| {
                        let new_head = self.engine.state().sync_state.unsafe_head();
                        (*val != new_head).then(|| *val = new_head).is_some()
                    });
                }

                // Wait for the next processing request.
                let Some(request) = request_channel.recv().await else {
                    error!(target: "engine", "Engine processing request receiver closed unexpectedly");
                    return Err(EngineError::ChannelClosed);
                };

                match request {
                    EngineProcessingRequest::Build(build_request) => {
                        let BuildRequest { attributes, result_tx } = *build_request;
                        let task = EngineTask::Build(Box::new(BuildTask::new(
                            self.client.clone(),
                            self.rollup.clone(),
                            attributes,
                            Some(result_tx),
                        )));
                        self.engine.enqueue(task);
                    }
                    EngineProcessingRequest::ProcessDerivedL2Attributes(attributes) => {
                        self.enqueue_for_finalization(&attributes);

                        let task = EngineTask::Consolidate(Box::new(ConsolidateTask::new(
                            self.client.clone(),
                            self.rollup.clone(),
                            *attributes,
                            true,
                        )));
                        self.engine.enqueue(task);
                    }
                    EngineProcessingRequest::ProcessFinalizedL1Block(finalized_l1_block) => {
                        // Attempt to finalize any L2 blocks that are contained within the finalized
                        // L1 chain.
                        self.try_finalize_next(*finalized_l1_block).await;
                    }
                    EngineProcessingRequest::ProcessUnsafeL2Block(envelope) => {
                        let task = EngineTask::Insert(Box::new(InsertTask::new(
                            self.client.clone(),
                            self.rollup.clone(),
                            *envelope,
                            false, /* The payload is not derived in this case. This is an unsafe
                                    * block. */
                        )));
                        self.engine.enqueue(task);
                    }
                    EngineProcessingRequest::Reset(reset_request) => {
                        warn!(target: "engine", "Received reset request");

                        let reset_res = self.reset().await;

                        // Send the result.
                        let response_payload = reset_res
                            .as_ref()
                            .map(|_| ())
                            .map_err(|e| EngineClientError::ResetForkchoiceError(e.to_string()));
                        if reset_request.result_tx.send(response_payload).await.is_err() {
                            warn!(target: "engine", "Sending reset response failed");
                        }

                        reset_res?;
                    }
                    EngineProcessingRequest::Seal(seal_request) => {
                        let SealRequest { payload_id, attributes, result_tx } = *seal_request;
                        let task = EngineTask::Seal(Box::new(SealTask::new(
                            self.client.clone(),
                            self.rollup.clone(),
                            payload_id,
                            attributes,
                            // The payload is not derived in this case.
                            false,
                            Some(result_tx),
                        )));
                        self.engine.enqueue(task);
                    }
                }
            }
        })
    }

    /// Enqueues a derived [`OpAttributesWithParent`] for finalization. When a new finalized L1
    /// block is observed that is `>=` the height of [`OpAttributesWithParent::derived_from`], the
    /// L2 block associated with the payload attributes will be finalized.
    fn enqueue_for_finalization(&mut self, attributes: &OpAttributesWithParent) {
        self.awaiting_finalization
            .entry(
                attributes.derived_from.map(|b| b.number).expect(
                    "Fatal: Cannot enqueue attributes for finalization that weren't derived",
                ),
            )
            .and_modify(|n| *n = (*n).max(attributes.block_number()))
            .or_insert(attributes.block_number());
    }

    /// Clears the finalization queue.
    fn clear_finalization_queue(&mut self) {
        self.awaiting_finalization.clear();
    }

    /// Attempts to finalize any L2 blocks that the finalizer knows about and are contained within
    /// the new finalized L1 chain.
    async fn try_finalize_next(&mut self, new_finalized_l1_block: BlockInfo) {
        // Find the highest safe L2 block that is contained within the finalized chain,
        // that the finalizer is aware of.
        let highest_safe =
            self.awaiting_finalization.range(..=new_finalized_l1_block.number).next_back();

        // If the highest safe block is found, enqueue a finalization task and drain the
        // queue of all L1 blocks not contained in the finalized L1 chain.
        if let Some((_, highest_safe_number)) = highest_safe {
            let task = EngineTask::Finalize(Box::new(FinalizeTask::new(
                self.client.clone(),
                self.rollup.clone(),
                *highest_safe_number,
            )));
            self.engine.enqueue(task);

            self.awaiting_finalization.retain(|&number, _| number > new_finalized_l1_block.number);
        }
    }

    /// Resets the inner [`Engine`] and propagates the reset to the derivation actor.
    async fn reset(&mut self) -> Result<(), EngineError> {
        // Reset the engine.
        let (l2_safe_head, l1_origin, system_config) =
            self.engine.reset(self.client.clone(), self.rollup.clone()).await?;

        // Signal the derivation actor to reset.
        let signal = ResetSignal { l2_safe_head, l1_origin, system_config: Some(system_config) };
        match self.derivation_client.send_signal(signal.signal()).await {
            Ok(_) => info!(target: "engine", "Sent reset signal to derivation actor"),
            Err(err) => {
                error!(target: "engine", ?err, "Failed to send reset signal to the derivation actor");
                return Err(EngineError::ChannelClosed);
            }
        }

        self.send_derivation_actor_safe_head_if_updated().await?;

        // Clear the queue of L2 blocks awaiting finalization.
        self.clear_finalization_queue();

        Ok(())
    }

    /// Drains the inner [`Engine`] task queue and attempts to update the safe head.
    async fn drain(&mut self) -> Result<(), EngineError> {
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
                        self.reset().await?;
                    }
                    EngineTaskErrorSeverity::Flush => {
                        // This error is encountered when the payload is marked INVALID
                        // by the engine api. Post-holocene, the payload is replaced by
                        // a "deposits-only" block and re-executed. At the same time,
                        // the channel and any remaining buffered batches are flushed.
                        warn!(target: "engine", ?err, "Invalid payload, Flushing derivation pipeline.");
                        match self.derivation_client.send_signal(Signal::FlushChannel).await {
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

        self.send_derivation_actor_safe_head_if_updated().await?;
        self.check_el_sync().await?;

        Ok(())
    }

    /// Checks if the EL has finished syncing, notifying the derivation actor if it has.
    async fn check_el_sync(&mut self) -> Result<(), EngineError> {
        if self.engine.state().el_sync_finished {
            if self.el_sync_complete {
                return Ok(());
            } else {
                self.el_sync_complete = true;
            }

            // Only reset the engine if the sync state does not already know about a finalized
            // block.
            if self.engine.state().sync_state.finalized_head() != L2BlockInfo::default() {
                info!(target: "engine", "finalized head is not default, so not resetting");
                return Ok(());
            }

            // If the sync status is finished, we can reset the engine and start derivation.
            info!(target: "engine", "Performing initial engine reset");
            self.reset().await?;

            self.derivation_client.notify_sync_completed().await.map_err(|e| {
                error!(target: "engine", ?e, "Failed to notify sync completed");
                EngineError::ChannelClosed
            })?;
        }

        Ok(())
    }

    /// Attempts to send the [`crate::DerivationActor`] the safe head if updated.
    async fn send_derivation_actor_safe_head_if_updated(&mut self) -> Result<(), EngineError> {
        let engine_safe_head = self.engine.state().sync_state.safe_head();
        if engine_safe_head == self.last_safe_head_sent {
            info!(target: "engine", safe_head = ?engine_safe_head, "Safe head unchanged");
            // This was already sent, so do not send it.
            return Ok(());
        }

        self.derivation_client.send_new_engine_safe_head(engine_safe_head).await.map_err(|e| {
            error!(target: "engine", ?e, "Failed to send new engine safe head");
            EngineError::ChannelClosed
        })?;

        info!(target: "engine", safe_head = ?engine_safe_head, "Attempted L2 Safe Head Update");
        self.last_safe_head_sent = engine_safe_head;

        Ok(())
    }
}
