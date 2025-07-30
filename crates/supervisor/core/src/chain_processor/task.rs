use super::handlers::{
    CrossSafeHandler, CrossUnsafeHandler, EventHandler, FinalizedHandler, InvalidationHandler,
    OriginHandler, ReplacementHandler, SafeBlockHandler, UnsafeBlockHandler,
};
use crate::{
    ChainRewinder, LogIndexer, ProcessorState, event::ChainEvent, syncnode::ManagedNodeProvider,
};
use alloy_primitives::ChainId;
use kona_interop::InteropValidator;
use kona_supervisor_storage::{
    DerivationStorage, HeadRefStorageWriter, LogStorage, StorageRewinder,
};
use std::{fmt::Debug, sync::Arc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Represents a task that processes chain events from a managed node.
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessorTask<P, W, V> {
    chain_id: ChainId,
    metrics_enabled: Option<bool>,
    cancel_token: CancellationToken,
    /// The channel for receiving node events.
    event_rx: mpsc::Receiver<ChainEvent>,

    // state
    state: ProcessorState,

    // Handlers for different types of chain events.
    unsafe_handler: UnsafeBlockHandler<P, W, V>,
    safe_handler: SafeBlockHandler<P, W, V>,
    origin_handler: OriginHandler<P, W>,
    invalidation_handler: InvalidationHandler<P, W>,
    replacement_handler: ReplacementHandler<P, W>,
    finalized_handler: FinalizedHandler<P, W>,
    cross_unsafe_handler: CrossUnsafeHandler<P>,
    cross_safe_handler: CrossSafeHandler<P>,
}

impl<P, W, V> ChainProcessorTask<P, W, V>
where
    P: ManagedNodeProvider + 'static,
    V: InteropValidator + 'static,
    W: LogStorage + DerivationStorage + HeadRefStorageWriter + StorageRewinder + 'static,
{
    /// Creates a new [`ChainProcessorTask`].
    pub fn new(
        validator: Arc<V>,
        chain_id: ChainId,
        managed_node: Arc<P>,
        db_provider: Arc<W>,
        cancel_token: CancellationToken,
        event_rx: mpsc::Receiver<ChainEvent>,
    ) -> Self {
        let log_indexer =
            Arc::new(LogIndexer::new(chain_id, managed_node.clone(), db_provider.clone()));
        let rewinder = Arc::new(ChainRewinder::new(chain_id, db_provider.clone()));

        let unsafe_handler = UnsafeBlockHandler::new(
            chain_id,
            validator.clone(),
            db_provider.clone(),
            log_indexer.clone(),
        );

        let safe_handler = SafeBlockHandler::new(
            chain_id,
            managed_node.clone(),
            db_provider.clone(),
            validator,
            log_indexer.clone(),
            rewinder,
        );

        let origin_handler =
            OriginHandler::new(chain_id, managed_node.clone(), db_provider.clone());

        let invalidation_handler =
            InvalidationHandler::new(chain_id, managed_node.clone(), db_provider.clone());

        let replacement_handler =
            ReplacementHandler::new(chain_id, log_indexer, db_provider.clone());

        let finalized_handler = FinalizedHandler::new(chain_id, managed_node.clone(), db_provider);

        let cross_unsafe_handler = CrossUnsafeHandler::new(chain_id, managed_node.clone());
        let cross_safe_handler = CrossSafeHandler::new(chain_id, managed_node);

        Self {
            chain_id,
            metrics_enabled: None,
            cancel_token,
            event_rx,

            state: ProcessorState::new(),

            // Handlers for different types of chain events.
            unsafe_handler,
            safe_handler,
            origin_handler,
            invalidation_handler,
            replacement_handler,
            finalized_handler,
            cross_unsafe_handler,
            cross_safe_handler,
        }
    }

    /// Enables metrics on the database environment.
    pub const fn with_metrics(mut self) -> Self {
        self.metrics_enabled = Some(true);
        self
    }

    /// Runs the chain processor task, which listens for events and processes them.
    /// This method will run indefinitely until the cancellation token is triggered.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                maybe_event = self.event_rx.recv() => {
                    if let Some(event) = maybe_event {
                        self.handle_event(event).await;
                    }
                }
                _ = self.cancel_token.cancelled() => {
                    info!(
                        target: "supervisor::chain_processor",
                        chain_id = self.chain_id,
                        "ChainProcessorTask cancellation requested, stopping..."
                    );
                    break;
                }
            }
        }
    }

    async fn handle_event(&mut self, event: ChainEvent) {
        let result = match event {
            ChainEvent::UnsafeBlock { block } => {
                self.unsafe_handler.handle(block, &mut self.state).await
            }
            ChainEvent::DerivedBlock { derived_ref_pair } => {
                self.safe_handler.handle(derived_ref_pair, &mut self.state).await
            }
            ChainEvent::DerivationOriginUpdate { origin } => {
                self.origin_handler.handle(origin, &mut self.state).await
            }
            ChainEvent::InvalidateBlock { block } => {
                self.invalidation_handler.handle(block, &mut self.state).await
            }
            ChainEvent::BlockReplaced { replacement } => {
                self.replacement_handler.handle(replacement, &mut self.state).await
            }
            ChainEvent::FinalizedSourceUpdate { finalized_source_block } => {
                self.finalized_handler.handle(finalized_source_block, &mut self.state).await
            }
            ChainEvent::CrossUnsafeUpdate { block } => {
                self.cross_unsafe_handler.handle(block, &mut self.state).await
            }
            ChainEvent::CrossSafeUpdate { derived_ref_pair } => {
                self.cross_safe_handler.handle(derived_ref_pair, &mut self.state).await
            }
        };

        if let Err(err) = result {
            debug!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                %err,
                ?event,
                "Failed to process event"
            );
        }
    }
}
