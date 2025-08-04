use std::io;

use async_trait::async_trait;
use kona_interop::InteropValidator;
use kona_supervisor_core::{ChainProcessor, event::ChainEvent, syncnode::ManagedNodeProvider};
use kona_supervisor_storage::{
    DerivationStorage, HeadRefStorageWriter, LogStorage, StorageRewinder,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::SupervisorActor;

/// Represents an actor that processes chain events using the [`ChainProcessor`].
/// It listens for [`ChainEvent`]s and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessorActor<P, W, V> {
    chain_processor: ChainProcessor<P, W, V>,
    cancel_token: CancellationToken,
    event_rx: mpsc::Receiver<ChainEvent>,
}

impl<P, W, V> ChainProcessorActor<P, W, V>
where
    P: ManagedNodeProvider + 'static,
    V: InteropValidator + 'static,
    W: LogStorage + DerivationStorage + HeadRefStorageWriter + StorageRewinder + 'static,
{
    /// Creates a new [`ChainProcessorActor`].
    pub const fn new(
        chain_processor: ChainProcessor<P, W, V>,
        cancel_token: CancellationToken,
        event_rx: mpsc::Receiver<ChainEvent>,
    ) -> Self {
        Self { chain_processor, cancel_token, event_rx }
    }
}

#[async_trait]
impl<P, W, V> SupervisorActor for ChainProcessorActor<P, W, V>
where
    P: ManagedNodeProvider + Send + Sync + 'static,
    V: InteropValidator + Send + Sync + 'static,
    W: LogStorage
        + DerivationStorage
        + HeadRefStorageWriter
        + StorageRewinder
        + Send
        + Sync
        + 'static,
{
    type InboundEvent = ChainEvent;
    type Error = io::Error;

    async fn start(mut self) -> Result<(), Self::Error> {
        info!(
            target: "supervisor::chain_processor_actor",
            "Starting ChainProcessorActor"
        );

        loop {
            tokio::select! {
                maybe_event = self.event_rx.recv() => {
                    if let Some(event) = maybe_event {
                        self.chain_processor.handle_event(event).await;
                    }
                }
                _ = self.cancel_token.cancelled() => {
                    info!(
                        target: "supervisor::chain_processor_actor",
                        "ChainProcessorActor cancellation requested, stopping..."
                    );
                    break;
                }
            }
        }

        Ok(())
    }
}
