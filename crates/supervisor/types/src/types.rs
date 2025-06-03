//! Types for communication between supervisor and op-node.
//!
//! This module defines the data structures used for communicating between the supervisor
//! and the op-node components in the rollup system. It includes block references,
//! block seals, derivation events, and event notifications.

use alloy_eips::BlockId;
use alloy_primitives::{B256, U64};
use derive_more::{Constructor, Display};
use kona_interop::DerivedRefPair;
use kona_protocol::BlockInfo;
use serde::{Deserialize, Serialize};
use std::fmt;

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

/// Represents a [`BlockReplacement`] event where one block replaces another.
#[derive(Debug, Clone, Display, PartialEq, Eq, Serialize, Deserialize)]
#[display("replacement: {replacement}, invalidated: {invalidated}")]
#[serde(rename_all = "camelCase")]
pub struct BlockReplacement<T = BlockInfo> {
    /// The block that replaces the invalidated block
    pub replacement: T,
    /// Hash of the block being invalidated and replaced
    pub invalidated: B256,
}

impl<T> BlockReplacement<T> {
    /// Creates a new [`BlockReplacement`].
    pub const fn new(replacement: T, invalidated: B256) -> Self {
        Self { replacement, invalidated }
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

/// Event sent by the node to the supervisor to share updates.
///
/// This struct is used to communicate various events that occur within the node.
/// At least one of the fields will be `Some`, and the rest will be `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Constructor)]
#[serde(rename_all = "camelCase")]
pub struct ManagedEvent {
    /// Indicates a successful reset operation with an optional message
    pub reset: Option<String>,

    /// Information about a new unsafe block
    pub unsafe_block: Option<BlockInfo>,

    /// Update about a newly derived L2 block from L1
    pub derivation_update: Option<DerivedRefPair>,

    /// Signals that there are no more L1 blocks to process
    pub exhaust_l1: Option<DerivedRefPair>,

    /// Confirms a successful block replacement operation
    pub replace_block: Option<BlockReplacement>,

    /// Indicates an update to the derivation origin
    pub derivation_origin_update: Option<BlockInfo>,
}

impl fmt::Display for ManagedEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(ref reset) = self.reset {
            parts.push(format!("reset: {reset}"));
        }
        if let Some(ref block) = self.unsafe_block {
            parts.push(format!("unsafe_block: {block}"));
        }
        if let Some(ref pair) = self.derivation_update {
            parts.push(format!("derivation_update: {pair}"));
        }
        if let Some(ref pair) = self.exhaust_l1 {
            parts.push(format!("exhaust_l1: {pair}"));
        }
        if let Some(ref replacement) = self.replace_block {
            parts.push(format!("replace_block: {replacement}"));
        }
        if let Some(ref origin) = self.derivation_origin_update {
            parts.push(format!("derivation_origin_update: {origin}"));
        }

        if parts.is_empty() { write!(f, "none") } else { write!(f, "{}", parts.join(", ")) }
    }
}
