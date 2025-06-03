//! Supervisor RPC response types.

use alloy_eips::BlockNumHash;
use alloy_primitives::{ChainId, map::HashMap};
use kona_protocol::BlockInfo;
use kona_supervisor_types::SuperHead;

/// Describes superchain sync status.
///
/// Specs: <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#supervisorsyncstatus>.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct SupervisorSyncStatus {
    /// [`BlockInfo`] of highest L1 block.
    pub min_synced_l1: BlockInfo,
    /// Timestamp of highest safe block.
    pub safe_timestamp: u64,
    /// Timestamp of highest finalized block.
    pub finalized_timestamp: u64,
    /// Map of all tracked chains and their individual [`SupervisorChainSyncStatus`].
    pub chains: HashMap<ChainId, SupervisorChainSyncStatus>,
}

/// Describes the sync status for a specific chain.
///
/// Specs: <https://github.com/ethereum-optimism/specs/blob/main/specs/interop/supervisor.md#supervisorchainsyncstatus>
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub struct SupervisorChainSyncStatus {
    /// Highest [`Unsafe`] head of chain [`BlockRef`].
    ///
    /// [`LocalUnsafe`]: op_alloy_consensus::interop::SafetyLevel::Unsafe
    pub r#unsafe: BlockInfo,
    /// Highest [`CrossUnsafe`] head of chain.
    ///
    /// [`CrossUnsafe`]: op_alloy_consensus::interop::SafetyLevel::CrossUnsafe
    pub cross_unsafe: BlockNumHash,
    /// Highest [`LocalSafe`] head of chain.
    ///
    /// [`LocalSafe`]: op_alloy_consensus::interop::SafetyLevel::LocalSafe
    pub local_safe: BlockNumHash,
    /// Highest [`Safe`] head of chain [`BlockNumHash`].
    ///
    /// [`Safe`]: op_alloy_consensus::interop::SafetyLevel::Safe
    pub safe: BlockNumHash,
    /// Highest [`Finalized`] head of chain [`BlockNumHash`].
    ///
    /// [`Finalized`]: op_alloy_consensus::interop::SafetyLevel::Finalized
    pub finalized: BlockNumHash,
}

impl From<SuperHead> for SupervisorChainSyncStatus {
    fn from(super_head: SuperHead) -> Self {
        let SuperHead { r#unsafe, cross_unsafe, local_safe, safe, finalized, .. } = super_head;

        Self {
            r#unsafe,
            local_safe: BlockNumHash::new(local_safe.number, local_safe.hash),
            cross_unsafe: BlockNumHash::new(cross_unsafe.number, cross_unsafe.hash),
            safe: BlockNumHash::new(safe.number, safe.hash),
            finalized: BlockNumHash::new(finalized.number, finalized.hash),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy_primitives::b256;

    #[cfg(feature = "serde")]
    #[test]
    fn test_serialize_supervisor_chain_sync_status() {
        const STATUS: &'static str = r#"
            {
                "unsafe": {
                    "number": 100,
                    "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
                    "timestamp": 40044440000,
                    "parentHash": "0x111def1234567890abcdef1234567890abcdef1234500000abcdef123456aaaa"
                },
                "crossUnsafe": {
                    "number": 90,
                    "hash": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
                },
                "localSafe": {
                    "number": 80,
                    "hash": "0x34567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef13"
                },
                "safe": {
                    "number": 70,
                    "hash": "0x567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234"
                },
                "finalized": {
                    "number": 60,
                    "hash": "0x34567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12"
                }
            }"#;

        assert_eq!(
            serde_json::from_str::<SupervisorChainSyncStatus>(STATUS).expect("should deserialize"),
            SupervisorChainSyncStatus {
                r#unsafe: BlockInfo {
                    number: 100,
                    hash: b256!(
                        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                    ),
                    timestamp: 40044440000,
                    parent_hash: b256!(
                        "0x111def1234567890abcdef1234567890abcdef1234500000abcdef123456aaaa"
                    ),
                },
                cross_unsafe: BlockNumHash::new(
                    90,
                    b256!("0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890")
                ),
                local_safe: BlockNumHash::new(
                    80,
                    b256!("0x34567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef13")
                ),
                safe: BlockNumHash::new(
                    70,
                    b256!("0x567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234")
                ),
                finalized: BlockNumHash::new(
                    60,
                    b256!("0x34567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12")
                ),
            }
        )
    }
}
