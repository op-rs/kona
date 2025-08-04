use super::chain::ChainProcessor;
use crate::{event::ChainEvent, syncnode::ManagedNodeProvider};
use kona_interop::InteropValidator;
use kona_supervisor_storage::{
    DerivationStorage, HeadRefStorageWriter, LogStorage, StorageRewinder,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

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

    /// Starts the actor, listening for events and calling handle_event.
    pub async fn start(mut self) {
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
    }
}
