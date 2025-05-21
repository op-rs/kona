//! Defines the basic types required for communication between supervisor and op-node.

use alloy_primitives::B256;
use alloy_eips::BlockId;
use op_alloy_consensus::OpReceiptEnvelope;
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub struct BlockSeal {
    pub hash: B256,
    pub number: u64,
    pub timestamp: u64
}

impl BlockSeal {
    #[allow(dead_code)]
    #[allow(unreachable_pub)]
    pub fn new(hash: B256, number: u64, timestamp: u64) -> Self {
        Self { hash, number, timestamp }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub struct BlockRef {
    pub hash: B256,
    pub number: u64,
    pub parent_hash: B256,
    pub time: u64
}

impl BlockRef {
    #[allow(dead_code)]
    #[allow(unreachable_pub)]
    pub fn new(hash: B256, number: u64, parent_hash: B256, time: u64) -> Self {
        Self { hash, number, parent_hash, time }
    }
}

#[allow(dead_code)]
type L1BlockRef = BlockRef;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub struct L2BlockRef<T = BlockId> {
    pub hash: B256,
    pub number: u64,
    pub parent_hash: B256,
    pub time: u64,
    pub l1_origin: T,
    pub sequence_number: u64 // distance to first block of epoch
}

impl<T> L2BlockRef<T> {
    #[allow(dead_code)]
    #[allow(unreachable_pub)]
    pub fn new(hash: B256, number: u64, parent_hash: B256, time: u64, l1_origin: T, sequence_number: u64) -> Self {
        Self { hash, number, parent_hash, time, l1_origin, sequence_number }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub struct DerivedBlockRefPair<T = BlockRef> {
    pub source: T,
    pub derived: T
}

impl<T> DerivedBlockRefPair<T> {
    #[allow(dead_code)]
    #[allow(unreachable_pub)]
    pub fn new(source: T, derived: T) -> Self {
        Self { source, derived }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub struct BlockReplacement<T = BlockRef> {
    pub replacement: T,
    pub invalidated: B256
}

impl<T> BlockReplacement<T> {
    #[allow(dead_code)]
    #[allow(unreachable_pub)]
    pub fn new(replacement: T, invalidated: B256) -> Self {
        Self { replacement, invalidated }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
#[allow(unreachable_pub)]
pub struct OutputV0 {
    pub state_root: B256,
    pub message_passer_storage_root: B256,
    pub block_hash: B256
}

impl OutputV0 {
    #[allow(dead_code)]
    #[allow(unreachable_pub)]
    pub fn new(state_root: B256, message_passer_storage_root: B256, block_hash: B256) -> Self {
        Self { state_root, message_passer_storage_root, block_hash }
    }
}

#[allow(dead_code)]
type Receipts = Vec<OpReceiptEnvelope>;

/// Sent by the node to the supervisor to share updates. 
/// Atleast one of the fields will be Some, and the rest will be None.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[allow(dead_code)]
#[allow(unreachable_pub)]
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
    #[allow(dead_code)]
    #[allow(unreachable_pub)]   
    pub fn new(reset: String, unsafe_block: Option<BlockRef>, derivation_update: Option<DerivedBlockRefPair>, exhaust_l1: Option<DerivedBlockRefPair>, replace_block: Option<BlockReplacement>, derivation_origin_update: Option<BlockRef>) -> Self {
        Self { reset, unsafe_block, derivation_update, exhaust_l1, replace_block, derivation_origin_update }
    }
}
