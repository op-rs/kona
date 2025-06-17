//! Types for communication between supervisor and op-node.
//!
//! This module defines the data structures used for communicating between the supervisor
//! and the op-node components in the rollup system. It includes block references,
//! block seals, derivation events, and event notifications.

use alloy_eips::BlockId;
use alloy_primitives::{B256, U64};
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
    pub number: U64,
    /// The block's timestamp
    pub timestamp: U64,
}

impl BlockSeal {
    /// Creates a new [`BlockSeal`] with the given hash, number, and timestamp.
    pub const fn new(hash: B256, number: U64, timestamp: U64) -> Self {
        Self { hash, number, timestamp }
    }
}

/// A reference to an L2 (layer 2) block with additional L2-specific information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L2BlockRef<T = BlockId> {
    /// The block's hash
    pub hash: B256,
    /// The block number in L2
    pub number: U64,
    /// The hash of the parent block
    pub parent_hash: B256,
    /// The block's timestamp
    pub timestamp: U64,
    /// Reference to the L1 origin block this L2 block is derived from
    pub l1_origin: T,
    /// Distance to the first block of the epoch (sequence position)
    pub sequence_number: U64,
}

impl<T> L2BlockRef<T> {
    /// Creates a new [`L2BlockRef`].
    pub const fn new(
        hash: B256,
        number: U64,
        parent_hash: B256,
        timestamp: U64,
        l1_origin: T,
        sequence_number: U64,
    ) -> Self {
        Self { hash, number, parent_hash, timestamp, l1_origin, sequence_number }
    }
}

/// Output data for version 0 of the protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
