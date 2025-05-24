//! Optimism Payload attributes that reference the parent L2 block.

use crate::{BlockInfo, L2BlockInfo};
use alloc::vec;
use op_alloy_consensus::OpTxType;
use op_alloy_rpc_types_engine::OpPayloadAttributes;

/// Optimism Payload Attributes with parent block reference and the L1 origin block.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpAttributesWithParent {
    /// The payload attributes.
    pub inner: OpPayloadAttributes,
    /// The parent block reference.
    pub parent: L2BlockInfo,
    /// The L1 block that the attributes were derived from.
    pub l1_origin: BlockInfo,
    /// Whether the current batch is the last in its span.
    pub is_last_in_span: bool,
}

impl OpAttributesWithParent {
    /// Create a new [OpAttributesWithParent] instance.
    pub const fn new(
        inner: OpPayloadAttributes,
        parent: L2BlockInfo,
        l1_origin: BlockInfo,
        is_last_in_span: bool,
    ) -> Self {
        Self { inner, parent, l1_origin, is_last_in_span }
    }

    /// Consumes `self` and returns the inner [`OpPayloadAttributes`].
    pub fn take_inner(self) -> OpPayloadAttributes {
        self.inner
    }

    /// Returns the payload attributes.
    pub const fn inner(&self) -> &OpPayloadAttributes {
        &self.inner
    }

    /// Returns the parent block reference.
    pub const fn parent(&self) -> &L2BlockInfo {
        &self.parent
    }

    /// Returns the L1 origin block reference.
    pub const fn l1_origin(&self) -> &BlockInfo {
        &self.l1_origin
    }

    /// Returns whether the current batch is the last in its span.
    pub const fn is_last_in_span(&self) -> bool {
        self.is_last_in_span
    }

    /// Returns `true` if all transactions in the payload are deposits.
    pub fn is_deposits_only(&self) -> bool {
        self.inner
            .transactions
            .iter()
            .all(|tx| tx.first().is_some_and(|tx| tx[0] == OpTxType::Deposit as u8))
    }

    /// Converts the [`OpAttributesWithParent`] into a deposits-only payload.
    pub fn as_deposits_only(&self) -> Self {
        Self {
            inner: OpPayloadAttributes {
                transactions: self.inner.transactions.as_ref().map(|txs| {
                    txs.iter()
                        .map(|_| alloy_primitives::Bytes::from(vec![OpTxType::Deposit as u8]))
                        .collect()
                }),
                ..self.inner.clone()
            },
            parent: self.parent,
            l1_origin: self.l1_origin,
            is_last_in_span: self.is_last_in_span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_attributes_with_parent() {
        let attributes = OpPayloadAttributes::default();
        let parent = L2BlockInfo::default();
        let l1_origin = BlockInfo::default();
        let is_last_in_span = true;
        let op_attributes_with_parent =
            OpAttributesWithParent::new(attributes.clone(), parent, l1_origin, is_last_in_span);

        assert_eq!(op_attributes_with_parent.inner(), &attributes);
        assert_eq!(op_attributes_with_parent.parent(), &parent);
        assert_eq!(op_attributes_with_parent.is_last_in_span(), is_last_in_span);
    }
}
