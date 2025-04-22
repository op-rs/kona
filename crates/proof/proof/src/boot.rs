//! This module contains the prologue phase of the client program, pulling in the boot information
//! through the `PreimageOracle` ABI as local keys.

use crate::errors::OracleProviderError;
use alloy_primitives::{B256, U256};
use kona_genesis::RollupConfig;
use kona_preimage::{PreimageKey, PreimageOracleClient};
use kona_registry::ROLLUP_CONFIGS;
use serde::{Deserialize, Serialize};

/// The local key ident for the L1 head hash.
pub const L1_HEAD_KEY: U256 = U256::from_be_slice(&[1]);

/// The local key ident for the L2 output root.
pub const L2_OUTPUT_ROOT_KEY: U256 = U256::from_be_slice(&[2]);

/// The local key ident for the L2 output root claim.
pub const L2_CLAIM_KEY: U256 = U256::from_be_slice(&[3]);

/// The local key ident for the L2 claim block number.
pub const L2_CLAIM_BLOCK_NUMBER_KEY: U256 = U256::from_be_slice(&[4]);

/// The local key ident for the L2 chain ID.
pub const L2_CHAIN_ID_KEY: U256 = U256::from_be_slice(&[5]);

/// The local key ident for the L2 rollup config.
pub const L2_ROLLUP_CONFIG_KEY: U256 = U256::from_be_slice(&[6]);

/// The boot information for the client program.
///
/// **Verified inputs:**
/// - `l1_head`: The L1 head hash containing the safe L2 chain data that may reproduce the L2 head
///   hash.
/// - `agreed_l2_output_root`:The agreed upon safe L2 output root.
/// - `chain_id`: The L2 chain ID.
///
/// **User submitted inputs:**
/// - `claimed_l2_output_root`: The L2 output root claim.
/// - `claimed_l2_block_number`: The L2 claim block number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootInfo {
    /// The L1 head hash containing the safe L2 chain data that may reproduce the L2 head hash.
    pub l1_head: B256,
    /// The agreed upon safe L2 output root.
    pub agreed_l2_output_root: B256,
    /// The L2 output root claim.
    pub claimed_l2_output_root: B256,
    /// The L2 claim block number.
    pub claimed_l2_block_number: u64,
    /// The L2 chain ID.
    pub chain_id: u64,
    /// The rollup config for the L2 chain.
    pub rollup_config: RollupConfig,
}

impl BootInfo {
    /// Load the boot information from the preimage oracle.
    ///
    /// ## Takes
    /// - `oracle`: The preimage oracle reader.
    ///
    /// ## Returns
    /// - `Ok(BootInfo)`: The boot information.
    /// - `Err(_)`: Failed to load the boot information.
    pub async fn load<O>(oracle: &O) -> Result<Self, OracleProviderError>
    where
        O: PreimageOracleClient + Send,
    {
        let mut l1_head: B256 = B256::ZERO;
        oracle
            .get_exact(PreimageKey::new_local(L1_HEAD_KEY.to()), l1_head.as_mut())
            .await
            .map_err(OracleProviderError::Preimage)?;

        let mut l2_output_root: B256 = B256::ZERO;
        oracle
            .get_exact(PreimageKey::new_local(L2_OUTPUT_ROOT_KEY.to()), l2_output_root.as_mut())
            .await
            .map_err(OracleProviderError::Preimage)?;

        let mut l2_claim: B256 = B256::ZERO;
        oracle
            .get_exact(PreimageKey::new_local(L2_CLAIM_KEY.to()), l2_claim.as_mut())
            .await
            .map_err(OracleProviderError::Preimage)?;

        let l2_claim_block = u64::from_be_bytes(
            oracle
                .get(PreimageKey::new_local(L2_CLAIM_BLOCK_NUMBER_KEY.to()))
                .await
                .map_err(OracleProviderError::Preimage)?
                .as_slice()
                .try_into()
                .map_err(OracleProviderError::SliceConversion)?,
        );
        let chain_id = u64::from_be_bytes(
            oracle
                .get(PreimageKey::new_local(L2_CHAIN_ID_KEY.to()))
                .await
                .map_err(OracleProviderError::Preimage)?
                .as_slice()
                .try_into()
                .map_err(OracleProviderError::SliceConversion)?,
        );

        // Attempt to load the rollup config from the chain ID. If there is no config for the chain,
        // fall back to loading the config from the preimage oracle.
        let rollup_config = if let Some(config) = ROLLUP_CONFIGS.get(&chain_id) {
            config.clone()
        } else {
            warn!(
                target: "boot_loader",
                "No rollup config found for chain ID {}, falling back to preimage oracle. This is insecure in production without additional validation!",
                chain_id
            );
            let ser_cfg = oracle
                .get(PreimageKey::new_local(L2_ROLLUP_CONFIG_KEY.to()))
                .await
                .map_err(OracleProviderError::Preimage)?;
            serde_json::from_slice(&ser_cfg).map_err(OracleProviderError::Serde)?
        };

        Ok(Self {
            l1_head,
            agreed_l2_output_root: l2_output_root,
            claimed_l2_output_root: l2_claim,
            claimed_l2_block_number: l2_claim_block,
            chain_id,
            rollup_config,
        })
    }
}
