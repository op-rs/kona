use crate::{CrossSafetyError, event::ChainEvent, safety_checker::CrossSafetyChecker};
use alloy_primitives::ChainId;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::CrossChainSafetyProvider;
use op_alloy_consensus::interop::SafetyLevel;
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// A background job that promotes blocks to a target safety level on a given chain.
///
/// It uses [`CrossChainSafetyProvider`] to fetch candidate blocks and the [`CrossSafetyChecker`]
/// to validate cross-chain message dependencies.
#[derive(Debug)]
pub struct CrossSafetyCheckerJob<P> {
    chain_id: ChainId,
    provider: Arc<P>,
    cancel_token: CancellationToken,
    interval: Duration,
    target_level: SafetyLevel,
    target_level_lower_bound: SafetyLevel,
    event_tx: mpsc::Sender<ChainEvent>,
    current_head: Mutex<Option<BlockInfo>>,
}

impl<P> CrossSafetyCheckerJob<P>
where
    P: CrossChainSafetyProvider + Send + Sync + 'static,
{
    /// Initializes the [`CrossSafetyCheckerJob`]
    pub fn new(
        chain_id: ChainId,
        provider: Arc<P>,
        cancel_token: CancellationToken,
        interval: Duration,
        target_level: SafetyLevel,
        event_tx: mpsc::Sender<ChainEvent>,
    ) -> Result<Self, CrossSafetyError> {
        let target_level_lower_bound = match target_level {
            SafetyLevel::CrossUnsafe => SafetyLevel::LocalUnsafe,
            SafetyLevel::CrossSafe => SafetyLevel::LocalSafe,
            // other target level is out of the scope of this module and handled separately.
            _ => return Err(CrossSafetyError::UnsupportedTargetLevel(target_level)),
        };
        Ok(Self {
            chain_id,
            provider,
            cancel_token,
            interval,
            target_level,
            target_level_lower_bound,
            event_tx,
            current_head: Mutex::new(None),
        })
    }

    /// Runs the job loop until cancelled, promoting blocks to the target [`SafetyLevel`].
    ///
    /// On each iteration:
    /// - Tries to promote the next eligible block
    /// - Waits for configured interval if promotion fails
    /// - Exits when [`CancellationToken`] is triggered
    pub async fn run(&self) {
        info!(
            target: "safety_checker",
            chain_id = self.chain_id,
            target_level = %self.target_level,
            "Started safety checker");

        let checker = CrossSafetyChecker::new(&*self.provider);

        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    info!(target: "safety_checker", chain_id = self.chain_id,target_level = %self.target_level, "Canceled safety checker");
                    break;
                }

                _ = async {
                    match self.promote_next_block(&checker).await {
                        Ok(block_info) => {
                            info!(
                                target: "safety_checker",
                                chain_id = self.chain_id,
                                target_level = %self.target_level,
                                %block_info,
                                "Promoted next candidate block"
                            );
                            // Update the local head in case of success
                            self.update_current_head(Option::from(block_info)).await;
                        }
                        Err(err) => {
                            match err {
                                 // don't spam warnings if head is already on top - nothing to promote
                                CrossSafetyError::NoBlockToPromote => {},
                                _ => {
                                    warn!(
                                        target: "safety_checker",
                                        chain_id = self.chain_id,
                                        target_level = %self.target_level,
                                        %err,
                                        "Error promoting next candidate block"
                                    );
                                }
                            }
                            tokio::time::sleep(self.interval).await;

                            // Reset the current head in case of error - this will prevent re-org inconsistency.
                            self.update_current_head(None).await;
                        }
                    }
                } => {}
            }
        }

        info!(target: "safety_checker", chain_id = self.chain_id, target_level = %self.target_level, "Stopped safety checker");
    }

    async fn update_current_head(&self, head: Option<BlockInfo>) {
        let mut current_head = self.current_head.lock().await;
        *current_head = head;
    }

    async fn get_current_head(&self) -> Option<BlockInfo> {
        let current_head = self.current_head.lock().await;
        *current_head
    }

    // Attempts to promote the next block at the target safety level,
    // after validating cross-chain dependencies.
    async fn promote_next_block(
        &self,
        checker: &CrossSafetyChecker<'_, P>,
    ) -> Result<BlockInfo, CrossSafetyError> {
        let candidate = self.find_next_promotable_block().await?;

        checker.verify_block_dependencies(self.chain_id, candidate, self.target_level)?;

        // TODO: Add more checks in future

        self.broadcast_event(candidate);

        Ok(candidate)
    }

    // Finds the next block that is eligible for promotion at the configured target level.
    async fn find_next_promotable_block(&self) -> Result<BlockInfo, CrossSafetyError> {
        let current_head = match self.get_current_head().await {
            Some(head) => head,
            None => self.provider.get_safety_head_ref(self.chain_id, self.target_level)?,
        };

        let upper_head =
            self.provider.get_safety_head_ref(self.chain_id, self.target_level_lower_bound)?;

        if current_head.number >= upper_head.number {
            return Err(CrossSafetyError::NoBlockToPromote);
        }

        let candidate = self.provider.get_block(self.chain_id, current_head.number + 1)?;

        Ok(candidate)
    }

    fn broadcast_event(&self, block_info: BlockInfo) {
        let event = match self.target_level {
            SafetyLevel::CrossUnsafe => Some(ChainEvent::CrossUnsafeUpdate { block: block_info }),
            SafetyLevel::CrossSafe => Some(ChainEvent::CrossSafeUpdate { block: block_info }),
            _ => None,
        };

        if let Some(event) = event {
            if let Err(err) = self.event_tx.try_send(event) {
                error!(
                    target: "safety_checker",
                    target_level = %self.target_level,
                    %err,
                    "Failed to broadcast event",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{B256, ChainId};
    use kona_supervisor_storage::{CrossChainSafetyProvider, StorageError};
    use kona_supervisor_types::Log;
    use mockall::mock;
    use op_alloy_consensus::interop::SafetyLevel;

    mock! {
        #[derive(Debug)]
        pub Provider {}

        impl CrossChainSafetyProvider for Provider {
            fn get_block(&self, chain_id: ChainId, block_number: u64) -> Result<BlockInfo, StorageError>;
            fn get_block_logs(&self, chain_id: ChainId, block_number: u64) -> Result<Vec<Log>, StorageError>;
            fn get_safety_head_ref(&self, chain_id: ChainId, level: SafetyLevel) -> Result<BlockInfo, StorageError>;
        }
    }

    fn b256(n: u64) -> B256 {
        let mut bytes = [0u8; 32];
        bytes[24..].copy_from_slice(&n.to_be_bytes());
        B256::from(bytes)
    }

    fn block(n: u64) -> BlockInfo {
        BlockInfo { number: n, hash: b256(n), parent_hash: b256(n - 1), timestamp: 0 }
    }

    #[tokio::test]
    async fn promotes_next_cross_unsafe_successfully() {
        let chain_id = 1;
        let mut mock = MockProvider::default();
        let (event_tx, mut event_rx) = mpsc::channel::<ChainEvent>(10);

        mock.expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == chain_id && *lvl == SafetyLevel::CrossUnsafe)
            .returning(|_, _| Ok(block(99)));

        mock.expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == chain_id && *lvl == SafetyLevel::LocalUnsafe)
            .returning(|_, _| Ok(block(100)));

        mock.expect_get_block()
            .withf(move |cid, num| *cid == chain_id && *num == 100)
            .returning(|_, _| Ok(block(100)));

        mock.expect_get_block_logs()
            .withf(move |cid, num| *cid == chain_id && *num == 100)
            .returning(|_, _| Ok(vec![]));

        let job = CrossSafetyCheckerJob::new(
            chain_id,
            Arc::new(mock),
            CancellationToken::new(),
            Duration::from_secs(1),
            SafetyLevel::CrossUnsafe,
            event_tx,
        )
        .expect("error initializing cross-safety checker job");

        let checker = CrossSafetyChecker::new(&*job.provider);
        let result = job.promote_next_block(&checker).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().number, 100);

        // Receive and assert the correct event
        let received_event = event_rx.recv().await.expect("expected event not received");

        assert_eq!(received_event, ChainEvent::CrossUnsafeUpdate { block: block(100) });
    }

    #[tokio::test]
    async fn promotes_next_cross_unsafe_failed_with_no_candidates() {
        let chain_id = 1;
        let mut mock = MockProvider::default();
        let (event_tx, _) = mpsc::channel::<ChainEvent>(10);

        mock.expect_get_safety_head_ref()
            .withf(|_, lvl| *lvl == SafetyLevel::CrossSafe)
            .returning(|_, _| Ok(block(200)));

        mock.expect_get_safety_head_ref()
            .withf(|_, lvl| *lvl == SafetyLevel::LocalSafe)
            .returning(|_, _| Ok(block(200)));

        let job = CrossSafetyCheckerJob::new(
            chain_id,
            Arc::new(mock),
            CancellationToken::new(),
            Duration::from_secs(1),
            SafetyLevel::CrossSafe,
            event_tx,
        )
        .expect("error initializing cross-safety checker job");

        let checker = CrossSafetyChecker::new(&*job.provider);
        let result = job.promote_next_block(&checker).await;

        assert!(matches!(result, Err(CrossSafetyError::NoBlockToPromote)));
    }

    #[test]
    fn returns_unsupported_target_level_error() {
        let chain_id = 1;
        let mock = MockProvider::default();
        let (event_tx, _) = mpsc::channel::<ChainEvent>(10);

        let err = CrossSafetyCheckerJob::new(
            chain_id,
            Arc::new(mock),
            CancellationToken::new(),
            Duration::from_secs(1),
            SafetyLevel::Finalized, // unsupported
            event_tx,
        )
        .unwrap_err();
        assert!(matches!(err, CrossSafetyError::UnsupportedTargetLevel(_)));
    }
}
