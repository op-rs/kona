//! Optimism Payload attributes that reference the parent L2 block.

use kona_protocol::L2BlockInfo;
use op_alloy_consensus::OpTxType;
use op_alloy_rpc_types_engine::OpPayloadAttributes;

/// Optimism Payload Attributes with parent block reference.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpAttributesWithParent {
    /// The payload attributes.
    pub attributes: OpPayloadAttributes,
    /// The parent block reference.
    pub parent: L2BlockInfo,
    /// Whether the current batch is the last in its span.
    pub is_last_in_span: bool,
}

impl OpAttributesWithParent {
    /// Create a new [OpAttributesWithParent] instance.
    pub const fn new(
        attributes: OpPayloadAttributes,
        parent: L2BlockInfo,
        is_last_in_span: bool,
    ) -> Self {
        Self { attributes, parent, is_last_in_span }
    }

    /// Returns the payload attributes.
    pub const fn attributes(&self) -> &OpPayloadAttributes {
        &self.attributes
    }

    /// Returns the parent block reference.
    pub const fn parent(&self) -> &L2BlockInfo {
        &self.parent
    }

    /// Returns whether the current batch is the last in its span.
    pub const fn is_last_in_span(&self) -> bool {
        self.is_last_in_span
    }

    /// Returns `true` if all transactions in the payload are deposits.
    pub fn is_deposits_only(&self) -> bool {
        self.attributes
            .transactions
            .iter()
            .all(|tx| tx.first().is_some_and(|tx| tx[0] == OpTxType::Deposit as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_attributes_with_parent() {
        let attributes = OpPayloadAttributes::default();
        let parent = L2BlockInfo::default();
        let is_last_in_span = true;
        let op_attributes_with_parent =
            OpAttributesWithParent::new(attributes.clone(), parent, is_last_in_span);

        assert_eq!(op_attributes_with_parent.attributes(), &attributes);
        assert_eq!(op_attributes_with_parent.parent(), &parent);
        assert_eq!(op_attributes_with_parent.is_last_in_span(), is_last_in_span);
    }
}
