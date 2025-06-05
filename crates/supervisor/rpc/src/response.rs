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
    /// Timestamp of highest cross-safe block.
    ///
    /// NOTE: Some fault-proof releases may already depend on `safe`, so we keep JSON field name as
    /// `safe`.
    #[cfg_attr(feature = "serde", serde(rename = "safeTimestamp"))]
    pub cross_safe_timestamp: u64,
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
    /// Highest [`Unsafe`] head of chain.
    ///
    /// [`Unsafe`]: op_alloy_consensus::interop::SafetyLevel::Unsafe
    pub local_unsafe: BlockInfo,
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
    /// NOTE: Some fault-proof releases may already depend on `safe`, so we keep JSON field name as
    /// `safe`.
    ///
    /// [`Safe`]: op_alloy_consensus::interop::SafetyLevel::Safe
    #[cfg_attr(feature = "serde", serde(rename = "safe"))]
    pub cross_safe: BlockNumHash,
    /// Highest [`Finalized`] head of chain [`BlockNumHash`].
    ///
    /// [`Finalized`]: op_alloy_consensus::interop::SafetyLevel::Finalized
    pub finalized: BlockNumHash,
}

impl From<SuperHead> for SupervisorChainSyncStatus {
    fn from(super_head: SuperHead) -> Self {
        let SuperHead { local_unsafe, cross_unsafe, local_safe, cross_safe, finalized, .. } =
            super_head;

        Self {
            local_unsafe,
            local_safe: BlockNumHash::new(local_safe.number, local_safe.hash),
            cross_unsafe: BlockNumHash::new(cross_unsafe.number, cross_unsafe.hash),
            cross_safe: BlockNumHash::new(cross_safe.number, cross_safe.hash),
            finalized: BlockNumHash::new(finalized.number, finalized.hash),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy_primitives::b256;

    const CHAIN_STATUS: &str = r#"
    {
        "localUnsafe": {
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

    const STATUS: &str = r#"
    {
        "minSyncedL1": {
            "number": 100,
            "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            "timestamp": 40044440000,
            "parentHash": "0x111def1234567890abcdef1234567890abcdef1234500000abcdef123456aaaa"
        },
        "safeTimestamp": 40044450000,
        "finalizedTimestamp": 40044460000,
        "chains" : {
            "1": {
                "localUnsafe": {
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
            }
        }
    }"#;

    #[cfg(feature = "serde")]
    #[test]
    fn test_serialize_supervisor_chain_sync_status() {
        assert_eq!(
            serde_json::from_str::<SupervisorChainSyncStatus>(CHAIN_STATUS)
                .expect("should deserialize"),
            SupervisorChainSyncStatus {
                local_unsafe: BlockInfo {
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
                cross_safe: BlockNumHash::new(
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

    #[cfg(feature = "serde")]
    #[test]
    fn test_serialize_supervisor_sync_status() {
        let mut chains = HashMap::default();

        chains.insert(
            1,
            SupervisorChainSyncStatus {
                local_unsafe: BlockInfo {
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
                    b256!("0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"),
                ),
                local_safe: BlockNumHash::new(
                    80,
                    b256!("0x34567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef13"),
                ),
                cross_safe: BlockNumHash::new(
                    70,
                    b256!("0x567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234"),
                ),
                finalized: BlockNumHash::new(
                    60,
                    b256!("0x34567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef12"),
                ),
            },
        );

        assert_eq!(
            serde_json::from_str::<SupervisorSyncStatus>(STATUS).expect("should deserialize"),
            SupervisorSyncStatus {
                min_synced_l1: BlockInfo {
                    number: 100,
                    hash: b256!(
                        "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                    ),
                    timestamp: 40044440000,
                    parent_hash: b256!(
                        "0x111def1234567890abcdef1234567890abcdef1234500000abcdef123456aaaa"
                    ),
                },
                cross_safe_timestamp: 40044450000,
                finalized_timestamp: 40044460000,
                chains,
            }
        )
    }
}
