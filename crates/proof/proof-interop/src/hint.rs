//! This module contains the [HintType] enum.

use alloc::{string::ToString, vec::Vec};
use core::{fmt::Display, str::FromStr};
use kona_proof::{errors::HintParsingError, Hint};

/// The [HintType] enum is used to specify the type of hint that was received.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HintType {
    /// A hint that specifies the block header of a layer 1 block.
    L1BlockHeader,
    /// A hint that specifies the transactions of a layer 1 block.
    L1Transactions,
    /// A hint that specifies the state node of a layer 1 block.
    L1Receipts,
    /// A hint that specifies a blob in the layer 1 beacon chain.
    L1Blob,
    /// A hint that specifies a precompile call on layer 1.
    L1Precompile,
    /// A hint that specifies the block header of a layer 2 block.
    L2BlockHeader,
    /// A hint that specifies the transactions of a layer 2 block.
    L2Transactions,
    /// A hint that specifies the receipts of a layer 2 block.
    L2Receipts,
    /// A hint that specifies the code of a contract on layer 2.
    L2Code,
    /// A hint that specifies the preimage of the agreed upon pre-state claim.
    AgreedPreState,
    /// A hint that specifies the preimage of an L2 output root within the agreed upon pre-state,
    /// by chain ID.
    L2OutputRoot,
    /// A hint that specifies the state node in the L2 state trie.
    L2StateNode,
    /// A hint that specifies the proof on the path to an account in the L2 state trie.
    L2AccountProof,
    /// A hint that specifies the proof on the path to a storage slot in an account within in the
    /// L2 state trie.
    L2AccountStorageProof,
    /// A hint that specifies loading the payload witness for an optimistic block.
    L2BlockData,
}

impl HintType {
    /// Creates a new [Hint] from `self` and the specified data. The data passed will be
    /// concatenated into a single byte array before being stored in the resulting [Hint].
    pub fn with_data(self, data: &[&[u8]]) -> Hint<Self> {
        let total_len = data.iter().map(|d| d.len()).sum();
        let hint_data = data.iter().fold(Vec::with_capacity(total_len), |mut acc, d| {
            acc.extend_from_slice(d);
            acc
        });
        Hint::new(self, hint_data)
    }
}

impl FromStr for HintType {
    type Err = HintParsingError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "l1-block-header" => Ok(Self::L1BlockHeader),
            "l1-transactions" => Ok(Self::L1Transactions),
            "l1-receipts" => Ok(Self::L1Receipts),
            "l1-blob" => Ok(Self::L1Blob),
            "l1-precompile" => Ok(Self::L1Precompile),
            "l2-block-header" => Ok(Self::L2BlockHeader),
            "l2-transactions" => Ok(Self::L2Transactions),
            "l2-receipts" => Ok(Self::L2Receipts),
            "l2-code" => Ok(Self::L2Code),
            "agreed-pre-state" => Ok(Self::AgreedPreState),
            "l2-output-root" => Ok(Self::L2OutputRoot),
            "l2-state-node" => Ok(Self::L2StateNode),
            "l2-account-proof" => Ok(Self::L2AccountProof),
            "l2-account-storage-proof" => Ok(Self::L2AccountStorageProof),
            "l2-block-data" => Ok(Self::L2BlockData),
            _ => Err(HintParsingError(value.to_string())),
        }
    }
}

impl From<HintType> for &str {
    fn from(value: HintType) -> Self {
        match value {
            HintType::L1BlockHeader => "l1-block-header",
            HintType::L1Transactions => "l1-transactions",
            HintType::L1Receipts => "l1-receipts",
            HintType::L1Blob => "l1-blob",
            HintType::L1Precompile => "l1-precompile",
            HintType::L2BlockHeader => "l2-block-header",
            HintType::L2Transactions => "l2-transactions",
            HintType::L2Receipts => "l2-receipts",
            HintType::L2Code => "l2-code",
            HintType::AgreedPreState => "agreed-pre-state",
            HintType::L2OutputRoot => "l2-output-root",
            HintType::L2StateNode => "l2-state-node",
            HintType::L2AccountProof => "l2-account-proof",
            HintType::L2AccountStorageProof => "l2-account-storage-proof",
            HintType::L2BlockData => "l2-block-data",
        }
    }
}

impl Display for HintType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s: &str = (*self).into();
        write!(f, "{}", s)
    }
}
