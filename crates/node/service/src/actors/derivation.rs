//! [NodeActor] implementation for the derivation sub-routine.

use crate::NodeActor;
use async_trait::async_trait;
use kona_derive::{
    errors::{PipelineError, PipelineErrorKind, ResetError},
    traits::{Pipeline, SignalReceiver},
    types::{ActivationSignal, ResetSignal, StepResult},
};
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use thiserror::Error;
use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender, error::SendError},
};
use tokio_util::sync::CancellationToken;

/// The [NodeActor] for the derivation sub-routine.
///
/// This actor is responsible for receiving messages from [NodeActor]s and stepping the
/// derivation pipeline forward to produce new payload attributes. The actor then sends the payload
/// to the [NodeActor] responsible for the execution sub-routine.
#[derive(Debug)]
pub struct DerivationActor<P>
where
    P: Pipeline + SignalReceiver,
{
    /// The derivation pipeline.
    pipeline: P,
    /// The latest L2 safe head.
    l2_safe_head: L2BlockInfo,
    /// A receiver that tells derivation to begin.
    sync_complete_rx: UnboundedReceiver<()>,
    /// A flag indicating whether the derivation pipeline is ready to start.
    engine_ready: bool,
    /// The sender for derived [OpAttributesWithParent]s produced by the actor.
    attributes_out: UnboundedSender<OpAttributesWithParent>,
    /// The receiver for L1 head update notifications.
    l1_head_updates: UnboundedReceiver<BlockInfo>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl<P> DerivationActor<P>
where
    P: Pipeline + SignalReceiver,
{
    /// Creates a new instance of the [DerivationActor].
    pub const fn new(
        pipeline: P,
        l2_safe_head: L2BlockInfo,
        sync_complete_rx: UnboundedReceiver<()>,
        attributes_out: UnboundedSender<OpAttributesWithParent>,
        l1_head_updates: UnboundedReceiver<BlockInfo>,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            pipeline,
            l2_safe_head,
            sync_complete_rx,
            engine_ready: false,
            attributes_out,
            l1_head_updates,
            cancellation,
        }
    }

    /// Attempts to step the derivation pipeline forward as much as possible in order to produce the
    /// next safe payload.
    async fn produce_next_safe_payload(
        &mut self,
    ) -> Result<OpAttributesWithParent, DerivationError> {
        // As we start the safe head at the disputed block's parent, we step the pipeline until the
        // first attributes are produced. All batches at and before the safe head will be
        // dropped, so the first payload will always be the disputed one.
        loop {
            match self.pipeline.step(self.l2_safe_head).await {
                StepResult::PreparedAttributes => { /* continue; attributes will be sent off. */ }
                StepResult::AdvancedOrigin => {
                    info!(
                        target: "derivation",
                        "Advanced L1 origin to block #{}",
                        self.pipeline.origin().ok_or(PipelineError::MissingOrigin.crit())?.number,
                    );
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
                                .system_config_by_number(self.l2_safe_head.block_info.number)
                                .await?;

                            if matches!(e, ResetError::HoloceneActivation) {
                                let l1_origin = self
                                    .pipeline
                                    .origin()
                                    .ok_or(PipelineError::MissingOrigin.crit())?;
                                self.pipeline
                                    .signal(
                                        ActivationSignal {
                                            l2_safe_head: self.l2_safe_head,
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
                                }

                                // Reset the pipeline to the initial L2 safe head and L1 origin,
                                // and try again.
                                let l1_origin = self
                                    .pipeline
                                    .origin()
                                    .ok_or(PipelineError::MissingOrigin.crit())?;
                                self.pipeline
                                    .signal(
                                        ResetSignal {
                                            l2_safe_head: self.l2_safe_head,
                                            l1_origin,
                                            system_config: Some(system_config),
                                        }
                                        .signal(),
                                    )
                                    .await?;
                            }
                        }
                        PipelineErrorKind::Critical(_) => {
                            error!(target: "derivation", "Critical derivation error: {e}");
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
}

#[async_trait]
impl<P> NodeActor for DerivationActor<P>
where
    P: Pipeline + SignalReceiver + Send + Sync,
{
    type InboundEvent = InboundDerivationMessage;
    type Error = DerivationError;

    async fn start(mut self) -> Result<(), Self::Error> {
        loop {
            select! {
                _ = self.cancellation.cancelled() => {
                    info!(
                        target: "derivation",
                        "Received shutdown signal. Exiting derivation task."
                    );
                    return Ok(());
                }
                _ = self.sync_complete_rx.recv() => {
                    if self.engine_ready {
                        // Already received the signal, ignore.
                        continue;
                    }
                    info!(target: "derivation", "Engine finished syncing, starting derivation.");
                    self.engine_ready = true;
                    self.sync_complete_rx.close();
                    // Optimistically process the first message.
                    self.process(InboundDerivationMessage::NewDataAvailable).await?;
                }
                msg = self.l1_head_updates.recv() => {
                    if msg.is_none() {
                        error!(
                            target: "derivation",
                            "L1 head update stream closed without cancellation. Exiting derivation task."
                        );
                        return Ok(());
                    }

                    self.process(InboundDerivationMessage::NewDataAvailable).await?;
                }
            }
        }
    }

    async fn process(&mut self, _: Self::InboundEvent) -> Result<(), Self::Error> {
        // Only attempt derivation once the engine finishes syncing.
        if !self.engine_ready {
            trace!(target: "derivation", "Engine not ready, skipping derivation.");
            return Ok(());
        }

        // Advance the pipeline as much as possible, new data may be available or there still may be
        // payloads in the attributes queue.
        let payload_attrs = match self.produce_next_safe_payload().await {
            Ok(attrs) => attrs,
            Err(DerivationError::Yield) => {
                // Yield until more data is available.
                return Ok(());
            }
            Err(e) => {
                return Err(e);
            }
        };

        self.attributes_out.send(payload_attrs).map_err(Box::new)?;
        Ok(())
    }
}

/// Messages that the [DerivationActor] can receive from other actors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundDerivationMessage {
    /// New data is potentially available for processing on the data availability layer.
    NewDataAvailable,
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
    #[error("Failed to send event to broadcast sender")]
    Sender(#[from] Box<SendError<OpAttributesWithParent>>),
}
