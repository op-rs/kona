use std::{sync::Arc, time::Duration};

use crate::{CrossSafetyError, safety_checker::CrossSafetyChecker};
use alloy_primitives::ChainId;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::CrossChainSafetyProvider;
use op_alloy_consensus::interop::SafetyLevel;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

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
        })
    }

    /// Runs the job loop until cancelled, promoting blocks to the target [`SafetyLevel`].
    ///
    /// On each iteration:
    /// - Tries to promote the next eligible block
    /// - Waits for configured interval if promotion fails
    /// - Exits when [`CancellationToken`] is triggered
    pub async fn run(self) {
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
                    match self.promote_next_block(&checker) {
                        Ok(block_info) => {
                            info!(
                                target: "safety_checker",
                                chain_id = self.chain_id,
                                target_level = %self.target_level,
                                %block_info,
                                "Promoted next candidate block"
                            );
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
                        }
                    }
                } => {}
            }
        }

        info!(target: "safety_checker", chain_id = self.chain_id, target_level = %self.target_level, "Stopped safety checker");
    }

    // Attempts to promote the next block at the target safety level,
    // after validating cross-chain dependencies.
    fn promote_next_block(
        &self,
        checker: &CrossSafetyChecker<'_, P>,
    ) -> Result<BlockInfo, CrossSafetyError> {
        let candidate = self.find_next_promotable_block()?;

        checker.verify_block_dependencies(self.chain_id, candidate, self.target_level)?;

        // TODO: Add more checks in future

        self.provider.update_safety_head_ref(self.chain_id, self.target_level, &candidate)?;

        Ok(candidate)
    }

    // Finds the next block that is eligible for promotion at the configured target level.
    fn find_next_promotable_block(&self) -> Result<BlockInfo, CrossSafetyError> {
        let current_head = self.provider.get_safety_head_ref(self.chain_id, self.target_level)?;
        let upper_head =
            self.provider.get_safety_head_ref(self.chain_id, self.target_level_lower_bound)?;

        if current_head.number >= upper_head.number {
            return Err(CrossSafetyError::NoBlockToPromote);
        }

        let candidate = self.provider.get_block(self.chain_id, current_head.number + 1)?;

        Ok(candidate)
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
            fn update_safety_head_ref(&self, chain_id: ChainId, level: SafetyLevel, block: &BlockInfo) -> Result<(), StorageError>;
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

    #[test]
    fn promotes_next_cross_unsafe_successfully() {
        let chain_id = 1;
        let mut mock = MockProvider::default();

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

        mock.expect_update_safety_head_ref()
            .withf(move |cid, lvl, blk| {
                *cid == chain_id && *lvl == SafetyLevel::CrossUnsafe && blk.number == 100
            })
            .returning(|_, _, _| Ok(()));

        let job = CrossSafetyCheckerJob::new(
            chain_id,
            Arc::new(mock),
            CancellationToken::new(),
            Duration::from_secs(1),
            SafetyLevel::CrossUnsafe,
        )
        .expect("error initializing cross-safety checker job");

        let checker = CrossSafetyChecker::new(&*job.provider);
        let result = job.promote_next_block(&checker);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().number, 100);
    }

    #[test]
    fn promotes_next_cross_unsafe_failed_with_no_candidates() {
        let chain_id = 1;
        let mut mock = MockProvider::default();

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
        )
        .expect("error initializing cross-safety checker job");

        let checker = CrossSafetyChecker::new(&*job.provider);
        let result = job.promote_next_block(&checker);

        assert!(matches!(result, Err(CrossSafetyError::NoBlockToPromote)));
    }

    #[test]
    fn returns_unsupported_target_level_error() {
        let chain_id = 1;
        let mock = MockProvider::default();

        let err = CrossSafetyCheckerJob::new(
            chain_id,
            Arc::new(mock),
            CancellationToken::new(),
            Duration::from_secs(1),
            SafetyLevel::Finalized, // unsupported
        )
        .unwrap_err();
        assert!(matches!(err, CrossSafetyError::UnsupportedTargetLevel(_)));
    }
}
