//! Types for communication between supervisor and op-node.
//!
//! This module defines the data structures used for communicating between the supervisor
//! and the op-node components in the rollup system. It includes block references,
//! block seals, derivation events, and event notifications.

use alloy_primitives::B256;
use kona_interop::ManagedEvent;
use serde::{Deserialize, Serialize};

// todo:: Determine appropriate locations for these structs and move them accordingly.
// todo:: Link these structs to the spec documentation after the related PR is merged.

/// Represents a sealed block with its hash, number, and timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSeal {
    /// The block's hash
    pub hash: B256,
    /// The block number
    pub number: u64,
    /// The block's timestamp
    pub timestamp: u64,
}

impl BlockSeal {
    /// Creates a new [`BlockSeal`] with the given hash, number, and timestamp.
    pub const fn new(hash: B256, number: u64, timestamp: u64) -> Self {
        Self { hash, number, timestamp }
    }
}
/// Output data for version 0 of the protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct OutputV0 {
    /// The state root hash
    pub state_root: B256,
    /// Storage root of the message passer contract
    pub message_passer_storage_root: B256,
    /// The block hash
    pub block_hash: B256,
}

impl OutputV0 {
    /// Creates a new [`OutputV0`] instance.
    pub const fn new(
        state_root: B256,
        message_passer_storage_root: B256,
        block_hash: B256,
    ) -> Self {
        Self { state_root, message_passer_storage_root, block_hash }
    }
}

/// Represents the events structure sent by the node to the supervisor.
#[derive(Debug, Serialize, Deserialize)]
pub struct SubscriptionEvent {
    /// Represents the event data sent by the node
    pub data: Option<ManagedEvent>,
}
