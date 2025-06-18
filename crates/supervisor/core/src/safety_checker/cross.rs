use alloy_primitives::ChainId;
use op_alloy_consensus::interop::SafetyLevel;
use thiserror::Error;
use kona_protocol::BlockInfo;
use kona_supervisor_storage::StorageError;
use kona_supervisor_types::{ExecutingMessage, Log};

/// Abstract provider for accessing chain safety status.
pub trait CrossChainSafetyProvider {
    fn get_block(&self, chain_id: ChainId, block_number: u64) -> Result<BlockInfo, StorageError>;
    fn get_block_logs(&self, chain_id: ChainId, block_number: u64)->Result<Vec<Log>, StorageError>;
    fn get_safe_head_ref(&self, chain_id: ChainId, level: SafetyLevel) -> Result<BlockInfo, StorageError>;
}

/// Performs dependency safety checks for cross-chain messages.
pub struct CrossSafetyChecker<'a, P> {
    provider: &'a P,
}

impl<'a, P> CrossSafetyChecker<'a, P>
where
    P: CrossChainSafetyProvider,
{
    pub fn new(provider: &'a P) -> Self {
        Self { provider }
    }

    /// Verifies that all executing messages in the given block meet the required safety level.
    pub fn verify_block_dependencies(
        &self,
        chain_id: ChainId,
        block: BlockInfo,
        required_level: SafetyLevel,
    ) -> Result<(), CrossSafetyError> {
        // Retrieve logs emitted in this block
        let executing_logs = self
            .provider
            .get_block_logs(chain_id, block.number)?;

        for log in executing_logs {
            if let Some(message) = log.executing_message {
                self.verify_message_dependency(&message, required_level)?;
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
        let block = self
            .provider
            .get_block(message.chain_id, message.block_number)?;
        let head = self
            .provider
            .get_safe_head_ref(message.chain_id, required_level)?;

        if head.number < block.number {
            return Err(CrossSafetyError::DependencyNotSafe { 
                chain_id: message.chain_id, 
                block_number: message.block_number});
        }

        Ok(())
    }
}

/// Errors returned when validating cross-chain message dependencies.
#[derive(Debug, Error)]
pub enum CrossSafetyError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("dependency on block {block_number} (chain {chain_id}) does not meet required safety level")]
    DependencyNotSafe {
        chain_id: ChainId,
        block_number: u64,
    },
}
