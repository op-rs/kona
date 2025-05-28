use kona_supervisor_storage::LogStorage;
use std::sync::Arc;
use kona_interop::InteropProvider;
use kona_interop::parse_log_to_executing_message;
use kona_protocol::BlockInfo;
use crate::logindexer::{
    log_to_log_hash, 
    payload_hash_to_log_hash, 
    LogEntry, 
    LogIndexerError,
};
use crate::{ManagedNode, SupervisorService};
use kona_supervisor_types::{Log, ExecutingMessage};

/// The [`LogIndexer`] is responsible for processing L2 receipts, extracting structured messages
/// [`ExecutingMessage`] and persis them to the state manager
pub struct LogIndexer {
    pub provider: Arc<ManagedNode>,
    pub state_manager: Arc<dyn LogStorage>,
}

impl LogIndexer {
    /// Creates a new [`LogIndexer`] with the given provider and state manager.
    ///
    /// # Arguments
    /// - `provider`: A shared reference to the interop provider used to fetch receipts.
    /// - `state_manager`: A shared reference to the component used to persist indexed logs.
    pub fn new(provider: Arc<ManagedNode>, state_manager: Arc<dyn LogStorage>) -> Self {
        Self {
            provider,
            state_manager,
        }
    }
    
    /// Processes the logs of a given block and stores them into the state manager.
    ///
    /// This function:
    /// - Fetches all receipts for the given block from the specified chain.
    /// - Iterates through all logs in all receipts.
    /// - For each log, computes a [`log_to_log_hash`] and optionally parses an [`ExecutingMessage`].
    /// - Records each log into a [`LogEntry`], including the message if found.
    /// - Saves all log entries atomically using the [`StateManager`].
    ///
    /// # Arguments
    /// - `chain_id`: The chain ID from which the block originated.
    /// - `block`: Metadata about the block being processed.
    pub async fn process_and_store_logs(
        &self,
        chain_id: u64,
        block: &BlockInfo,
    ) -> Result<(), LogIndexerError> {
        let receipts = self.provider
            .receipts_by_number(chain_id, block.number)
            .await
            .map_err(|_| LogIndexerError::FetchFailed {
                chain_id,
                block_number: block.number,
            })?;

        let mut log_entries = Vec::new();

        for receipt in &receipts {
            for log in receipt.logs() {
                let log_hash = log_to_log_hash(log);

                let executing_message = parse_log_to_executing_message(log).map(|msg| {
                    let payload_hash = payload_hash_to_log_hash(msg.payloadHash, msg.identifier.origin);
                    ExecutingMessage {
                        chain_id,
                        block_number: msg.identifier.blockNumber.try_into().unwrap_or(0),
                        log_index: msg.identifier.logIndex.try_into().unwrap_or(0),
                        timestamp: msg.identifier.timestamp.try_into().unwrap_or(0),
                        hash: payload_hash,
                    }
                });

                log_entries.push(Log {
                    index: log.index,
                    hash: log_hash,
                    executing_message,
                });
            }
        }

        self.state_manager.store_block_logs(block, log_entries)
            .map_err(|_| LogIndexerError::StateWrite {
                chain_id,
                block_number: block.number,
            })?;
        Ok(())
    }
}