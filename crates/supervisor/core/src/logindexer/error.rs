use thiserror::Error;

/// Error type for the [`LogIndexer`].
#[derive(Error, Debug)]
pub enum LogIndexerError {
    /// Failed to write processed logs for a block to the state manager.
    #[error("Failed to write logs for block {block_number} on chain {chain_id} to state")]
    StateWrite {
        /// The chain id of the block
        chain_id: u64,
        /// The block number whose logs failed to persist.
        block_number: u64,
    },
    /// Failed to fetch logs for a block from the state manager.   
    #[error("Failed to fetch logs for block {block_number} on chain {chain_id}")]
    FetchFailed {
        /// The chain id of the block
        chain_id: u64,
        /// The block number whose logs failed to persist.
        block_number: u64,
    }
}
