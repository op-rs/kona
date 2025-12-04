use alloy_primitives::{Address, B256};
use ambassador::{Delegate, delegatable_trait};

use crate::info::bedrock_base::ambassador_impl_L1BlockInfoBedrockBaseFields;
use crate::info::{
    HasBaseField, L1BlockInfoBedrockBaseFields, bedrock_base::L1BlockInfoBedrockBase,
};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Default, Copy, Delegate)]
#[delegate(L1BlockInfoBedrockBaseFields, target = "base")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct L1BlockInfoEcotoneBase {
    base: L1BlockInfoBedrockBase,
    /// The current blob base fee on L1
    pub blob_base_fee: u128,
    /// The fee scalar for L1 blobspace data
    pub blob_base_fee_scalar: u32,
    /// The fee scalar for L1 data
    pub base_fee_scalar: u32,
}

impl HasBaseField<L1BlockInfoBedrockBase> for L1BlockInfoEcotoneBase {
    fn base(&self) -> L1BlockInfoBedrockBase {
        self.base
    }
}

impl L1BlockInfoEcotoneBase {
    /// Construct new from all values.
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        number: u64,
        time: u64,
        base_fee: u64,
        block_hash: B256,
        sequence_number: u64,
        batcher_address: Address,
        blob_base_fee: u128,
        blob_base_fee_scalar: u32,
        base_fee_scalar: u32,
    ) -> Self {
        Self {
            base: L1BlockInfoBedrockBase::new(
                number,
                time,
                base_fee,
                block_hash,
                sequence_number,
                batcher_address,
            ),
            blob_base_fee,
            blob_base_fee_scalar,
            base_fee_scalar,
        }
    }
    /// Construct from default values and `base_fee`.
    pub(crate) fn new_from_base_fee(base_fee: u64) -> Self {
        Self { base: L1BlockInfoBedrockBase::new_from_base_fee(base_fee), ..Default::default() }
    }
    /// Construct from default values and `block_hash`.
    pub(crate) fn new_from_block_hash(block_hash: B256) -> Self {
        let base = L1BlockInfoBedrockBase::new_from_block_hash(block_hash);
        Self { base, ..Default::default() }
    }
    /// Construct from default values and `sequence_number`.
    pub(crate) fn new_from_sequence_number(sequence_number: u64) -> Self {
        Self {
            base: L1BlockInfoBedrockBase::new_from_sequence_number(sequence_number),
            ..Default::default()
        }
    }
    /// Construct from default values and `batcher_address`.
    pub(crate) fn new_from_batcher_address(batcher_address: Address) -> Self {
        Self {
            base: L1BlockInfoBedrockBase::new_from_batcher_address(batcher_address),
            ..Default::default()
        }
    }
    /// Construct from default values and `blob_base_fee`.
    pub(crate) fn new_from_blob_base_fee(blob_base_fee: u128) -> Self {
        Self { base: Default::default(), blob_base_fee, ..Default::default() }
    }
    /// Construct from default values and `blob_base_fee_scalar`.
    pub(crate) fn new_from_blob_base_fee_scalar(blob_base_fee_scalar: u32) -> Self {
        Self {
            base: Default::default(),
            base_fee_scalar: blob_base_fee_scalar,
            ..Default::default()
        }
    }
    /// Construct from default values and `base_fee_scalar`.
    pub(crate) fn new_from_base_fee_scalar(base_fee_scalar: u32) -> Self {
        Self { base: Default::default(), base_fee_scalar, ..Default::default() }
    }
    /// Construct from default values, `number` and `block_hash`.
    pub(crate) fn new_from_number_and_block_hash(number: u64, block_hash: B256) -> Self {
        let base = L1BlockInfoBedrockBase::new_from_number_and_block_hash(number, block_hash);
        Self { base, ..Default::default() }
    }
}

/*
impl L1BlockInfoBedrockBaseFields for L1BlockInfoEcotoneBase {
    fn number(&self) -> u64 {
        self.base().number()
    }

    fn time(&self) -> u64 {
        self.base().time()
    }

    fn base_fee(&self) -> u64 {
        self.base().base_fee()
    }

    fn block_hash(&self) -> B256 {
        self.base().block_hash()
    }

    fn sequence_number(&self) -> u64 {
        self.base().sequence_number()
    }

    fn batcher_address(&self) -> Address {
        self.base().batcher_address()
    }
}
    */

/// Accessors to fields in Ecotone and later.
#[delegatable_trait]
pub trait L1BlockInfoEcotoneBaseFields: L1BlockInfoBedrockBaseFields {
    /// The current blob base fee on L1
    fn blob_base_fee(&self) -> u128;
    /// The fee scalar for L1 blobspace data
    fn blob_base_fee_scalar(&self) -> u32;
    /// The fee scalar for L1 data
    fn base_fee_scalar(&self) -> u32;
}

impl L1BlockInfoEcotoneBaseFields for L1BlockInfoEcotoneBase {
    /// The current blob base fee on L1
    fn blob_base_fee(&self) -> u128 {
        self.blob_base_fee
    }
    /// The fee scalar for L1 blobspace data
    fn blob_base_fee_scalar(&self) -> u32 {
        self.blob_base_fee_scalar
    }
    /// The fee scalar for L1 data
    fn base_fee_scalar(&self) -> u32 {
        self.base_fee_scalar
    }
}
