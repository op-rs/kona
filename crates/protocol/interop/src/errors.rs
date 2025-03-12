//! Error types for the `kona-interop` crate.

use crate::{InteropProvider, SafetyLevel};
use alloc::vec::Vec;
use alloy_primitives::{Address, B256};
use thiserror::Error;

/// Derived from op-supervisor
// todo: rm once resolved <https://github.com/ethereum-optimism/optimism/issues/14603>
const UNKNOWN_CHAIN_MSG: &str = "unknown chain: ";
/// Derived from [op-supervisor](https://github.com/ethereum-optimism/optimism/blob/4ba2eb00eafc3d7de2c8ceb6fd83913a8c0a2c0d/op-supervisor/supervisor/backend/backend.go#L479)
// todo: rm once resolved <https://github.com/ethereum-optimism/optimism/issues/14603>
const MINIMUM_SAFETY_MSG: &str = "does not meet the minimum safety";

/// An error type for the [MessageGraph] struct.
///
/// [MessageGraph]: crate::MessageGraph
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MessageGraphError<E> {
    /// Dependency set is impossibly empty
    #[error("Dependency set is impossibly empty")]
    EmptyDependencySet,
    /// Remote message not found
    #[error("Remote message not found on chain ID {0} with message hash {1}")]
    RemoteMessageNotFound(u64, B256),
    /// Invalid message origin
    #[error("Invalid message origin. Expected {0}, got {1}")]
    InvalidMessageOrigin(Address, Address),
    /// Invalid message payload hash
    #[error("Invalid message hash. Expected {0}, got {1}")]
    InvalidMessageHash(B256, B256),
    /// Invalid message timestamp
    #[error("Invalid message timestamp. Expected {0}, got {1}")]
    InvalidMessageTimestamp(u64, u64),
    /// Message is in the future
    #[error("Message is in the future. Expected timestamp to be <= {0}, got {1}")]
    MessageInFuture(u64, u64),
    /// Invalid messages were found
    #[error("Invalid messages found on chains: {0:?}")]
    InvalidMessages(Vec<u64>),
    /// Missing a [RollupConfig] for a chain ID
    ///
    /// [RollupConfig]: kona_genesis::RollupConfig
    #[error("Missing a RollupConfig for chain ID {0}")]
    MissingRollupConfig(u64),
    /// Interop provider error
    #[error("Interop provider: {0}")]
    InteropProviderError(#[from] E),
}

/// A [Result] alias for the [MessageGraphError] type.
#[allow(type_alias_bounds)]
pub type MessageGraphResult<T, P: InteropProvider> =
    core::result::Result<T, MessageGraphError<P::Error>>;

/// An error type for the [SuperRoot] struct's serialization and deserialization.
///
/// [SuperRoot]: crate::SuperRoot
#[derive(Debug, Clone, Error)]
pub enum SuperRootError {
    /// Invalid super root version byte
    #[error("Invalid super root version byte")]
    InvalidVersionByte,
    /// Unexpected encoded super root length
    #[error("Unexpected encoded super root length")]
    UnexpectedLength,
    /// Slice conversion error
    #[error("Slice conversion error: {0}")]
    SliceConversionError(#[from] core::array::TryFromSliceError),
}

/// A [Result] alias for the [SuperRootError] type.
pub type SuperRootResult<T> = core::result::Result<T, SuperRootError>;

/// Invalid [`ExecutingMessage`](crate::ExecutingMessage) error.
#[derive(Error, Debug)]
pub enum InvalidInboxEntry {
    /// Message does not meet minimum safety level
    #[error("message does not meet min safety level, got: {got}, expected: {expected}")]
    MinimumSafety {
        /// Actual level of the message
        got: SafetyLevel,
        /// Minimum acceptable level that was passed to supervisor
        expected: SafetyLevel,
    },
    /// Invalid chain
    #[error("unsupported chain id: {0}")]
    UnknownChain(u64),
}

impl InvalidInboxEntry {
    /// Parses error message. Returns `None`, if message is not recognized.
    // todo: match on error code instead of message string once resolved <https://github.com/ethereum-optimism/optimism/issues/14603>
    pub fn parse_err_msg(err_msg: &str) -> Option<Self> {
        // Check if it's invalid message call, message example:
        // `failed to check message: failed to check log: unknown chain: 14417`
        if err_msg.contains(UNKNOWN_CHAIN_MSG) {
            if let Ok(chain_id) =
                err_msg.split(' ').last().expect("message contains chain id").parse::<u64>()
            {
                return Some(Self::UnknownChain(chain_id))
            }
        // Check if it's `does not meet the minimum safety` error, message example:
        // `message {0x4200000000000000000000000000000000000023 4 1 1728507701 901}
        // (safety level: unsafe) does not meet the minimum safety cross-unsafe"`
        } else if err_msg.contains(MINIMUM_SAFETY_MSG) {
            let message_safety = if err_msg.contains("safety level: safe") {
                SafetyLevel::Safe
            } else if err_msg.contains("safety level: local-safe") {
                SafetyLevel::LocalSafe
            } else if err_msg.contains("safety level: cross-unsafe") {
                SafetyLevel::CrossUnsafe
            } else if err_msg.contains("safety level: unsafe") {
                SafetyLevel::Unsafe
            } else if err_msg.contains("safety level: invalid") {
                SafetyLevel::Invalid
            } else {
                // Unexpected level name
                return None
            };
            let expected_safety = if err_msg.contains("safety finalized") {
                SafetyLevel::Finalized
            } else if err_msg.contains("safety safe") {
                SafetyLevel::Safe
            } else if err_msg.contains("safety local-safe") {
                SafetyLevel::LocalSafe
            } else if err_msg.contains("safety cross-unsafe") {
                SafetyLevel::CrossUnsafe
            } else if err_msg.contains("safety unsafe") {
                SafetyLevel::Unsafe
            } else {
                // Unexpected level name
                return None
            };

            return Some(Self::MinimumSafety { expected: expected_safety, got: message_safety })
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN_SAFETY_CROSS_UNSAFE_ERROR: &str = "message {0x4200000000000000000000000000000000000023 4 1 1728507701 901} (safety level: unsafe) does not meet the minimum safety cross-unsafe";
    const MIN_SAFETY_UNSAFE_ERROR: &str = "message {0x4200000000000000000000000000000000000023 1091637521 4369 0 901} (safety level: invalid) does not meet the minimum safety unsafe";
    const MIN_SAFETY_FINALIZED_ERROR: &str = "message {0x4200000000000000000000000000000000000023 1091600001 215 1170 901} (safety level: safe) does not meet the minimum safety finalized";
    const INVALID_CHAIN: &str =
        "failed to check message: failed to check log: unknown chain: 14417";
    const RANDOM_ERROR: &str = "gibberish error";

    #[test]
    fn test_op_supervisor_error_parsing() {
        assert!(matches!(
            InvalidInboxEntry::parse_err_msg(MIN_SAFETY_CROSS_UNSAFE_ERROR).unwrap(),
            InvalidInboxEntry::MinimumSafety {
                expected: SafetyLevel::CrossUnsafe,
                got: SafetyLevel::Unsafe
            }
        ));

        assert!(matches!(
            InvalidInboxEntry::parse_err_msg(MIN_SAFETY_UNSAFE_ERROR).unwrap(),
            InvalidInboxEntry::MinimumSafety {
                expected: SafetyLevel::Unsafe,
                got: SafetyLevel::Invalid
            }
        ));

        assert!(matches!(
            InvalidInboxEntry::parse_err_msg(MIN_SAFETY_FINALIZED_ERROR).unwrap(),
            InvalidInboxEntry::MinimumSafety {
                expected: SafetyLevel::Finalized,
                got: SafetyLevel::Safe,
            }
        ));

        assert!(matches!(
            InvalidInboxEntry::parse_err_msg(INVALID_CHAIN).unwrap(),
            InvalidInboxEntry::UnknownChain(14417)
        ));

        assert!(InvalidInboxEntry::parse_err_msg(RANDOM_ERROR).is_none());
    }
}
