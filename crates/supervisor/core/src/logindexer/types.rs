use alloy_primitives::B256;

/// A parsed executing message extracted from a log emitted by the
/// `CrossL2Inbox` contract on an L2 chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutingMessage {
    /// The chain ID where the message was observed.
    pub chain_id: u64,
    /// The block number that contained the log.
    pub block_number: u64,
    /// The log index within the block.
    pub log_index: u16,
    /// The timestamp of the block.
    pub timestamp: u64,
    /// A unique hash identifying the log (based on payload + origin).
    pub hash: B256,
}

/// A reference entry representing a log observed in an L2 receipt.
///
/// This struct does **not** store the actual log content. Instead:
/// - `hash` is the hash of the log (as computed by [`log_to_log_hash`]),
///   which uniquely identifies the log entry and can be used for lookups or comparisons.
/// - `executing_message` is present if the log represents an `ExecutingMessage` emitted
///   by the `CrossL2Inbox` contract.
///
/// This is the unit persisted by the log indexer into the database for later validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// The hash of the log, derived from the log address and payload.
    pub hash: B256,
    /// The parsed message, if the log matches an `ExecutingMessage` event.
    pub executing_message: Option<ExecutingMessage>,
}

