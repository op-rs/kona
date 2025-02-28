//! Block Types for Optimism.

use crate::{DecodeError, L1BlockInfoTx};
use alloy_consensus::{Block, Transaction, Typed2718};
use alloy_eips::{eip2718::Eip2718Error, BlockNumHash};
use alloy_primitives::B256;
use kona_genesis::ChainGenesis;
use op_alloy_consensus::OpBlock;

/// Block Header Info
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Default)]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct BlockInfo {
    /// The block hash
    pub hash: B256,
    /// The block number
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub number: u64,
    /// The parent block hash
    pub parent_hash: B256,
    /// The block timestamp
    #[cfg_attr(feature = "serde", serde(with = "alloy_serde::quantity"))]
    pub timestamp: u64,
}

impl BlockInfo {
    /// Instantiates a new [BlockInfo].
    pub const fn new(hash: B256, number: u64, parent_hash: B256, timestamp: u64) -> Self {
        Self { hash, number, parent_hash, timestamp }
    }

    /// Returns the block ID.
    pub const fn id(&self) -> BlockNumHash {
        BlockNumHash { hash: self.hash, number: self.number }
    }
}

impl<T> From<Block<T>> for BlockInfo {
    fn from(block: Block<T>) -> Self {
        Self::from(&block)
    }
}

impl<T> From<&Block<T>> for BlockInfo {
    fn from(block: &Block<T>) -> Self {
        Self {
            hash: block.header.hash_slow(),
            number: block.header.number,
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
        }
    }
}

impl core::fmt::Display for BlockInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "BlockInfo {{ hash: {}, number: {}, parent_hash: {}, timestamp: {} }}",
            self.hash, self.number, self.parent_hash, self.timestamp
        )
    }
}

/// L2 Block Header Info
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct L2BlockInfo {
    /// The base [BlockInfo]
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub block_info: BlockInfo,
    /// The L1 origin [BlockNumHash]
    #[cfg_attr(feature = "serde", serde(rename = "l1origin", alias = "l1Origin"))]
    pub l1_origin: BlockNumHash,
    /// The sequence number of the L2 block
    #[cfg_attr(
        feature = "serde",
        serde(with = "alloy_serde::quantity", rename = "sequenceNumber", alias = "seqNum")
    )]
    pub seq_num: u64,
}

#[cfg(feature = "arbitrary")]
impl arbitrary::Arbitrary<'_> for L2BlockInfo {
    fn arbitrary(g: &mut arbitrary::Unstructured<'_>) -> arbitrary::Result<Self> {
        Ok(Self {
            block_info: g.arbitrary()?,
            l1_origin: BlockNumHash { number: g.arbitrary()?, hash: g.arbitrary()? },
            seq_num: g.arbitrary()?,
        })
    }
}

/// An error that can occur when converting an OP [Block] to [L2BlockInfo].
#[derive(Debug, Clone, thiserror::Error)]
pub enum FromBlockError {
    /// The genesis block hash does not match the expected value.
    #[error("Invalid genesis hash")]
    InvalidGenesisHash,
    /// The L2 block is missing the L1 info deposit transaction.
    #[error("L2 block is missing L1 info deposit transaction ({0})")]
    MissingL1InfoDeposit(B256),
    /// The first payload transaction has an unexpected type.
    #[error("First payload transaction has unexpected type: {0}")]
    UnexpectedTxType(u8),
    /// Failed to decode the first transaction into an OP transaction.
    #[error("Failed to decode the first transaction into an OP transaction: {0}")]
    TxEnvelopeDecodeError(Eip2718Error),
    /// The first payload transaction is not a deposit transaction.
    #[error("First payload transaction is not a deposit transaction, type: {0}")]
    FirstTxNonDeposit(u8),
    /// Failed to decode the [L1BlockInfoTx] from the deposit transaction.
    #[error("Failed to decode the L1BlockInfoTx from the deposit transaction: {0}")]
    BlockInfoDecodeError(#[from] DecodeError),
}

impl PartialEq<Self> for FromBlockError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InvalidGenesisHash, Self::InvalidGenesisHash) => true,
            (Self::MissingL1InfoDeposit(a), Self::MissingL1InfoDeposit(b)) => a == b,
            (Self::UnexpectedTxType(a), Self::UnexpectedTxType(b)) => a == b,
            (Self::TxEnvelopeDecodeError(_), Self::TxEnvelopeDecodeError(_)) => true,
            (Self::FirstTxNonDeposit(a), Self::FirstTxNonDeposit(b)) => a == b,
            (Self::BlockInfoDecodeError(a), Self::BlockInfoDecodeError(b)) => a == b,
            _ => false,
        }
    }
}

impl From<Eip2718Error> for FromBlockError {
    fn from(value: Eip2718Error) -> Self {
        Self::TxEnvelopeDecodeError(value)
    }
}

impl L2BlockInfo {
    /// Instantiates a new [L2BlockInfo].
    pub const fn new(block_info: BlockInfo, l1_origin: BlockNumHash, seq_num: u64) -> Self {
        Self { block_info, l1_origin, seq_num }
    }

    /// Constructs an [L2BlockInfo] from a given OP [Block] and [ChainGenesis].
    pub fn from_block_and_genesis(
        block: &OpBlock,
        genesis: &ChainGenesis,
    ) -> Result<Self, FromBlockError> {
        let block_info = BlockInfo::from(block);

        let (l1_origin, sequence_number) = if block_info.number == genesis.l2.number {
            if block_info.hash != genesis.l2.hash {
                return Err(FromBlockError::InvalidGenesisHash);
            }
            (genesis.l1, 0)
        } else {
            if block.body.transactions.is_empty() {
                return Err(FromBlockError::MissingL1InfoDeposit(block_info.hash));
            }

            let tx = &block.body.transactions[0];

            let Some(tx) = tx.as_deposit() else {
                return Err(FromBlockError::FirstTxNonDeposit(tx.ty()));
            };

            let l1_info = L1BlockInfoTx::decode_calldata(tx.input().as_ref())
                .map_err(FromBlockError::BlockInfoDecodeError)?;
            (l1_info.id(), l1_info.sequence_number())
        };

        Ok(Self { block_info, l1_origin, seq_num: sequence_number })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloy_consensus::{Header, TxEnvelope};
    use alloy_primitives::b256;

    #[test]
    fn test_from_block_error_partial_eq() {
        assert_eq!(FromBlockError::InvalidGenesisHash, FromBlockError::InvalidGenesisHash);
        assert_eq!(
            FromBlockError::MissingL1InfoDeposit(b256!(
                "04d6fefc87466405ba0e5672dcf5c75325b33e5437da2a42423080aab8be889b"
            )),
            FromBlockError::MissingL1InfoDeposit(b256!(
                "04d6fefc87466405ba0e5672dcf5c75325b33e5437da2a42423080aab8be889b"
            )),
        );
        assert_eq!(FromBlockError::UnexpectedTxType(1), FromBlockError::UnexpectedTxType(1));
        assert_eq!(
            FromBlockError::TxEnvelopeDecodeError(Eip2718Error::UnexpectedType(1)),
            FromBlockError::TxEnvelopeDecodeError(Eip2718Error::UnexpectedType(1))
        );
        assert_eq!(FromBlockError::FirstTxNonDeposit(1), FromBlockError::FirstTxNonDeposit(1));
        assert_eq!(
            FromBlockError::BlockInfoDecodeError(DecodeError::InvalidSelector),
            FromBlockError::BlockInfoDecodeError(DecodeError::InvalidSelector)
        );
    }

    #[test]
    fn test_l2_block_info_invalid_genesis_hash() {
        let genesis = ChainGenesis {
            l1: BlockNumHash { hash: B256::from([4; 32]), number: 2 },
            l2: BlockNumHash { hash: B256::from([5; 32]), number: 1 },
            ..Default::default()
        };
        let op_block = OpBlock {
            header: Header {
                number: 1,
                parent_hash: B256::from([2; 32]),
                timestamp: 1,
                ..Default::default()
            },
            body: Default::default(),
        };
        let err = L2BlockInfo::from_block_and_genesis(&op_block, &genesis).unwrap_err();
        assert_eq!(err, FromBlockError::InvalidGenesisHash);
    }

    #[test]
    fn test_from_block() {
        let block: Block<TxEnvelope, Header> = Block {
            header: Header {
                number: 1,
                parent_hash: B256::from([2; 32]),
                timestamp: 1,
                ..Default::default()
            },
            body: Default::default(),
        };
        let block_info = BlockInfo::from(&block);
        assert_eq!(
            block_info,
            BlockInfo {
                hash: b256!("04d6fefc87466405ba0e5672dcf5c75325b33e5437da2a42423080aab8be889b"),
                number: block.header.number,
                parent_hash: block.header.parent_hash,
                timestamp: block.header.timestamp,
            }
        );
    }

    #[test]
    fn test_block_info_display() {
        let hash = B256::from([1; 32]);
        let parent_hash = B256::from([2; 32]);
        let block_info = BlockInfo::new(hash, 1, parent_hash, 1);
        assert_eq!(
            block_info.to_string(),
            "BlockInfo { hash: 0x0101010101010101010101010101010101010101010101010101010101010101, number: 1, parent_hash: 0x0202020202020202020202020202020202020202020202020202020202020202, timestamp: 1 }"
        );
    }

    #[test]
    #[cfg(feature = "arbitrary")]
    fn test_arbitrary_block_info() {
        use arbitrary::Arbitrary;
        use rand::Rng;
        let mut bytes = [0u8; 1024];
        rand::rng().fill(bytes.as_mut_slice());
        BlockInfo::arbitrary(&mut arbitrary::Unstructured::new(&bytes)).unwrap();
    }

    #[test]
    #[cfg(feature = "arbitrary")]
    fn test_arbitrary_l2_block_info() {
        use arbitrary::Arbitrary;
        use rand::Rng;
        let mut bytes = [0u8; 1024];
        rand::rng().fill(bytes.as_mut_slice());
        L2BlockInfo::arbitrary(&mut arbitrary::Unstructured::new(&bytes)).unwrap();
    }

    #[test]
    fn test_block_id_bounds() {
        let block_info = BlockInfo {
            hash: B256::from([1; 32]),
            number: 0,
            parent_hash: B256::from([2; 32]),
            timestamp: 1,
        };
        let expected = BlockNumHash { hash: B256::from([1; 32]), number: 0 };
        assert_eq!(block_info.id(), expected);

        let block_info = BlockInfo {
            hash: B256::from([1; 32]),
            number: u64::MAX,
            parent_hash: B256::from([2; 32]),
            timestamp: 1,
        };
        let expected = BlockNumHash { hash: B256::from([1; 32]), number: u64::MAX };
        assert_eq!(block_info.id(), expected);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_deserialize_block_info() {
        let block_info = BlockInfo {
            hash: B256::from([1; 32]),
            number: 1,
            parent_hash: B256::from([2; 32]),
            timestamp: 1,
        };

        let json = r#"{
            "hash": "0x0101010101010101010101010101010101010101010101010101010101010101",
            "number": 1,
            "parentHash": "0x0202020202020202020202020202020202020202020202020202020202020202",
            "timestamp": 1
        }"#;

        let deserialized: BlockInfo = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, block_info);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_deserialize_block_info_with_hex() {
        let block_info = BlockInfo {
            hash: B256::from([1; 32]),
            number: 1,
            parent_hash: B256::from([2; 32]),
            timestamp: 1,
        };

        let json = r#"{
            "hash": "0x0101010101010101010101010101010101010101010101010101010101010101",
            "number": "0x1",
            "parentHash": "0x0202020202020202020202020202020202020202020202020202020202020202",
            "timestamp": "0x1"
        }"#;

        let deserialized: BlockInfo = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, block_info);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_deserialize_l2_block_info() {
        let l2_block_info = L2BlockInfo {
            block_info: BlockInfo {
                hash: B256::from([1; 32]),
                number: 1,
                parent_hash: B256::from([2; 32]),
                timestamp: 1,
            },
            l1_origin: BlockNumHash { hash: B256::from([3; 32]), number: 2 },
            seq_num: 3,
        };

        let json = r#"{
            "hash": "0x0101010101010101010101010101010101010101010101010101010101010101",
            "number": 1,
            "parentHash": "0x0202020202020202020202020202020202020202020202020202020202020202",
            "timestamp": 1,
            "l1origin": {
                "hash": "0x0303030303030303030303030303030303030303030303030303030303030303",
                "number": 2
            },
            "sequenceNumber": 3
        }"#;

        let deserialized: L2BlockInfo = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, l2_block_info);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_deserialize_l2_block_info_hex() {
        let l2_block_info = L2BlockInfo {
            block_info: BlockInfo {
                hash: B256::from([1; 32]),
                number: 1,
                parent_hash: B256::from([2; 32]),
                timestamp: 1,
            },
            l1_origin: BlockNumHash { hash: B256::from([3; 32]), number: 2 },
            seq_num: 3,
        };

        let json = r#"{
            "hash": "0x0101010101010101010101010101010101010101010101010101010101010101",
            "number": "0x1",
            "parentHash": "0x0202020202020202020202020202020202020202020202020202020202020202",
            "timestamp": "0x1",
            "l1origin": {
                "hash": "0x0303030303030303030303030303030303030303030303030303030303030303",
                "number": 2
            },
            "sequenceNumber": "0x3"
        }"#;

        let deserialized: L2BlockInfo = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, l2_block_info);
    }
}
