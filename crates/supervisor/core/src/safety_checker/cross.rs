use crate::{
    CrossSafetyError,
    safety_checker::error::ValidationError::InteropValidationError,
};
use alloy_primitives::ChainId;
use derive_more::Constructor;
use kona_interop::InteropValidator;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::CrossChainSafetyProvider;
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
                // Check whether the message passes interop timestamps related validation
                self.validator
                    .validate_interop_timestamps(
                        message.chain_id,
                        message.timestamp,
                        self.chain_id,
                        block.timestamp,
                        None,
                    )
                    .map_err(InteropValidationError)?;

                // Check weather the message passes a dependency check
                self.verify_message_dependency(&message, required_level)?;

                // todo: check the init message actually exists in log database
            }
        }

        Ok(())
    }

    /// Ensures that the block a message depends on satisfies the given safety level.
    fn verify_message_dependency(
        &self,
        message: &ExecutingMessage,
        required_level: SafetyLevel,
    ) -> Result<(), CrossSafetyError> {
        let block = self.provider.get_block(message.chain_id, message.block_number)?;
        let head = self.provider.get_safety_head_ref(message.chain_id, required_level)?;

        if head.number < block.number {
            return Err(CrossSafetyError::DependencyNotSafe {
                chain_id: message.chain_id,
                block_number: message.block_number,
            });
        }
        // todo: add check if the dependency is not safe and can possibly create a cyclic dependency
        // and return proper error for that

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use kona_interop::{InteropValidationError, DerivedRefPair};
    use kona_supervisor_storage::StorageError;
    use kona_supervisor_types::Log;
    use mockall::mock;
    use op_alloy_consensus::interop::SafetyLevel;
    
    mock! (
        #[derive(Debug)]
        pub Provider {}

        impl CrossChainSafetyProvider for Provider {
            fn get_block(&self, chain_id: ChainId, block_number: u64) -> Result<BlockInfo, StorageError>;
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
            .expect_get_block()
            .withf(move |cid, num| *cid == chain_id && *num == 100)
            .returning(move |_, _| Ok(block_info));

        provider
            .expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == chain_id && *lvl == SafetyLevel::CrossSafe)
            .returning(move |_, _| Ok(head_info));

        let checker = CrossSafetyChecker::new(1, &validator, &provider);
        let result = checker.verify_message_dependency(&msg, SafetyLevel::CrossSafe);
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
            .expect_get_block()
            .withf(move |cid, num| *cid == chain_id && *num == 105)
            .returning(move |_, _| Ok(dep_block));

        provider
            .expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == chain_id && *lvl == SafetyLevel::CrossSafe)
            .returning(move |_, _| Ok(head_block));

        let checker = CrossSafetyChecker::new(1, &validator, &provider);
        let result = checker.verify_message_dependency(&msg, SafetyLevel::CrossSafe);

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

        let exec_msg = ExecutingMessage {
            chain_id: init_chain_id,
            block_number: 100,
            log_index: 0,
            timestamp: 200,
            hash: b256(999),
        };

        let log = Log { index: 0, hash: b256(999), executing_message: Some(exec_msg) };

        let dep_block =
            BlockInfo { number: 100, hash: b256(100), parent_hash: b256(99), timestamp: 195 };

        let head =
            BlockInfo { number: 101, hash: b256(101), parent_hash: b256(100), timestamp: 200 };

        let mut provider = MockProvider::default();
        let mut validator = MockValidator::default();

        provider
            .expect_get_block_logs()
            .withf(move |cid, num| *cid == exec_chain_id && *num == 101)
            .returning(move |_, _| Ok(vec![log.clone()]));

        provider
            .expect_get_block()
            .withf(move |cid, num| *cid == init_chain_id && *num == 100)
            .returning(move |_, _| Ok(dep_block));

        provider
            .expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == init_chain_id && *lvl == SafetyLevel::CrossSafe)
            .returning(move |_, _| Ok(head));

        validator
            .expect_validate_interop_timestamps()
            .returning(move |_, _, _, _, _| Ok(()));

        let checker = CrossSafetyChecker::new(exec_chain_id, &validator, &provider);
        let result = checker.validate_block(block, SafetyLevel::CrossSafe);
        assert!(result.is_ok());
    }
}
