//! Types for communication between supervisor and op-node.
//!
//! This module defines the data structures used for communicating between the supervisor
//! and the op-node components in the rollup system. It includes block references,
//! block seals, derivation events, and event notifications.

use alloy_primitives::{B256, U64};
use alloy_eips::BlockId;
use op_alloy_consensus::OpReceiptEnvelope;
use serde::{Serialize, Deserialize};

/// Represents a sealed block with its hash, number, and timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSeal {
    /// The block's hash
    pub hash: B256,
    /// The block number
    pub number: U64,
    /// The block's timestamp
    pub timestamp: U64
}

impl BlockSeal {
    /// Creates a new block seal with the given hash, number, and timestamp.
    pub fn new(hash: B256, number: U64, timestamp: U64) -> Self {
        Self { hash, number, timestamp }
    }
}

/// A reference to a block with its essential identifying information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRef {
    /// The block's hash
    pub hash: B256,
    /// The block number
    pub number: U64,
    /// The hash of the parent block
    pub parent_hash: B256,
    /// The block's timestamp
    pub timestamp: U64
}

impl BlockRef {
    /// Creates a new block reference.
    pub fn new(hash: B256, number: U64, parent_hash: B256, timestamp: U64) -> Self {
        Self { hash, number, parent_hash, timestamp }
    }
}

/// A reference to an L1 (layer 1) block.
pub type L1BlockRef = BlockRef;

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
    pub time: U64,
    /// Reference to the L1 origin block this L2 block is derived from
    pub l1_origin: T,
    /// Distance to the first block of the epoch (sequence position)
    pub sequence_number: U64
}

impl<T> L2BlockRef<T> {
    /// Creates a new L2 block reference.
    pub fn new(hash: B256, number: U64, parent_hash: B256, time: U64, l1_origin: T, sequence_number: U64) -> Self {
        Self { hash, number, parent_hash, time, l1_origin, sequence_number }
    }
}

/// Represents a pair of block references: a source and its derived block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedBlockRefPair<T = BlockRef> {
    /// The source block (typically from L1)
    pub source: T,
    /// The derived block (typically for L2)
    pub derived: T
}

impl<T> DerivedBlockRefPair<T> {
    /// Creates a new derived block reference pair.
    pub fn new(source: T, derived: T) -> Self {
        Self { source, derived }
    }
}

/// Represents a block replacement event where one block replaces another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockReplacement<T = BlockRef> {
    /// The block that replaces the invalidated block
    pub replacement: T,
    /// Hash of the block being invalidated and replaced
    pub invalidated: B256
}

impl<T> BlockReplacement<T> {
    /// Creates a new block replacement.
    pub fn new(replacement: T, invalidated: B256) -> Self {
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
    pub block_hash: B256
}

impl OutputV0 {
    /// Creates a new OutputV0 instance.
    pub fn new(state_root: B256, message_passer_storage_root: B256, block_hash: B256) -> Self {
        Self { state_root, message_passer_storage_root, block_hash }
    }
}

/// Collection of transaction receipts.
pub type Receipts = Vec<OpReceiptEnvelope>;

/// Event sent by the node to the supervisor to share updates.
///
/// This struct is used to communicate various events that occur within the node.
/// At least one of the fields will be `Some`, and the rest will be `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedEvent {
    /// Indicates a successful reset operation with an optional message
    pub reset: Option<String>,
    
    /// Information about a new unsafe block
    pub unsafe_block: Option<BlockRef>,
    
    /// Update about a newly derived L2 block from L1
    pub derivation_update: Option<DerivedBlockRefPair>,
    
    /// Signals that there are no more L1 blocks to process
    pub exhaust_l1: Option<DerivedBlockRefPair>,
    
    /// Confirms a successful block replacement operation
    pub replace_block: Option<BlockReplacement>,
    
    /// Indicates an update to the derivation origin
    pub derivation_origin_update: Option<BlockRef>,
}

impl ManagedEvent {
    /// Creates a new ManagedEvent with the specified fields.
    pub fn new(
        reset: Option<String>, 
        unsafe_block: Option<BlockRef>, 
        derivation_update: Option<DerivedBlockRefPair>, 
        exhaust_l1: Option<DerivedBlockRefPair>, 
        replace_block: Option<BlockReplacement>, 
        derivation_origin_update: Option<BlockRef>
    ) -> Self {
        Self { 
            reset, 
            unsafe_block, 
            derivation_update, 
            exhaust_l1, 
            replace_block, 
            derivation_origin_update 
        }
    }
    
    /// Creates a default ManagedEvent with all fields set to None.
    pub fn default() -> Self {
        Self { 
            reset: None, 
            unsafe_block: None, 
            derivation_update: None, 
            exhaust_l1: None, 
            replace_block: None, 
            derivation_origin_update: None 
        }
    }
}