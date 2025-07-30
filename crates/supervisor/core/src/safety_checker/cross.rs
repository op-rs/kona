use crate::{
    CrossSafetyError,
    safety_checker::{ValidationError, ValidationError::InitiatingMessageNotFound},
};
use alloy_primitives::ChainId;
use derive_more::Constructor;
use kona_interop::InteropValidator;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::{CrossChainSafetyProvider, StorageError};
use kona_supervisor_types::ExecutingMessage;
use op_alloy_consensus::interop::SafetyLevel;

/// Uses a [`CrossChainSafetyProvider`] to verify the safety of cross-chain message dependencies.
#[derive(Debug, Constructor)]
pub struct CrossSafetyChecker<'a, P, V> {
    chain_id: ChainId,
    validator: &'a V,
    provider: &'a P,
}

impl<P, V> CrossSafetyChecker<'_, P, V>
where
    P: CrossChainSafetyProvider,
    V: InteropValidator,
{
    /// Verifies that all executing messages in the given block are valid based on the validity
    /// checks
    pub fn validate_block(
        &self,
        block: BlockInfo,
        required_level: SafetyLevel,
    ) -> Result<(), CrossSafetyError> {
        // Retrieve logs emitted in this block
        let executing_logs = self.provider.get_block_logs(self.chain_id, block.number)?;

        for log in executing_logs {
            if let Some(message) = log.executing_message {
                let initiating_block =
                    self.provider.get_block(message.chain_id, message.block_number)?;

                // Check whether the message passes interop timestamps related validation
                self.validator
                    .validate_interop_timestamps(
                        message.chain_id,
                        message.timestamp,
                        self.chain_id,
                        block.timestamp,
                        None,
                    )
                    .map_err(ValidationError::InteropValidationError)?;

                // Check weather the message exists and valid
                self.validate_executing_message(initiating_block, &message)?;
                // Check weather the message passes the dependency check
                self.verify_message_dependency(initiating_block, &message, required_level)?;
            }
        }

        Ok(())
    }

    /// Ensures that the block a message depends on satisfies the given safety level.
    fn verify_message_dependency(
        &self,
        init_block: BlockInfo,
        message: &ExecutingMessage,
        required_level: SafetyLevel,
    ) -> Result<(), CrossSafetyError> {
        let head = self.provider.get_safety_head_ref(message.chain_id, required_level)?;

        if head.number < init_block.number {
            return Err(CrossSafetyError::DependencyNotSafe {
                chain_id: message.chain_id,
                block_number: message.block_number,
            });
        }
        // todo: add check if the dependency is not safe and can possibly create a cyclic dependency
        // and return proper error for that

        Ok(())
    }

    fn validate_executing_message(
        &self,
        init_block: BlockInfo,
        message: &ExecutingMessage,
    ) -> Result<(), CrossSafetyError> {
        // Ensure timestamp invariant
        if init_block.timestamp != message.timestamp {
            return Err(ValidationError::TimestampInvariantViolation {
                expected_timestamp: init_block.timestamp,
                actual_timestamp: message.timestamp,
            }
            .into());
        }

        // Try to fetch the original log from storage
        let init_msg = self
            .provider
            .get_log(message.chain_id, message.block_number, message.log_index)
            .map_err(|err| match err {
                StorageError::EntryNotFound(_) => {
                    CrossSafetyError::ValidationError(InitiatingMessageNotFound)
                }
                other => other.into(),
            })?;

        // Verify the hash of the message against the original
        // Don't need to verify the checksum as we're already verifying all the individual fields.
        if init_msg.hash != message.hash {
            return Err(ValidationError::InvalidMessageHash {
                message_hash: message.hash,
                original_hash: init_msg.hash,
            }
            .into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use kona_interop::{DerivedRefPair, InteropValidationError};
    use kona_supervisor_storage::{EntryNotFoundError, StorageError};
    use kona_supervisor_types::Log;
    use mockall::mock;
    use op_alloy_consensus::interop::SafetyLevel;

    mock! (
        #[derive(Debug)]
        pub Provider {}

        impl CrossChainSafetyProvider for Provider {
            fn get_block(&self, chain_id: ChainId, block_number: u64) -> Result<BlockInfo, StorageError>;
            fn get_log(&self, chain_id: ChainId, block_number: u64, log_index: u32) -> Result<Log, StorageError>;
            fn get_block_logs(&self, chain_id: ChainId, block_number: u64) -> Result<Vec<Log>, StorageError>;
            fn get_safety_head_ref(&self, chain_id: ChainId, level: SafetyLevel) -> Result<BlockInfo, StorageError>;
            fn update_current_cross_unsafe(&self, chain_id: ChainId, block: &BlockInfo) -> Result<(), StorageError>;
            fn update_current_cross_safe(&self, chain_id: ChainId, block: &BlockInfo) -> Result<DerivedRefPair, StorageError>;
        }
    );

    mock! (
        #[derive(Debug)]
        pub Validator {}

        impl InteropValidator for Validator {
            fn validate_interop_timestamps(
                &self,
                initiating_chain_id: ChainId,
                initiating_timestamp: u64,
                executing_chain_id: ChainId,
                executing_timestamp: u64,
                timeout: Option<u64>,
            ) -> Result<(), InteropValidationError>;

            fn is_post_interop(&self, chain_id: ChainId, timestamp: u64) -> bool;

            fn is_interop_activation_block(&self, chain_id: ChainId, block: BlockInfo) -> bool;
        }
    );

    fn b256(n: u64) -> B256 {
        let mut bytes = [0u8; 32];
        bytes[24..].copy_from_slice(&n.to_be_bytes());
        B256::from(bytes)
    }

    #[test]
    fn verify_message_dependency_success() {
        let chain_id = 1;
        let msg = ExecutingMessage {
            chain_id,
            block_number: 100,
            log_index: 0,
            timestamp: 0,
            hash: b256(0),
        };

        let block_info =
            BlockInfo { number: 100, hash: b256(100), parent_hash: b256(99), timestamp: 0 };

        let head_info =
            BlockInfo { number: 101, hash: b256(101), parent_hash: b256(100), timestamp: 0 };

        let mut provider = MockProvider::default();
        let validator = MockValidator::default();

        provider
            .expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == chain_id && *lvl == SafetyLevel::CrossSafe)
            .returning(move |_, _| Ok(head_info));

        let checker = CrossSafetyChecker::new(1, &validator, &provider);
        let result = checker.verify_message_dependency(block_info, &msg, SafetyLevel::CrossSafe);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_message_dependency_failed() {
        let chain_id = 1;
        let msg = ExecutingMessage {
            chain_id,
            block_number: 105, // dependency is ahead of safety head
            log_index: 0,
            timestamp: 0,
            hash: b256(123),
        };

        let dep_block =
            BlockInfo { number: 105, hash: b256(105), parent_hash: b256(104), timestamp: 0 };

        let head_block = BlockInfo {
            number: 100, // safety head is behind the message dependency
            hash: b256(100),
            parent_hash: b256(99),
            timestamp: 0,
        };

        let mut provider = MockProvider::default();
        let validator = MockValidator::default();

        provider
            .expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == chain_id && *lvl == SafetyLevel::CrossSafe)
            .returning(move |_, _| Ok(head_block));

        let checker = CrossSafetyChecker::new(1, &validator, &provider);
        let result = checker.verify_message_dependency(dep_block, &msg, SafetyLevel::CrossSafe);

        assert!(
            matches!(result, Err(CrossSafetyError::DependencyNotSafe { .. })),
            "Expected DependencyNotSafe error"
        );
    }

    #[test]
    fn validate_block_success() {
        let init_chain_id = 1;
        let exec_chain_id = 2;

        let block =
            BlockInfo { number: 101, hash: b256(101), parent_hash: b256(100), timestamp: 200 };

        let dep_block =
            BlockInfo { number: 100, hash: b256(100), parent_hash: b256(99), timestamp: 195 };

        let exec_msg = ExecutingMessage {
            chain_id: init_chain_id,
            block_number: 100,
            log_index: 0,
            timestamp: 195,
            hash: b256(999),
        };

        let init_log = Log {
            index: 0,
            hash: b256(999), // Matches msg.hash → passes checksum
            executing_message: None,
        };

        let exec_log = Log { index: 0, hash: b256(999), executing_message: Some(exec_msg) };

        let head =
            BlockInfo { number: 101, hash: b256(101), parent_hash: b256(100), timestamp: 200 };

        let mut provider = MockProvider::default();
        let mut validator = MockValidator::default();

        provider
            .expect_get_block_logs()
            .withf(move |cid, num| *cid == exec_chain_id && *num == 101)
            .returning(move |_, _| Ok(vec![exec_log.clone()]));

        provider
            .expect_get_block()
            .withf(move |cid, num| *cid == init_chain_id && *num == 100)
            .returning(move |_, _| Ok(dep_block));

        provider
            .expect_get_log()
            .withf(move |cid, blk, idx| *cid == init_chain_id && *blk == 100 && *idx == 0)
            .returning(move |_, _, _| Ok(init_log.clone()));

        provider
            .expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == init_chain_id && *lvl == SafetyLevel::CrossSafe)
            .returning(move |_, _| Ok(head));

        validator.expect_validate_interop_timestamps().returning(move |_, _, _, _, _| Ok(()));

        let checker = CrossSafetyChecker::new(exec_chain_id, &validator, &provider);
        let result = checker.validate_block(block, SafetyLevel::CrossSafe);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_executing_message_timestamp_violation() {
        let chain_id = 1;
        let msg = ExecutingMessage {
            chain_id,
            block_number: 100,
            log_index: 0,
            timestamp: 1234,
            hash: b256(999),
        };

        let init_block = BlockInfo {
            number: 100,
            hash: b256(100),
            parent_hash: b256(99),
            timestamp: 9999, // Different timestamp to trigger invariant violation
        };

        let provider = MockProvider::default();
        let validator = MockValidator::default();

        let checker = CrossSafetyChecker::new(chain_id, &validator, &provider);

        let result = checker.validate_executing_message(init_block, &msg);
        assert!(matches!(
            result,
            Err(CrossSafetyError::ValidationError(
                ValidationError::TimestampInvariantViolation { .. }
            ))
        ));
    }

    #[test]
    fn validate_executing_message_initiating_message_not_found() {
        let chain_id = 1;
        let msg = ExecutingMessage {
            chain_id,
            block_number: 100,
            log_index: 0,
            timestamp: 1234,
            hash: b256(999),
        };

        let init_block =
            BlockInfo { number: 100, hash: b256(100), parent_hash: b256(99), timestamp: 1234 };

        let mut provider = MockProvider::default();
        provider
            .expect_get_log()
            .withf(move |cid, blk, idx| *cid == chain_id && *blk == 100 && *idx == 0)
            .returning(|_, _, _| {
                Err(StorageError::EntryNotFound(EntryNotFoundError::LogNotFound {
                    block_number: 100,
                    log_index: 0,
                }))
            });

        let validator = MockValidator::default();

        let checker = CrossSafetyChecker::new(chain_id, &validator, &provider);
        let result = checker.validate_executing_message(init_block, &msg);

        assert!(matches!(
            result,
            Err(CrossSafetyError::ValidationError(InitiatingMessageNotFound))
        ));
    }

    #[test]
    fn validate_executing_message_hash_mismatch() {
        let chain_id = 1;
        let msg = ExecutingMessage {
            chain_id,
            block_number: 100,
            log_index: 0,
            timestamp: 1234,
            hash: b256(123),
        };

        let init_block =
            BlockInfo { number: 100, hash: b256(100), parent_hash: b256(99), timestamp: 1234 };

        let init_log = Log {
            index: 0,
            hash: b256(990), // Checksum mismatch
            executing_message: None,
        };

        let mut provider = MockProvider::default();
        provider
            .expect_get_log()
            .withf(move |cid, blk, idx| *cid == chain_id && *blk == 100 && *idx == 0)
            .returning(move |_, _, _| Ok(init_log.clone()));

        let validator = MockValidator::default();

        let checker = CrossSafetyChecker::new(chain_id, &validator, &provider);
        let result = checker.validate_executing_message(init_block, &msg);

        assert!(matches!(
            result,
            Err(CrossSafetyError::ValidationError(ValidationError::InvalidMessageHash {
                message_hash: _,
                original_hash: _
            }))
        ));
    }

    #[test]
    fn validate_executing_message_success() {
        let chain_id = 1;
        let timestamp = 1234;

        let init_block = BlockInfo {
            number: 100,
            hash: b256(100),
            parent_hash: b256(99),
            timestamp, // Matches msg.timestamp
        };

        let init_log = Log {
            index: 0,
            hash: b256(999), // Matches msg.hash → passes checksum
            executing_message: None,
        };

        let msg = ExecutingMessage {
            chain_id,
            block_number: 100,
            log_index: 0,
            timestamp,
            hash: b256(999),
        };

        let mut provider = MockProvider::default();
        provider
            .expect_get_log()
            .withf(move |cid, blk, idx| *cid == chain_id && *blk == 100 && *idx == 0)
            .returning(move |_, _, _| Ok(init_log.clone()));

        let validator = MockValidator::default();

        let checker = CrossSafetyChecker::new(chain_id, &validator, &provider);

        let result = checker.validate_executing_message(init_block, &msg);
        assert!(result.is_ok(), "Expected successful validation");
    }
}
