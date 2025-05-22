//! Defines the basic types required for communication between supervisor and op-node.

use alloy_primitives::{B256, U64};
use alloy_eips::BlockId;
use op_alloy_consensus::OpReceiptEnvelope;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSeal {
    pub hash: B256,
    pub number: U64,
    pub timestamp: U64
}

impl BlockSeal {
    pub fn new(hash: B256, number: U64, timestamp: U64) -> Self {
        Self { hash, number, timestamp }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockRef {
    pub hash: B256,
    pub number: U64,
    pub parent_hash: B256,
    pub timestamp: U64
}

impl BlockRef {
    pub fn new(hash: B256, number: U64, parent_hash: B256, timestamp: U64) -> Self {
        Self { hash, number, parent_hash, timestamp }
    }
}

pub type L1BlockRef = BlockRef;

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L2BlockRef<T = BlockId> {
    pub hash: B256,
    pub number: U64,
    pub parent_hash: B256,
    pub time: U64,
    pub l1_origin: T,
    pub sequence_number: U64 // distance to first block of epoch
}

impl<T> L2BlockRef<T> {
    pub fn new(hash: B256, number: U64, parent_hash: B256, time: U64, l1_origin: T, sequence_number: U64) -> Self {
        Self { hash, number, parent_hash, time, l1_origin, sequence_number }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedBlockRefPair<T = BlockRef> {
    pub source: T,
    pub derived: T
}

impl<T> DerivedBlockRefPair<T> {
    pub fn new(source: T, derived: T) -> Self {
        Self { source, derived }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockReplacement<T = BlockRef> {
    pub replacement: T,
    pub invalidated: B256
}

impl<T> BlockReplacement<T> {
    pub fn new(replacement: T, invalidated: B256) -> Self {
        Self { replacement, invalidated }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputV0 {
    pub state_root: B256,
    pub message_passer_storage_root: B256,
    pub block_hash: B256
}

impl OutputV0 {
    pub fn new(state_root: B256, message_passer_storage_root: B256, block_hash: B256) -> Self {
        Self { state_root, message_passer_storage_root, block_hash }
    }
}

pub type Receipts = Vec<OpReceiptEnvelope>;

/// Sent by the node to the supervisor to share updates. 
/// Atleast one of the fields will be Some, and the rest will be None.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]

pub struct ManagedEvent {
    /// The reset was successful
    pub reset: String,
    /// There was a new unsafe block
    pub unsafe_block: Option<BlockRef>,
    /// The node derived new L2 block from L1
    pub derivation_update: Option<DerivedBlockRefPair>,
    /// There are no more L1 blocks to process with the node
    pub exhaust_l1: Option<DerivedBlockRefPair>,
    /// The node has successfully replaced the block
    pub replace_block: Option<BlockReplacement>,
    /// The node has successfully updated the derivation origin
    pub derivation_origin_update: Option<BlockRef>,
}

impl ManagedEvent {
    pub fn new(reset: String, unsafe_block: Option<BlockRef>, derivation_update: Option<DerivedBlockRefPair>, exhaust_l1: Option<DerivedBlockRefPair>, replace_block: Option<BlockReplacement>, derivation_origin_update: Option<BlockRef>) -> Self {
        Self { reset, unsafe_block, derivation_update, exhaust_l1, replace_block, derivation_origin_update }
    }
    pub fn default() -> Self {
        Self { reset: "".to_string(), unsafe_block: None, derivation_update: None, exhaust_l1: None, replace_block: None, derivation_origin_update: None }
    }
}