use crate::CrossSafetyError;
use alloy_primitives::ChainId;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::CrossChainSafetyProvider;
use kona_supervisor_types::ExecutingMessage;
use op_alloy_consensus::interop::SafetyLevel;

/// Uses a [`CrossChainSafetyProvider`] to verify the safety of cross-chain message dependencies.
#[derive(Debug)]
pub struct CrossSafetyChecker<'a, P> {
    provider: &'a P,
}

#[allow(dead_code)]
impl<'a, P> CrossSafetyChecker<'a, P>
where
    P: CrossChainSafetyProvider,
{
    pub(crate) const fn new(provider: &'a P) -> Self {
        Self { provider }
    }

    /// Verifies that all executing messages in the given block meet the required safety level.
    pub fn verify_block_dependencies(
        &self,
        chain_id: ChainId,
        block: BlockInfo,
        required_level: SafetyLevel,
    ) -> Result<(), CrossSafetyError> {
        let head = self.provider.get_safety_head_ref(chain_id, required_level)?;

        // Check the candidate block is valid
        if head.number + 1 == block.number && block.is_parent_of(&head) {
            return Err(CrossSafetyError::DependencyNotSafe {
                chain_id,
                block_number: block.number,
            });
        }

        // Retrieve logs emitted in this block
        let executing_logs = self.provider.get_block_logs(chain_id, block.number)?;

        for log in executing_logs {
            if let Some(message) = log.executing_message {
                self.verify_message_dependency(&message, &head)?;
            }
        }

        Ok(())
    }

    /// Ensures that the block a message depends on satisfies the given safety level.
    fn verify_message_dependency(
        &self,
        message: &ExecutingMessage,
        head: &BlockInfo,
    ) -> Result<(), CrossSafetyError> {
        let block = self.provider.get_block(message.chain_id, message.block_number)?;

        if head.number < block.number {
            return Err(CrossSafetyError::DependencyNotSafe {
                chain_id: message.chain_id,
                block_number: message.block_number,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use kona_supervisor_storage::StorageError;
    use kona_supervisor_types::Log;
    use mockall::mock;
    use op_alloy_consensus::interop::SafetyLevel;

    mock! (
        #[derive(Debug)]
        pub Provider {}

        impl CrossChainSafetyProvider for Provider {
            fn get_block(&self, chain_id: ChainId, block_number: u64) -> Result<BlockInfo, StorageError>;
            fn get_safety_head_ref(&self, chain_id: ChainId, level: SafetyLevel) -> Result<BlockInfo, StorageError>;
            fn get_block_logs(&self, chain_id: ChainId, block_number: u64) -> Result<Vec<Log>, StorageError>;
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

        provider
            .expect_get_block()
            .withf(move |cid, num| *cid == chain_id && *num == 100)
            .returning(move |_, _| Ok(block_info));

        let checker = CrossSafetyChecker::new(&provider);
        let result = checker.verify_message_dependency(&msg, &head_info);
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

        provider
            .expect_get_block()
            .withf(move |cid, num| *cid == chain_id && *num == 105)
            .returning(move |_, _| Ok(dep_block));

        let checker = CrossSafetyChecker::new(&provider);
        let result = checker.verify_message_dependency(&msg, &head_block);

        assert!(
            matches!(result, Err(CrossSafetyError::DependencyNotSafe { .. })),
            "Expected DependencyNotSafe error"
        );
    }

    #[test]
    fn verify_block_dependencies_success() {
        let chain_id = 1u64;
        let block =
            BlockInfo { number: 101, hash: b256(101), parent_hash: b256(100), timestamp: 0 };

        let exec_msg = ExecutingMessage {
            chain_id,
            block_number: 100,
            log_index: 0,
            timestamp: 0,
            hash: b256(999),
        };

        let log = Log { index: 0, hash: b256(999), executing_message: Some(exec_msg) };

        let dep_block =
            BlockInfo { number: 100, hash: b256(100), parent_hash: b256(99), timestamp: 0 };

        let head = BlockInfo { number: 101, hash: b256(101), parent_hash: b256(100), timestamp: 0 };

        let mut provider = MockProvider::default();

        provider
            .expect_get_block_logs()
            .withf(move |cid, num| *cid == chain_id && *num == 101)
            .returning(move |_, _| Ok(vec![log.clone()]));

        provider
            .expect_get_block()
            .withf(move |cid, num| *cid == chain_id && *num == 100)
            .returning(move |_, _| Ok(dep_block));

        provider
            .expect_get_safety_head_ref()
            .withf(move |cid, lvl| *cid == chain_id && *lvl == SafetyLevel::CrossSafe)
            .returning(move |_, _| Ok(head));

        let checker = CrossSafetyChecker::new(&provider);
        let result = checker.verify_block_dependencies(chain_id, block, SafetyLevel::CrossSafe);
        assert!(result.is_ok());
    }
}
