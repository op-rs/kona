use super::Metrics;
use crate::{
    ChainProcessorError, ChainRewinder, LogIndexer, config::RollupConfig, event::ChainEvent,
    syncnode::ManagedNodeProvider,
};
use alloy_primitives::ChainId;
use kona_interop::{BlockReplacement, DerivedRefPair};
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{
    DerivationStorage, HeadRefStorageWriter, LogStorage, StorageError, StorageRewinder,
};
use kona_supervisor_types::BlockSeal;
use std::{fmt::Debug, sync::Arc};
use tokio::sync::{RwLock, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace};

/// Represents a task that processes chain events from a managed node.
/// It listens for events emitted by the managed node and handles them accordingly.
#[derive(Debug)]
pub struct ChainProcessorTask<P, W> {
    rollup_config: RollupConfig,
    chain_id: ChainId,
    metrics_enabled: Option<bool>,

    managed_node: Arc<P>,

    state_manager: Arc<W>,

    log_indexer: Arc<LogIndexer<P, W>>,

    rewinder: Arc<ChainRewinder<W>>,

    cancel_token: CancellationToken,

    /// The channel for receiving node events.
    event_rx: mpsc::Receiver<ChainEvent>,

    invalidated_block: RwLock<Option<DerivedRefPair>>,
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
        let log_indexer = LogIndexer::new(chain_id, managed_node.clone(), state_manager.clone());
        let rewinder = ChainRewinder::new(chain_id, state_manager.clone());

        Self {
            rollup_config,
            chain_id,
            metrics_enabled: None,
            cancel_token,
            managed_node,
            event_rx,
            state_manager,
            log_indexer: Arc::from(log_indexer),
            invalidated_block: RwLock::new(None),
            rewinder: Arc::from(rewinder),
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
                            target: "supervisor::chain_processor",
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
                        target: "supervisor::chain_processor",
                        chain_id = self.chain_id,
                        "ChainProcessorTask cancellation requested, stopping..."
                    );
                    break;
                }
            }
        }
    }

    async fn handle_event(&self, event: ChainEvent) {
        match event {
            ChainEvent::UnsafeBlock { block } => {
                let _ = self
                    .observe_block_processing("local_unsafe", || async {
                        self.handle_unsafe_event(block).await.inspect_err(|err| {
                            error!(
                                target: "supervisor::chain_processor",
                                chain_id = self.chain_id,
                                block_number = block.number,
                                %err,
                                "Failed to process unsafe block"
                            );
                        })
                    })
                    .await;
            }
            ChainEvent::DerivedBlock { derived_ref_pair } => {
                let _ = self
                    .observe_block_processing("local_safe", || async {
                        self.handle_safe_event(derived_ref_pair).await.inspect_err(|err| {
                            error!(
                                target: "supervisor::chain_processor",
                                chain_id = self.chain_id,
                                block_number = derived_ref_pair.derived.number,
                                %err,
                                "Failed to process local safe derived block pair"
                            );
                        })
                    })
                    .await;
            }
            ChainEvent::DerivationOriginUpdate { origin } => {
                let _ = self.handle_derivation_origin_update(origin).await.inspect_err(|err| {
                    error!(
                        target: "supervisor::chain_processor",
                        chain_id = self.chain_id,
                        block_number = origin.number,
                        %err,
                        "Failed to update derivation origin"
                    );
                });
            }
            ChainEvent::InvalidateBlock { block } => {
                let _ = self.handle_invalidate_block(block).await.inspect_err(|err| {
                    error!(
                        target: "supervisor::chain_processor",
                        chain_id = self.chain_id,
                        block_number = block.number,
                        %err,
                        "Failed to invalidate block"
                    );
                });
            }
            ChainEvent::BlockReplaced { replacement } => {
                let _ = self.handle_block_replacement(replacement).await.inspect_err(|err| {
                    error!(
                        target: "supervisor::chain_processor",
                        chain_id = self.chain_id,
                        %err,
                        "Failed to handle block replacement"
                    );
                });
            }
            ChainEvent::FinalizedSourceUpdate { finalized_source_block } => {
                let _ = self
                    .observe_block_processing("finalized", || async {
                        self.handle_finalized_l1_update(finalized_source_block).await.inspect_err(
                            |err| {
                                error!(
                                    target: "supervisor::chain_processor",
                                    chain_id = self.chain_id,
                                    block_number = finalized_source_block.number,
                                    %err,
                                    "Failed to process finalized source update"
                                );
                            },
                        )
                    })
                    .await;
            }
            ChainEvent::CrossUnsafeUpdate { block } => {
                let _ = self
                    .observe_block_processing("cross_unsafe", || async {
                        self.handle_cross_unsafe_update(block).await.inspect_err(|err| {
                            error!(
                                target: "supervisor::chain_processor",
                                chain_id = self.chain_id,
                                block_number = block.number,
                                %err,
                                "Failed to process cross unsafe update"
                            );
                        })
                    })
                    .await;
            }
            ChainEvent::CrossSafeUpdate { derived_ref_pair } => {
                let _ = self
                    .observe_block_processing("cross_safe", || async {
                        self.handle_cross_safe_update(derived_ref_pair).await.inspect_err(|err| {
                            error!(
                                target: "supervisor::chain_processor",
                                chain_id = self.chain_id,
                                block_number = derived_ref_pair.derived.number,
                                %err,
                                "Failed to process cross safe update"
                            );
                        })
                    })
                    .await;
            }
        }
    }

    async fn handle_block_replacement(
        &self,
        replacement: BlockReplacement,
    ) -> Result<(), ChainProcessorError> {
        debug!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            %replacement,
            "Handling block replacement"
        );

        let mut guard = self.invalidated_block.write().await;
        // check if invalidated block is same as replacement block
        if let Some(invalidated_ref_pair) = *guard {
            if invalidated_ref_pair.derived.hash != replacement.invalidated {
                debug!(
                    target: "supervisor::chain_processor",
                    chain_id = self.chain_id,
                    invalidated_block = %invalidated_ref_pair.derived,
                    replacement_block = %replacement.replacement,
                    "Replacement block does not match invalidated block, skipping"
                );
                return Ok(());
            }

            // save the derived block
            let derived_ref_pair = DerivedRefPair {
                source: invalidated_ref_pair.source,
                derived: replacement.replacement,
            };
            self.retry_with_resync_derived_block(derived_ref_pair).await?;
            *guard = None;
            return Ok(());
        }
        Ok(())
    }

    async fn handle_invalidate_block(&self, block: BlockInfo) -> Result<(), ChainProcessorError> {
        debug!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            invalidated_block = %block,
            "Processing invalidate block"
        );

        let mut invalidated_block = self.invalidated_block.write().await;
        if invalidated_block.is_some() {
            debug!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                block_number = block.number,
                "Invalidated block already set, skipping"
            );
            return Ok(());
        }

        let source_block = self.state_manager.derived_to_source(block.id())?;

        // rewind the storage to the block before the invalidated block
        let to = block.id();
        self.state_manager.rewind(&to)?;

        let block_seal = BlockSeal::new(block.hash, block.number, block.timestamp);
        self.managed_node.invalidate_block(block_seal).await?;

        *invalidated_block = Some(DerivedRefPair { source: source_block, derived: block });
        Ok(())
    }

    async fn handle_finalized_l1_update(
        &self,
        finalized_source_block: BlockInfo,
    ) -> Result<BlockInfo, ChainProcessorError> {
        debug!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = finalized_source_block.number,
            "Processing finalized L1 update"
        );
        let finalized_derived_block =
            self.state_manager.update_finalized_using_source(finalized_source_block)?;
        self.managed_node.update_finalized(finalized_derived_block.id()).await?;
        Ok(finalized_derived_block)
    }

    async fn handle_derivation_origin_update(
        &self,
        origin: BlockInfo,
    ) -> Result<(), ChainProcessorError> {
        debug!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = origin.number,
            "Processing derivation origin update"
        );

        let invalidated_block = self.invalidated_block.read().await;
        if invalidated_block.is_some() {
            trace!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                block_number = origin.number,
                "Invalidated block set, skipping derivation origin update"
            );
            return Ok(());
        }

        match self.state_manager.save_source_block(origin) {
            Ok(_) => Ok(()),
            Err(StorageError::BlockOutOfOrder | StorageError::ConflictError) => {
                error!(
                    target: "supervisor::chain_processor",
                    chain_id = self.chain_id,
                    block_number = origin.number,
                    "Source block out of order detected, resetting managed node"
                );

                if let Err(err) = self.managed_node.reset().await {
                    error!(
                        target: "supervisor::chain_processor",
                        chain_id = self.chain_id,
                        %err,
                        "Failed to reset managed node after block out of order"
                    );
                }
                Err(StorageError::BlockOutOfOrder.into())
            }
            Err(err) => Err(err.into()),
        }
    }

    async fn handle_safe_event(
        &self,
        derived_ref_pair: DerivedRefPair,
    ) -> Result<BlockInfo, ChainProcessorError> {
        debug!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = derived_ref_pair.derived.number,
            "Processing local safe derived block pair"
        );

        let invalidated_block = self.invalidated_block.read().await;
        if invalidated_block.is_some() {
            trace!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                block_number = derived_ref_pair.derived.number,
                "Invalidated block already set, skipping safe event processing"
            );
            return Ok(derived_ref_pair.derived);
        }

        if self.rollup_config.is_post_interop(derived_ref_pair.derived.timestamp) {
            return self.process_safe_derived_block(derived_ref_pair).await
        }

        if self.rollup_config.is_interop_activation_block(derived_ref_pair.derived) {
            info!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                block_number = derived_ref_pair.derived.number,
                "Initialising derivation storage for interop activation block"
            );
            self.state_manager.initialise_derivation_storage(derived_ref_pair)?;
            return Ok(derived_ref_pair.derived);
        }

        Ok(derived_ref_pair.derived)
    }

    async fn process_safe_derived_block(
        &self,
        derived_ref_pair: DerivedRefPair,
    ) -> Result<BlockInfo, ChainProcessorError> {
        match self.state_manager.save_derived_block(derived_ref_pair) {
            Ok(_) => Ok(derived_ref_pair.derived),
            Err(StorageError::BlockOutOfOrder) => {
                error!(
                    target: "supervisor::chain_processor",
                    chain_id = self.chain_id,
                    block_number = derived_ref_pair.derived.number,
                    "Block out of order detected, resetting managed node"
                );

                if let Err(err) = self.managed_node.reset().await {
                    error!(
                        target: "supervisor::chain_processor",
                        chain_id = self.chain_id,
                        %err,
                        "Failed to reset managed node after block out of order"
                    );
                }
                Err(StorageError::BlockOutOfOrder.into())
            }

            Err(StorageError::ReorgRequired) => {
                debug!(
                    target: "supervisor::chain_processor",
                    chain = self.chain_id,
                    derived_block = %derived_ref_pair.derived,
                    "Local derivation conflict detected — rewinding"
                );
                self.rewinder.handle_local_reorg(&derived_ref_pair)?;
                self.retry_with_resync_derived_block(derived_ref_pair).await
            }

            Err(StorageError::FutureData) => {
                self.retry_with_resync_derived_block(derived_ref_pair).await
            }

            Err(err) => {
                error!(
                    target: "supervisor::chain_processor",
                    chain_id = self.chain_id,
                    block_number = derived_ref_pair.derived.number,
                    %err,
                    "Failed to save derived block pair"
                );
                Err(err.into())
            }
        }
    }

    async fn retry_with_resync_derived_block(
        &self,
        derived_ref_pair: DerivedRefPair,
    ) -> Result<BlockInfo, ChainProcessorError> {
        self.log_indexer
            .clone()
            .process_and_store_logs(&derived_ref_pair.derived)
            .await
            .inspect_err(|err| {
                error!(
                    target: "supervisor::chain_processor",
                    chain_id = self.chain_id,
                    block_number = derived_ref_pair.derived.number,
                    %err,
                    "Error resyncing logs for derived block"
                );
            })?;
        self.state_manager.save_derived_block(derived_ref_pair).inspect_err(|err| {
            error!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                block_number = derived_ref_pair.derived.number,
                %err,
                "Error saving derived block after resync"
            );
        })?;

        Ok(derived_ref_pair.derived)
    }

    async fn handle_unsafe_event(
        &self,
        block: BlockInfo,
    ) -> Result<BlockInfo, ChainProcessorError> {
        debug!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = block.number,
            "Processing unsafe block"
        );

        let invalidated_block = self.invalidated_block.read().await;
        if invalidated_block.is_some() {
            trace!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                block_number = block.number,
                "Invalidated block already set, skipping unsafe event processing"
            );
            return Ok(block);
        }

        if self.rollup_config.is_post_interop(block.timestamp) {
            self.log_indexer.clone().sync_logs(block);
            return Ok(block);
        }

        if self.rollup_config.is_interop_activation_block(block) {
            info!(
                target: "supervisor::chain_processor",
                chain_id = self.chain_id,
                block_number = block.number,
                "Initialising log storage for interop activation block"
            );
            self.state_manager.initialise_log_storage(block)?;
            return Ok(block);
        }

        Ok(block)
    }

    async fn handle_cross_unsafe_update(
        &self,
        block: BlockInfo,
    ) -> Result<BlockInfo, ChainProcessorError> {
        debug!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = block.number,
            "Processing cross unsafe update"
        );

        self.managed_node.update_cross_unsafe(block.id()).await?;
        Ok(block)
    }

    async fn handle_cross_safe_update(
        &self,
        derived_ref_pair: DerivedRefPair,
    ) -> Result<BlockInfo, ChainProcessorError> {
        debug!(
            target: "supervisor::chain_processor",
            chain_id = self.chain_id,
            block_number = derived_ref_pair.derived.number,
            "Processing cross safe update"
        );

        self.managed_node
            .update_cross_safe(derived_ref_pair.source.id(), derived_ref_pair.derived.id())
            .await?;
        Ok(derived_ref_pair.derived)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::Genesis,
        event::ChainEvent,
        syncnode::{
            AuthenticationError, BlockProvider, ClientError, ManagedNodeController,
            ManagedNodeDataProvider, ManagedNodeError, NodeSubscriber,
        },
    };
    use alloy_primitives::B256;
    use alloy_rpc_types_eth::BlockNumHash;
    use async_trait::async_trait;
    use kona_interop::DerivedRefPair;
    use kona_protocol::BlockInfo;
    use kona_supervisor_storage::{
        DerivationStorageReader, DerivationStorageWriter, HeadRefStorageWriter, LogStorageReader,
        LogStorageWriter, StorageError,
    };
    use kona_supervisor_types::{BlockSeal, Log, OutputV0, Receipts};
    use mockall::mock;
    use std::time::Duration;
    use tokio::sync::mpsc;

    mock!(
        #[derive(Debug)]
        pub Node {}

        #[async_trait]
        impl NodeSubscriber for Node {
            async fn start_subscription(
                &self,
                _event_tx: mpsc::Sender<ChainEvent>,
            ) -> Result<(), ManagedNodeError>;
        }

        #[async_trait]
        impl BlockProvider for Node {
            async fn fetch_receipts(&self, _block_hash: B256) -> Result<Receipts, ManagedNodeError>;
            async fn block_by_number(&self, _number: u64) -> Result<BlockInfo, ManagedNodeError>;
        }

        #[async_trait]
        impl ManagedNodeDataProvider for Node {
            async fn output_v0_at_timestamp(
                &self,
                _timestamp: u64,
            ) -> Result<OutputV0, ManagedNodeError>;

            async fn pending_output_v0_at_timestamp(
                &self,
                _timestamp: u64,
            ) -> Result<OutputV0, ManagedNodeError>;

            async fn l2_block_ref_by_timestamp(
                &self,
                _timestamp: u64,
            ) -> Result<BlockInfo, ManagedNodeError>;
        }

        #[async_trait]
        impl ManagedNodeController for Node {
            async fn update_finalized(
                &self,
                _finalized_block_id: BlockNumHash,
            ) -> Result<(), ManagedNodeError>;

            async fn update_cross_unsafe(
                &self,
                cross_unsafe_block_id: BlockNumHash,
            ) -> Result<(), ManagedNodeError>;

            async fn update_cross_safe(
                &self,
                source_block_id: BlockNumHash,
                derived_block_id: BlockNumHash,
            ) -> Result<(), ManagedNodeError>;

            async fn reset(&self) -> Result<(), ManagedNodeError>;

            async fn invalidate_block(&self, seal: BlockSeal) -> Result<(), ManagedNodeError>;
        }
    );

    mock!(
        #[derive(Debug)]
        pub Db {}

        impl LogStorageWriter for Db {
            fn initialise_log_storage(
                &self,
                block: BlockInfo,
            ) -> Result<(), StorageError>;

            fn store_block_logs(
                &self,
                block: &BlockInfo,
                logs: Vec<Log>,
            ) -> Result<(), StorageError>;
        }

        impl LogStorageReader for Db {
            fn get_block(&self, block_number: u64) -> Result<BlockInfo, StorageError>;
            fn get_latest_block(&self) -> Result<BlockInfo, StorageError>;
            fn get_log(&self,block_number: u64,log_index: u32) -> Result<Log, StorageError>;
            fn get_logs(&self, block_number: u64) -> Result<Vec<Log>, StorageError>;
        }

        impl DerivationStorageReader for Db {
            fn derived_to_source(&self, derived_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derived_block_at_source(&self, source_block_id: BlockNumHash) -> Result<BlockInfo, StorageError>;
            fn latest_derivation_state(&self) -> Result<DerivedRefPair, StorageError>;
        }

        impl DerivationStorageWriter for Db {
            fn initialise_derivation_storage(
                &self,
                incoming_pair: DerivedRefPair,
            ) -> Result<(), StorageError>;

            fn save_derived_block(
                &self,
                incoming_pair: DerivedRefPair,
            ) -> Result<(), StorageError>;

            fn save_source_block(
                &self,
                source: BlockInfo,
            ) -> Result<(), StorageError>;
        }

        impl HeadRefStorageWriter for Db {
            fn update_finalized_using_source(
                &self,
                block_info: BlockInfo,
            ) -> Result<BlockInfo, StorageError>;

            fn update_current_cross_unsafe(
                &self,
                block: &BlockInfo,
            ) -> Result<(), StorageError>;

            fn update_current_cross_safe(
                &self,
                block: &BlockInfo,
            ) -> Result<DerivedRefPair, StorageError>;
        }

        impl StorageRewinder for Db {
            fn rewind_log_storage(&self, to: &BlockNumHash) -> Result<(), StorageError>;
            fn rewind(&self, to: &BlockNumHash) -> Result<(), StorageError>;
        }
    );

    fn genesis() -> Genesis {
        let l2 = BlockInfo::new(B256::from([1u8; 32]), 0, B256::ZERO, 50);
        let l1 = BlockInfo::new(B256::from([2u8; 32]), 10, B256::ZERO, 1000);
        Genesis::new(l1, l2)
    }

    fn get_rollup_config(interop_time: u64) -> RollupConfig {
        RollupConfig::new(genesis(), 2, Some(interop_time))
    }

    #[tokio::test]
    async fn test_handle_unsafe_event_pre_interop() {
        let mockdb = MockDb::new();
        let mocknode = MockNode::new();

        // Send unsafe block event
        let block = BlockInfo::new(B256::ZERO, 123, B256::ZERO, 10);

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);

        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        tx.send(ChainEvent::UnsafeBlock { block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_unsafe_event_post_interop() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();

        // Send unsafe block event
        let block = BlockInfo::new(B256::ZERO, 123, B256::ZERO, 1003);

        mockdb.expect_store_block_logs().returning(move |_block, _log| Ok(()));
        mocknode.expect_fetch_receipts().returning(move |block_hash| {
            assert!(block_hash == block.hash);
            Ok(Receipts::default())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);

        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        tx.send(ChainEvent::UnsafeBlock { block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_unsafe_event_interop_activation() {
        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();

        // Block that triggers interop activation
        let block = BlockInfo::new(B256::ZERO, 123, B256::ZERO, 1001); // Use timestamp/number that triggers activation

        let rollup_config = get_rollup_config(1000);

        mockdb.expect_initialise_log_storage().returning(move |b| {
            assert_eq!(b, block);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        tx.send(ChainEvent::UnsafeBlock { block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derived_event_pre_interop() {
        let block_pair = DerivedRefPair {
            source: BlockInfo {
                number: 123,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
            derived: BlockInfo {
                number: 1234,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 999,
            },
        };

        let mockdb = MockDb::new();
        let mocknode = MockNode::new();

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        // Send unsafe block event
        tx.send(ChainEvent::DerivedBlock { derived_ref_pair: block_pair }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derived_event_post_interop() {
        let block_pair = DerivedRefPair {
            source: BlockInfo {
                number: 123,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
            derived: BlockInfo {
                number: 1234,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 1003,
            },
        };

        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();

        mockdb.expect_save_derived_block().returning(move |_pair: DerivedRefPair| {
            assert_eq!(_pair, block_pair);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        // Send unsafe block event
        tx.send(ChainEvent::DerivedBlock { derived_ref_pair: block_pair }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derived_event_interop_activation() {
        let block_pair = DerivedRefPair {
            source: BlockInfo {
                number: 123,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
            derived: BlockInfo {
                number: 1234,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 1001,
            },
        };

        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();

        mockdb.expect_initialise_derivation_storage().returning(move |_pair: DerivedRefPair| {
            assert_eq!(_pair, block_pair);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);

        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        // Send unsafe block event
        tx.send(ChainEvent::DerivedBlock { derived_ref_pair: block_pair }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derived_event_block_out_of_order_triggers_reset() {
        let block_pair = DerivedRefPair {
            source: BlockInfo {
                number: 123,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
            derived: BlockInfo {
                number: 1234,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 1003, // post-interop
            },
        };

        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();

        // Simulate BlockOutOfOrder error
        mockdb
            .expect_save_derived_block()
            .returning(move |_pair: DerivedRefPair| Err(StorageError::BlockOutOfOrder));

        // Expect reset to be called
        mocknode.expect_reset().returning(|| Ok(()));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        tx.send(ChainEvent::DerivedBlock { derived_ref_pair: block_pair }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derived_event_block_triggers_reorg() {
        let block_pair = DerivedRefPair {
            source: BlockInfo {
                number: 123,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
            derived: BlockInfo {
                number: 1234,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 1003, // post-interop
            },
        };

        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();

        // Simulate ReorgRequired error
        mockdb
            .expect_save_derived_block()
            .returning(move |_pair: DerivedRefPair| Err(StorageError::ReorgRequired));

        mockdb.expect_get_block().returning(move |num| {
            Ok(BlockInfo {
                number: num,
                hash: B256::random(), // different hash from safe derived block
                parent_hash: B256::ZERO,
                timestamp: 1003, // post-interop
            })
        });

        // Expect reorg on log storage
        mockdb.expect_rewind_log_storage().returning(|_block_id| Ok(()));

        mockdb.expect_store_block_logs().returning(|_block_id, _logs| Ok(()));

        mocknode.expect_fetch_receipts().returning(|_receipts| Ok(Receipts::default()));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        tx.send(ChainEvent::DerivedBlock { derived_ref_pair: block_pair }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derived_event_block_triggers_resync() {
        let block_pair = DerivedRefPair {
            source: BlockInfo {
                number: 123,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
            derived: BlockInfo {
                number: 1234,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 1003, // post-interop
            },
        };

        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();

        // Simulate ReorgRequired error
        mockdb
            .expect_save_derived_block()
            .returning(move |_pair: DerivedRefPair| Err(StorageError::FutureData));

        mockdb.expect_get_block().returning(move |num| {
            Ok(BlockInfo {
                number: num,
                hash: B256::random(), // different hash from safe derived block
                parent_hash: B256::ZERO,
                timestamp: 1003, // post-interop
            })
        });

        mockdb.expect_store_block_logs().returning(|_block_id, _logs| Ok(()));

        mocknode.expect_fetch_receipts().returning(|_receipts| Ok(Receipts::default()));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        tx.send(ChainEvent::DerivedBlock { derived_ref_pair: block_pair }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derived_event_other_error() {
        let block_pair = DerivedRefPair {
            source: BlockInfo {
                number: 123,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 0,
            },
            derived: BlockInfo {
                number: 1234,
                hash: B256::ZERO,
                parent_hash: B256::ZERO,
                timestamp: 1003, // post-interop
            },
        };

        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();

        // Simulate a different error
        mockdb
            .expect_save_derived_block()
            .returning(move |_pair: DerivedRefPair| Err(StorageError::DatabaseNotInitialised));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = get_rollup_config(1000);
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        tx.send(ChainEvent::DerivedBlock { derived_ref_pair: block_pair }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_derivation_origin_update_triggers() {
        let origin =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();

        let origin_clone = origin;
        mockdb.expect_save_source_block().returning(move |block_info: BlockInfo| {
            assert_eq!(block_info, origin_clone);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        // Send derivation origin update event
        tx.send(ChainEvent::DerivationOriginUpdate { origin }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_finalized_source_update_triggers() {
        let finalized_source_block =
            BlockInfo { number: 99, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        let mut mocknode = MockNode::new();
        let mut mockdb = MockDb::new();

        // The finalized_derived_block returned by update_finalized_using_source
        let finalized_derived_block =
            BlockInfo { number: 5, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        // Expect update_finalized_using_source to be called with finalized_source_block
        mockdb.expect_update_finalized_using_source().returning(move |block_info: BlockInfo| {
            assert_eq!(block_info, finalized_source_block);
            Ok(finalized_derived_block)
        });

        // Expect update_finalized to be called with the derived block's id
        let finalized_derived_block_id = finalized_derived_block.id();
        mocknode.expect_update_finalized().returning(move |block_id| {
            assert_eq!(block_id, finalized_derived_block_id);
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        // Send FinalizedSourceUpdate event
        tx.send(ChainEvent::FinalizedSourceUpdate { finalized_source_block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_finalized_source_update_db_error() {
        let finalized_source_block =
            BlockInfo { number: 99, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 1234578 };

        let mut mocknode = MockNode::new();
        let mut mockdb = MockDb::new();

        // DB returns error
        mockdb
            .expect_update_finalized_using_source()
            .returning(|_block_info: BlockInfo| Err(StorageError::DatabaseNotInitialised));

        // Managed node's update_finalized should NOT be called
        mocknode.expect_update_finalized().never();

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        // Send FinalizedSourceUpdate event
        tx.send(ChainEvent::FinalizedSourceUpdate { finalized_source_block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_cross_unsafe_update_triggers() {
        let block =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        let mockdb = MockDb::new();
        let mut mocknode = MockNode::new();

        mocknode.expect_update_cross_unsafe().returning(move |cross_unsafe_block| {
            assert_eq!(cross_unsafe_block, block.id());
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        // Send derivation origin update event
        tx.send(ChainEvent::CrossUnsafeUpdate { block }).await.unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_cross_safe_update_triggers() {
        let derived =
            BlockInfo { number: 42, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };
        let source =
            BlockInfo { number: 1, hash: B256::ZERO, parent_hash: B256::ZERO, timestamp: 123456 };

        let mockdb = MockDb::new();
        let mut mocknode = MockNode::new();

        mocknode.expect_update_cross_safe().returning(move |source_id, derived_id| {
            assert_eq!(derived_id, derived.id());
            assert_eq!(source_id, source.id());
            Ok(())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);

        let cancel_token = CancellationToken::new();
        let (tx, rx) = mpsc::channel(10);

        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        // Send derivation origin update event
        tx.send(ChainEvent::CrossSafeUpdate {
            derived_ref_pair: DerivedRefPair { source, derived },
        })
        .await
        .unwrap();

        let task_handle = tokio::spawn(task.run());

        // Give it time to process
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Stop the task
        cancel_token.cancel();
        task_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_already_set_skips() {
        let mockdb = MockDb::new();
        let mocknode = MockNode::new();

        let block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);

        // Set up state: invalidated_block is already set
        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        let cancel_token = CancellationToken::new();
        let (_tx, rx) = mpsc::channel(10);
        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );
        {
            let mut guard = task.invalidated_block.write().await;
            *guard = Some(DerivedRefPair { source: block, derived: block });
        }

        let result = task.handle_invalidate_block(block).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_derived_to_source_error() {
        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);

        mockdb.expect_derived_to_source().returning(move |_id| Err(StorageError::FutureData));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        let cancel_token = CancellationToken::new();
        let (_tx, rx) = mpsc::channel(10);
        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        let result = task.handle_invalidate_block(block).await;
        assert!(matches!(result, Err(ChainProcessorError::StorageError(StorageError::FutureData))));

        // make sure invalidated_block is not set
        let guard = task.invalidated_block.read().await;
        assert!(guard.is_none());
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_rewind_error() {
        let mut mockdb = MockDb::new();
        let mocknode = MockNode::new();
        let block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);

        mockdb.expect_derived_to_source().returning(move |_id| Ok(block));
        mockdb.expect_rewind().returning(move |_to| Err(StorageError::DatabaseNotInitialised));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        let cancel_token = CancellationToken::new();
        let (_tx, rx) = mpsc::channel(10);
        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        let result = task.handle_invalidate_block(block).await;
        assert!(matches!(
            result,
            Err(ChainProcessorError::StorageError(StorageError::DatabaseNotInitialised))
        ));

        // make sure invalidated_block is not set
        let guard = task.invalidated_block.read().await;
        assert!(guard.is_none());
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_managed_node_error() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();
        let block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);

        mockdb.expect_derived_to_source().returning(move |_id| Ok(block));
        mockdb.expect_rewind().returning(move |_to| Ok(()));
        mocknode.expect_invalidate_block().returning(move |_seal| {
            Err(ManagedNodeError::ClientError(ClientError::Authentication(
                AuthenticationError::InvalidHeader,
            )))
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        let cancel_token = CancellationToken::new();
        let (_tx, rx) = mpsc::channel(10);
        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        let result = task.handle_invalidate_block(block).await;
        assert!(matches!(result, Err(ChainProcessorError::ManagedNode(_))));

        // make sure invalidated_block is not set
        let guard = task.invalidated_block.read().await;
        assert!(guard.is_none());
    }

    #[tokio::test]
    async fn test_handle_invalidate_block_success_sets_invalidated() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();
        let derived_block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);
        let source_block = BlockInfo::new(B256::from([2u8; 32]), 41, B256::ZERO, 12344);

        mockdb.expect_derived_to_source().returning(move |_id| Ok(source_block));
        mockdb.expect_rewind().returning(move |_to| Ok(()));
        mocknode.expect_invalidate_block().returning(move |_seal| Ok(()));

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        let cancel_token = CancellationToken::new();
        let (_tx, rx) = mpsc::channel(10);
        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        let result = task.handle_invalidate_block(derived_block).await;
        assert!(result.is_ok());

        // make sure invalidated_block is set
        let guard = task.invalidated_block.read().await;
        let pair = guard.as_ref().expect("invalidated_block should be set");
        assert_eq!(pair.derived, derived_block);
        assert_eq!(pair.source, source_block);
    }

    #[tokio::test]
    async fn test_handle_block_replacement_no_invalidated_block() {
        let mockdb = MockDb::new();
        let mocknode = MockNode::new();

        let replacement = BlockReplacement {
            invalidated: B256::from([1u8; 32]),
            replacement: BlockInfo::new(B256::from([2u8; 32]), 43, B256::ZERO, 12346),
        };

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        let cancel_token = CancellationToken::new();
        let (_tx, rx) = mpsc::channel(10);
        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );

        let result = task.handle_block_replacement(replacement).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_block_replacement_invalidated_hash_mismatch() {
        let mockdb = MockDb::new();
        let mocknode = MockNode::new();

        let invalidated_block = BlockInfo::new(B256::from([3u8; 32]), 42, B256::ZERO, 12345);
        let replacement = BlockReplacement {
            invalidated: B256::from([1u8; 32]), // does not match invalidated_block.hash
            replacement: BlockInfo::new(B256::from([2u8; 32]), 43, B256::ZERO, 12346),
        };

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        let cancel_token = CancellationToken::new();
        let (_tx, rx) = mpsc::channel(10);
        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );
        {
            let mut guard = task.invalidated_block.write().await;
            *guard = Some(DerivedRefPair { source: invalidated_block, derived: invalidated_block });
        }

        let result = task.handle_block_replacement(replacement).await;
        assert!(result.is_ok());

        // invalidated_block should remain set
        let guard = task.invalidated_block.read().await;
        assert!(guard.is_some());
    }

    #[tokio::test]
    async fn test_handle_block_replacement_success() {
        let mut mockdb = MockDb::new();
        let mut mocknode = MockNode::new();

        let source_block = BlockInfo::new(B256::from([1u8; 32]), 45, B256::ZERO, 12345);
        let invalidated_block = BlockInfo::new(B256::from([1u8; 32]), 42, B256::ZERO, 12345);
        let replacement_block = BlockInfo::new(B256::from([2u8; 32]), 42, B256::ZERO, 12346);

        mockdb.expect_save_derived_block().returning(move |_pair| Ok(()));
        mockdb.expect_store_block_logs().returning(move |_block, _logs| Ok(()));

        mocknode.expect_fetch_receipts().returning(move |_block_hash| {
            assert_eq!(_block_hash, replacement_block.hash);
            Ok(Receipts::default())
        });

        let writer = Arc::new(mockdb);
        let managed_node = Arc::new(mocknode);
        let cancel_token = CancellationToken::new();
        let (_tx, rx) = mpsc::channel(10);
        let rollup_config = RollupConfig::default();
        let task = ChainProcessorTask::new(
            rollup_config,
            1,
            managed_node,
            writer,
            cancel_token.clone(),
            rx,
        );
        {
            let mut guard = task.invalidated_block.write().await;
            *guard = Some(DerivedRefPair { source: source_block, derived: invalidated_block });
        }

        let replacement = BlockReplacement {
            invalidated: invalidated_block.hash,
            replacement: replacement_block,
        };

        let result = task.handle_block_replacement(replacement).await;
        assert!(result.is_ok());

        // invalidated_block should be cleared
        let guard = task.invalidated_block.read().await;
        assert!(guard.is_none());
    }
}
