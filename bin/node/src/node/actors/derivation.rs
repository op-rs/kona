//! [NodeActor] implementation for the derivation sub-routine.

use crate::node::{NodeActor, NodeEvent};
use async_trait::async_trait;
use kona_derive::{
    errors::{PipelineError, PipelineErrorKind, ResetError},
    traits::{Pipeline, SignalReceiver},
    types::{ActivationSignal, ResetSignal, StepResult},
};
use kona_protocol::L2BlockInfo;
use kona_rpc::OpAttributesWithParent;
use thiserror::Error;
use tokio::{
    select,
    sync::broadcast::{
        Receiver, Sender,
        error::{RecvError, SendError},
    },
};
use tokio_util::sync::CancellationToken;

/// The [NodeActor] for the derivation sub-routine.
///
/// This actor is responsible for receiving messages from [NodeActor]s and stepping the
/// derivation pipeline forward to produce new payload attributes. The actor then sends the payload
/// to the [NodeActor] responsible for the execution sub-routine.
pub struct DerivationActor<P>
where
    P: Pipeline + SignalReceiver,
{
    /// The derivation pipeline.
    pipeline: P,
    /// The latest L2 safe head.
    l2_safe_head: L2BlockInfo,
    /// The sender for signals to other actors.
    sender: Sender<NodeEvent>,
    /// The receiver for signals from other actors.
    receiver: Receiver<NodeEvent>,
    /// The cancellation token, shared between all tasks.
    cancellation: CancellationToken,
}

impl<P> DerivationActor<P>
where
    P: Pipeline + SignalReceiver,
{
    /// Creates a new instance of the [DerivationActor].
    pub fn new(
        pipeline: P,
        l2_safe_head: L2BlockInfo,
        sender: Sender<NodeEvent>,
        receiver: Receiver<NodeEvent>,
        cancellation: CancellationToken,
    ) -> Self {
        Self { pipeline, l2_safe_head, sender, receiver, cancellation }
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
                            // more data. We can continue stepping until
                            // we receive an Eof.
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
    type Error = DerivationError;
    type Event = NodeEvent;

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
                msg = self.receiver.recv() => {
                    self.process(msg?).await?;
                }
            }
        }
    }

    async fn process(&mut self, msg: Self::Event) -> Result<(), Self::Error> {
        match msg {
            NodeEvent::L1HeadUpdate(_) | NodeEvent::L2ForkchoiceUpdate(_) => {
                // Update the local view of the safe head.
                //
                // TODO: Remove in favor of req-resp; We don't want to maintain a local view once
                // we have the engine hooked up.
                if let NodeEvent::L2ForkchoiceUpdate(safe_head) = msg {
                    self.l2_safe_head = safe_head;
                }

                // Advance the pipeline as much as possible, new data may be available.
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

                self.sender.send(NodeEvent::DerivedPayload(payload_attrs))?;
                Ok(())
            }
            _ => {
                // Ignore all other messages.
                Ok(())
            }
        }
    }
}

/// An error from the [DerivationActor].
#[derive(Error, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum DerivationError {
    /// An error originating from the derivation pipeline.
    #[error(transparent)]
    Pipeline(#[from] PipelineErrorKind),
    /// Waiting for more data to be available.
    #[error("Waiting for more data to be available")]
    Yield,
    /// An error originating from the broadcast sender.
    #[error("Failed to send event to broadcast sender")]
    Sender(#[from] SendError<NodeEvent>),
    /// An error originating from the broadcast receiver.
    #[error("Failed to receive event from broadcast receiver")]
    Receiver(#[from] RecvError),
}
