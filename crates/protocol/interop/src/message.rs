//! Interop message primitives.
//!
//! <https://specs.optimism.io/interop/messaging.html#messaging>
//! <https://github.com/ethereum-optimism/optimism/blob/34d5f66ade24bd1f3ce4ce7c0a6cfc1a6540eca1/packages/contracts-bedrock/src/L2/CrossL2Inbox.sol>

use crate::constants::CROSS_L2_INBOX_ADDRESS;
use alloc::{vec, vec::Vec};
use alloy_primitives::{Bytes, Log, keccak256};
use alloy_sol_types::{SolEvent, sol};
use derive_more::{AsRef, From};
use op_alloy_consensus::OpReceiptEnvelope;

sol! {
    /// @notice The struct for a pointer to a message payload in a remote (or local) chain.
    #[derive(Default, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    struct MessageIdentifier {
        address origin;
        uint256 blockNumber;
        uint256 logIndex;
        uint256 timestamp;
        #[cfg_attr(feature = "serde", serde(rename = "chainID"))]
        uint256 chainId;
    }

    /// @notice Emitted when a cross chain message is being executed.
    /// @param payloadHash Hash of message payload being executed.
    /// @param identifier Encoded Identifier of the message.
    ///
    /// Parameter names are derived from the `op-supervisor` JSON field names.
    /// See the relevant definition in the Optimism repository:
    /// [Ethereum-Optimism/op-supervisor](https://github.com/ethereum-optimism/optimism/blob/4ba2eb00eafc3d7de2c8ceb6fd83913a8c0a2c0d/op-supervisor/supervisor/types/types.go#L61-L64).
    #[derive(Default, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    event ExecutingMessage(bytes32 indexed payloadHash, MessageIdentifier identifier);

    /// @notice Executes a cross chain message on the destination chain.
    /// @param _id      Identifier of the message.
    /// @param _target  Target address to call.
    /// @param _message Message payload to call target with.
    function executeMessage(
        MessageIdentifier calldata _id,
        address _target,
        bytes calldata _message
    ) external;
}

/// A [RawMessagePayload] is the raw payload of an initiating message.
#[derive(Debug, Clone, From, AsRef, PartialEq, Eq)]
pub struct RawMessagePayload(Bytes);

impl From<&Log> for RawMessagePayload {
    fn from(log: &Log) -> Self {
        let mut data = vec![0u8; log.topics().len() * 32 + log.data.data.len()];
        for (i, topic) in log.topics().iter().enumerate() {
            data[i * 32..(i + 1) * 32].copy_from_slice(topic.as_ref());
        }
        data[(log.topics().len() * 32)..].copy_from_slice(log.data.data.as_ref());
        data.into()
    }
}

impl From<Vec<u8>> for RawMessagePayload {
    fn from(data: Vec<u8>) -> Self {
        Self(Bytes::from(data))
    }
}

impl From<executeMessageCall> for ExecutingMessage {
    fn from(call: executeMessageCall) -> Self {
        Self { identifier: call._id, payloadHash: keccak256(call._message.as_ref()) }
    }
}

/// An [`ExecutingDescriptor`] is a part of the payload to `supervisor_checkAccessList`
/// Spec: <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#executingdescriptor>
#[derive(Default, Debug, PartialEq, Eq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExecutingDescriptor {
    /// The timestamp used to enforce timestamp [invariant](https://github.com/ethereum-optimism/specs/blob/main/specs/interop/derivation.md#invariants)
    timestamp: u64,
    /// The timeout that requests verification to still hold at `timestamp+timeout`
    /// (message expiry may drop previously valid messages).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    timeout: Option<u64>,
}

impl ExecutingDescriptor {
    /// Create a new [`ExecutingDescriptor`] from the timestamp and timeout
    pub const fn new(timestamp: u64, timeout: Option<u64>) -> Self {
        Self { timestamp, timeout }
    }
}

/// A wrapper type for [ExecutingMessage] containing the chain ID of the chain that the message was
/// executed on.
#[derive(Debug)]
pub struct EnrichedExecutingMessage {
    /// The inner [ExecutingMessage].
    pub inner: ExecutingMessage,
    /// The chain ID of the chain that the message was executed on.
    pub executing_chain_id: u64,
    /// The timestamp of the block that the executing message was included in.
    pub executing_timestamp: u64,
}

impl EnrichedExecutingMessage {
    /// Create a new [EnrichedExecutingMessage] from an [ExecutingMessage] and a chain ID.
    pub const fn new(
        inner: ExecutingMessage,
        executing_chain_id: u64,
        executing_timestamp: u64,
    ) -> Self {
        Self { inner, executing_chain_id, executing_timestamp }
    }
}

/// Extracts all [ExecutingMessage] events from list of [OpReceiptEnvelope]s.
///
/// See [`parse_log_to_executing_message`].
///
/// Note: filters out logs that don't contain executing message events.
pub fn extract_executing_messages(receipts: &[OpReceiptEnvelope]) -> Vec<ExecutingMessage> {
    receipts.iter().fold(Vec::new(), |mut acc, envelope| {
        let executing_messages = envelope.logs().iter().filter_map(parse_log_to_executing_message);

        acc.extend(executing_messages);
        acc
    })
}

/// Parses [`Log`]s to [`ExecutingMessage`]s.
///
/// See [`parse_log_to_executing_message`] for more details. Return iterator maps 1-1 with input.
pub fn parse_logs_to_executing_msgs<'a>(
    logs: impl Iterator<Item = &'a Log>,
) -> impl Iterator<Item = Option<ExecutingMessage>> {
    logs.map(parse_log_to_executing_message)
}

/// Parse [`Log`] to [`ExecutingMessage`], if any.
///
/// Max one [`ExecutingMessage`] event can exist per log. Returns `None` if log doesn't contain
/// executing message event.
pub fn parse_log_to_executing_message(log: &Log) -> Option<ExecutingMessage> {
    (log.address == CROSS_L2_INBOX_ADDRESS && log.topics().len() == 2)
        .then(|| ExecutingMessage::decode_log_data(&log.data).ok())
        .flatten()
}
