use super::{
    Metrics,
    handlers::{
        CrossSafeHandler, CrossUnsafeHandler, EventHandler, FinalizedHandler, InvalidationHandler,
        OriginHandler, ReplacementHandler, SafeBlockHandler, UnsafeBlockHandler,
    },
};
use crate::{
    ChainProcessorError, ChainRewinder, LogIndexer, ProcessorState, config::RollupConfig,
    event::ChainEvent, syncnode::ManagedNodeProvider,
};
use alloy_primitives::ChainId;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{
    DerivationStorage, HeadRefStorageWriter, LogStorage, StorageRewinder,
};
use std::{fmt::Debug, sync::Arc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Represents a task that processes chain events from a managed node.
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessorTask<P, W> {
    chain_id: ChainId,
    metrics_enabled: Option<bool>,
    cancel_token: CancellationToken,
    /// The channel for receiving node events.
    event_rx: mpsc::Receiver<ChainEvent>,

    // state
    state: Arc<ProcessorState>,

    // Handlers for different types of chain events.
    unsafe_handler: UnsafeBlockHandler<P, W>,
    safe_handler: SafeBlockHandler<P, W>,
    origin_handler: OriginHandler<P, W>,
    invalidation_handler: InvalidationHandler<P, W>,
    replacement_handler: ReplacementHandler<P, W>,
    finalized_handler: FinalizedHandler<P, W>,
    cross_unsafe_handler: CrossUnsafeHandler<P>,
    cross_safe_handler: CrossSafeHandler<P>,
}

impl<P, W> ChainProcessorTask<P, W>
where
    P: ManagedNodeProvider + 'static,
    W: LogStorage + DerivationStorage + HeadRefStorageWriter + StorageRewinder + 'static,
{
    /// Creates a new [`ChainProcessorTask`].
    pub fn new(
        rollup_config: RollupConfig,
        chain_id: ChainId,
        managed_node: Arc<P>,
        state_manager: Arc<W>,
        cancel_token: CancellationToken,
        event_rx: mpsc::Receiver<ChainEvent>,
    ) -> Self {
        let state = Arc::new(ProcessorState::new());
        let log_indexer =
            Arc::new(LogIndexer::new(chain_id, managed_node.clone(), state_manager.clone()));
        let rewinder = Arc::new(ChainRewinder::new(chain_id, state_manager.clone()));

        let unsafe_handler = UnsafeBlockHandler::new(
            rollup_config.clone(),
            chain_id,
            state_manager.clone(),
            log_indexer.clone(),
        );

        let safe_handler = SafeBlockHandler::new(
            rollup_config,
            chain_id,
            managed_node.clone(),
            state_manager.clone(),
            log_indexer.clone(),
            rewinder.clone(),
        );

        let origin_handler =
            OriginHandler::new(chain_id, managed_node.clone(), state_manager.clone());

        let invalidation_handler =
            InvalidationHandler::new(chain_id, managed_node.clone(), state_manager.clone());

        let replacement_handler =
            ReplacementHandler::new(chain_id, log_indexer.clone(), state_manager.clone());

        let finalized_handler =
            FinalizedHandler::new(chain_id, managed_node.clone(), state_manager.clone());

        let cross_unsafe_handler = CrossUnsafeHandler::new(chain_id, managed_node.clone());
        let cross_safe_handler = CrossSafeHandler::new(chain_id, managed_node.clone());

        Self {
            chain_id,
            metrics_enabled: None,
            cancel_token,
            event_rx,

            state,

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

    /// Observes an async call, recording metrics and latency for block processing.
    /// The latecy is calculated as the difference between the current system time and the block's
    /// timestamp.
    async fn observe_block_processing<Fut, F>(
        &self,
        event_type: &'static str,
        f: F,
    ) -> Result<BlockInfo, ChainProcessorError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<BlockInfo, ChainProcessorError>>,
    {
        let result = f().await;

        if !self.metrics_enabled.unwrap_or(false) {
            return result;
        }

        match &result {
            Ok(block) => {
                metrics::counter!(
                    Metrics::BLOCK_PROCESSING_SUCCESS_TOTAL,
                    "type" => event_type,
                    "chain_id" => self.chain_id.to_string()
                )
                .increment(1);

                // Calculate elapsed time for block processing
                let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => duration.as_secs_f64(),
                    Err(e) => {
                        error!(
                            target: "chain_processor",
                            chain_id = self.chain_id,
                            "SystemTime error when recording block processing latency: {e}"
                        );
                        return result;
                    }
                };

                let latency = now - block.timestamp as f64;
                metrics::histogram!(
                    Metrics::BLOCK_PROCESSING_LATENCY_SECONDS,
                    "type" => event_type,
                    "chain_id" => self.chain_id.to_string()
                )
                .record(latency);
            }
            Err(_err) => {
                metrics::counter!(
                    Metrics::BLOCK_PROCESSING_ERROR_TOTAL,
                    "type" => event_type,
                    "chain_id" => self.chain_id.to_string()
                )
                .increment(1);
            }
        }

        result
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
                        target: "chain_processor",
                        chain_id = self.chain_id,
                        "ChainProcessorTask cancellation requested, stopping..."
                    );
                    break;
                }
            }
        }
    }

    async fn handle_event(&self, event: ChainEvent) {
        use ChainEvent::*;
        let state = self.state.clone();

        let result = match event {
            UnsafeBlock { block } => self.unsafe_handler.handle(block, state).await,
            DerivedBlock { derived_ref_pair } => {
                self.safe_handler.handle(derived_ref_pair, state).await
            }
            DerivationOriginUpdate { origin } => self.origin_handler.handle(origin, state).await,
            InvalidateBlock { block } => self.invalidation_handler.handle(block, state).await,
            BlockReplaced { replacement } => {
                self.replacement_handler.handle(replacement, state).await
            }
            FinalizedSourceUpdate { finalized_source_block } => {
                self.finalized_handler.handle(finalized_source_block, state).await
            }
            CrossUnsafeUpdate { block } => self.cross_unsafe_handler.handle(block, state).await,
            CrossSafeUpdate { derived_ref_pair } => {
                self.cross_safe_handler.handle(derived_ref_pair, state).await
            }
        };

        if let Err(err) = result {
            error!(
                target: "chain_processor",
                chain_id = self.chain_id,
                %err,
                ?event,
                "Failed to process event"
            );
        }
    }
}
