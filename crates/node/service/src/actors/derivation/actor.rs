//! [NodeActor] implementation for the derivation sub-routine.

use crate::{
    Metrics, NodeActor,
    actors::derivation::{DerivationActorRequest, DerivationEngineClient},
};
use async_trait::async_trait;
use kona_derive::{
    ActivationSignal, Pipeline, PipelineError, PipelineErrorKind, ResetError, ResetSignal, Signal,
    SignalReceiver, StepResult,
};
use kona_protocol::{L2BlockInfo, OpAttributesWithParent};
use thiserror::Error;
use tokio::{select, sync::mpsc};
use tokio_util::sync::CancellationToken;

/// The [NodeActor] for the derivation sub-routine.
///
/// This actor is responsible for receiving messages from [NodeActor]s and stepping the
/// derivation pipeline forward to produce new payload attributes. The actor then sends the payload
/// to the [NodeActor] responsible for the execution sub-routine.
#[derive(Debug)]
pub struct DerivationActor<DerivationEngineClient_, PipelineSignalReceiver>
where
    DerivationEngineClient_: DerivationEngineClient,
    PipelineSignalReceiver: Pipeline + SignalReceiver,
{
    /// The cancellation token, shared between all tasks.
    cancellation_token: CancellationToken,
    /// The channel on which all inbound requests are received by the [`DerivationActor`].
    inbound_request_rx: mpsc::Receiver<DerivationActorRequest>,
    /// The Engine client used to interact with the engine.
    engine_client: DerivationEngineClient_,
    /// The derivation pipeline.
    pipeline: PipelineSignalReceiver,

    /// The engine's L2 safe head, according to updates from the Engine.
    engine_l2_safe_head: L2BlockInfo,
    /// Whether we are waiting on the engine to acknowledge the last derived attributes
    awaiting_engine_l2_safe_head_update: bool,

    /// A flag indicating whether derivation is idle. Derivation is considered idle when it
    /// has yielded to wait for more data on the DAL.
    derivation_idle: bool,
    /// A flag indicating whether derivation is waiting for a signal. When waiting for a
    /// signal, derivation cannot process any incoming events.
    waiting_for_signal: bool,
    /// Whether the engine sync has completed. This will only ever go from false -> true.
    has_engine_sync_completed: bool,
}

impl<DerivationEngineClient_, PipelineSignalReceiver>
    DerivationActor<DerivationEngineClient_, PipelineSignalReceiver>
where
    DerivationEngineClient_: DerivationEngineClient,
    PipelineSignalReceiver: Pipeline + SignalReceiver,
{
    /// Creates a new instance of the [DerivationActor].
    pub fn new(
        engine_client: DerivationEngineClient_,
        cancellation_token: CancellationToken,
        inbound_request_rx: mpsc::Receiver<DerivationActorRequest>,
        pipeline: PipelineSignalReceiver,
    ) -> Self {
        Self {
            cancellation_token,
            pipeline,
            inbound_request_rx,
            engine_client,
            derivation_idle: true,
            waiting_for_signal: false,
            engine_l2_safe_head: L2BlockInfo::default(),
            awaiting_engine_l2_safe_head_update: false,
            has_engine_sync_completed: false,
        }
    }

    /// Handles a [`Signal`] received over the derivation signal receiver channel.
    async fn signal(&mut self, signal: Signal) {
        if let Signal::Reset(ResetSignal { l1_origin, .. }) = signal {
            kona_macros::set!(counter, Metrics::DERIVATION_L1_ORIGIN, l1_origin.number);
        }

        match self.pipeline.signal(signal).await {
            Ok(_) => info!(target: "derivation", ?signal, "[SIGNAL] Executed Successfully"),
            Err(e) => {
                error!(target: "derivation", ?e, ?signal, "Failed to signal derivation pipeline")
            }
        }
    }

    /// Attempts to step the derivation pipeline forward as much as possible in order to produce the
    /// next safe payload.
    async fn produce_next_attributes(&mut self) -> Result<OpAttributesWithParent, DerivationError> {
        // As we start the safe head at the disputed block's parent, we step the pipeline until the
        // first attributes are produced. All batches at and before the safe head will be
        // dropped, so the first payload will always be the disputed one.
        loop {
            match self.pipeline.step(self.engine_l2_safe_head).await {
                StepResult::PreparedAttributes => { /* continue; attributes will be sent off. */ }
                StepResult::AdvancedOrigin => {
                    let origin =
                        self.pipeline.origin().ok_or(PipelineError::MissingOrigin.crit())?.number;

                    kona_macros::set!(counter, Metrics::DERIVATION_L1_ORIGIN, origin);
                    debug!(target: "derivation", l1_block = origin, "Advanced L1 origin");
                }
                StepResult::OriginAdvanceErr(e) | StepResult::StepFailed(e) => {
                    match e {
                        PipelineErrorKind::Temporary(e) => {
                            // NotEnoughData is transient, and doesn't imply we need to wait for
                            // more data. We can continue stepping until we receive an Eof.
                            if matches!(e, PipelineError::NotEnoughData) {
                                continue;
                            }

                            debug!(
                                target: "derivation",
                                "Exhausted data source for now; Yielding until the chain has extended."
                            );
                            return Err(DerivationError::Yield);
                        }
                        PipelineErrorKind::Reset(e) => {
                            warn!(target: "derivation", "Derivation pipeline is being reset: {e}");

                            let system_config = self
                                .pipeline
                                .system_config_by_number(self.engine_l2_safe_head.block_info.number)
                                .await?;

                            if matches!(e, ResetError::HoloceneActivation) {
                                let l1_origin = self
                                    .pipeline
                                    .origin()
                                    .ok_or(PipelineError::MissingOrigin.crit())?;

                                self.pipeline
                                    .signal(
                                        ActivationSignal {
                                            l2_safe_head: self.engine_l2_safe_head,
                                            l1_origin,
                                            system_config: Some(system_config),
                                        }
                                        .signal(),
                                    )
                                    .await?;
                            } else {
                                if let ResetError::ReorgDetected(expected, new) = e {
                                    warn!(
                                        target: "derivation",
                                        "L1 reorg detected! Expected: {expected} | New: {new}"
                                    );

                                    kona_macros::inc!(counter, Metrics::L1_REORG_COUNT);
                                }
                                // send the `reset` signal to the engine actor only when interop is
                                // not active.
                                if !self.pipeline.rollup_config().is_interop_active(
                                    self.engine_l2_safe_head.block_info.timestamp,
                                ) {
                                    self.engine_client.reset_engine_forkchoice().await.map_err(|e| {
                                        error!(target: "derivation", ?e, "Failed to send reset request");
                                        DerivationError::Sender(Box::new(e))
                                    })?;
                                }
                                self.waiting_for_signal = true;
                                return Err(DerivationError::Yield);
                            }
                        }
                        PipelineErrorKind::Critical(_) => {
                            error!(target: "derivation", "Critical derivation error: {e}");
                            kona_macros::inc!(counter, Metrics::DERIVATION_CRITICAL_ERROR);
                            return Err(e.into());
                        }
                    }
                }
            }

            // If there are any new attributes, send them to the execution actor.
            if let Some(attrs) = self.pipeline.next() {
                return Ok(attrs);
            }
        }
    }

    /// Attempts to process the next payload attributes.
    ///
    /// There are a few constraints around stepping on the derivation pipeline.
    /// - The l2 safe head ([`L2BlockInfo`]) must not be the zero hash.
    /// - The pipeline must not be stepped on with the same L2 safe head twice.
    /// - Errors must be bubbled up to the caller.
    ///
    /// In order to achieve this, the channel to receive the L2 safe head
    /// [`L2BlockInfo`] from the engine is *only* marked as _seen_ after payload
    /// attributes are successfully produced. If the pipeline step errors,
    /// the same [`L2BlockInfo`] is used again. If the [`L2BlockInfo`] is the
    /// zero hash, the pipeline is not stepped on.
    async fn process(&mut self, msg: DerivationActorRequest) -> Result<(), DerivationError> {
        match msg {
            DerivationActorRequest::ProcessL1HeadUpdateRequest(l1_head) => {
                info!(target: "derivation", l1_head = ?l1_head, "Processing l1 head update");

                // If derivation isn't idle and the message hasn't observed a safe head update
                // already, check if the safe head has changed before continuing.
                // This is to prevent attempts to progress the pipeline while it is
                // in the middle of processing a channel.
                if !self.derivation_idle && self.awaiting_engine_l2_safe_head_update {
                    info!(target: "derivation", "Safe head hasn't changed, skipping derivation.");
                    return Ok(());
                }
            }
            DerivationActorRequest::ProcessEngineSafeHeadUpdateRequest(safe_head) => {
                info!(target: "derivation", safe_head = ?safe_head, "Received safe head from engine.");
                self.engine_l2_safe_head = safe_head;
                self.awaiting_engine_l2_safe_head_update = false;
            }
            DerivationActorRequest::ProcessEngineSyncCompletionRequest => {
                info!(target: "derivation", "Engine finished syncing, starting derivation.");
                self.has_engine_sync_completed = true;
            }
            _ => {
                error!(target: "derivation", ?msg, "Unexpected request in process(...)");
                return Err(DerivationError::RequestReceiveFailed)
            }
        }

        // Only attempt derivation once the engine finishes syncing.
        if !self.has_engine_sync_completed {
            info!(target: "derivation", "Engine not ready, skipping derivation");
            return Ok(());
        } else if self.waiting_for_signal {
            info!(target: "derivation", "Waiting to receive a signal, skipping derivation");
            return Ok(());
        } else if self.engine_l2_safe_head.block_info.hash.is_zero() {
            warn!(target: "derivation", engine_safe_head = ?self.engine_l2_safe_head.block_info.number, "Waiting for engine to initialize state prior to derivation.");
            return Ok(());
        }
        trace!(target: "derivation", "Attempting derivation.");

        // Advance the pipeline as much as possible, new data may be available or there still may be
        // payloads in the attributes queue.
        let payload_attributes = match self.produce_next_attributes().await {
            Ok(attrs) => attrs,
            Err(DerivationError::Yield) => {
                info!(target: "derivation", "Yielding derivation until more data is available.");
                // Yield until more data is available.
                self.derivation_idle = true;
                return Ok(());
            }
            Err(e) => {
                return Err(e);
            }
        };
        trace!(target: "derivation", ?payload_attributes, "Produced payload attributes.");

        // Mark derivation as busy.
        self.derivation_idle = false;
        self.awaiting_engine_l2_safe_head_update = true;

        // Send payload attributes out for processing.
        self.engine_client
            .send_derived_attributes(payload_attributes)
            .await
            .map_err(|e| DerivationError::Sender(Box::new(e)))?;

        Ok(())
    }
}

#[async_trait]
impl<DerivationEngineClient_, PipelineSignalReceiver> NodeActor
    for DerivationActor<DerivationEngineClient_, PipelineSignalReceiver>
where
    DerivationEngineClient_: DerivationEngineClient + 'static,
    PipelineSignalReceiver: Pipeline + SignalReceiver + Send + Sync + 'static,
{
    type Error = DerivationError;
    type StartData = ();

    async fn start(mut self, _: Self::StartData) -> Result<(), Self::Error> {
        loop {
            select! {
                biased;

                _ = self.cancellation_token.cancelled() => {
                    info!(
                        target: "derivation",
                        "Received shutdown signal. Exiting derivation task."
                    );
                    return Ok(());
                }
                req = self.inbound_request_rx.recv() => {
                    let Some(request_type) = req else {
                        error!(target: "derivation", "DerivationActor inbound request receiver closed unexpectedly");
                        self.cancellation_token.cancel();
                        return Err(DerivationError::RequestReceiveFailed);
                    };

                    match request_type {
                        DerivationActorRequest::ProcessEngineSignalRequest(signal) => {
                            self.signal(signal).await;
                            self.waiting_for_signal = false;
                        },
                        req @ DerivationActorRequest::ProcessL1HeadUpdateRequest(_)
                            | req @ DerivationActorRequest::ProcessEngineSafeHeadUpdateRequest(_)
                            | req @ DerivationActorRequest::ProcessEngineSyncCompletionRequest => {
                            self.process(req).await?;
                        },
                    }
                }
            }
        }
    }
}

/// An error from the [DerivationActor].
#[derive(Error, Debug)]
pub enum DerivationError {
    /// An error originating from the derivation pipeline.
    #[error(transparent)]
    Pipeline(#[from] PipelineErrorKind),
    /// Waiting for more data to be available.
    #[error("Waiting for more data to be available")]
    Yield,
    /// An error originating from the broadcast sender.
    #[error("Failed to send event to broadcast sender: {0}")]
    Sender(Box<dyn std::error::Error>),
    /// Failed to receive inbound request
    #[error("Failed to receive inbound request")]
    RequestReceiveFailed,
}
