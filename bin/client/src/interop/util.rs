//! Utilities for the interop proof program

use alloc::string::ToString;
use alloy_primitives::B256;
use kona_preimage::{errors::PreimageOracleError, CommsClient, PreimageKey, PreimageKeyType};
use kona_proof::errors::OracleProviderError;
use kona_proof_interop::{HintType, PreState};

/// Fetches the safe head hash of the L2 chain, using the active L2 chain in the [PreState].
pub(crate) async fn fetch_l2_safe_head_hash<O>(
    caching_oracle: &O,
    pre: &PreState,
) -> Result<B256, OracleProviderError>
where
    O: CommsClient,
{
    // Fetch the output root of the safe head block for the current L2 chain.
    let rich_output = match pre {
        PreState::SuperRoot(super_root) => {
            super_root.output_roots.first().ok_or(OracleProviderError::Preimage(
                PreimageOracleError::Other("No output roots in super root".to_string()),
            ))?
        }
        PreState::TransitionState(transition_state) => {
            transition_state.pre_state.output_roots.get(transition_state.step as usize).ok_or(
                OracleProviderError::Preimage(PreimageOracleError::Other(
                    "No output roots in transition state's pending progress".to_string(),
                )),
            )?
        }
    };

    HintType::L2OutputRoot
        .with_data(&[
            rich_output.output_root.as_slice(),
            rich_output.chain_id.to_be_bytes().as_slice(),
        ])
        .send(caching_oracle)
        .await?;
    let output_preimage = caching_oracle
        .get(PreimageKey::new(*output_root, PreimageKeyType::Keccak256))
        .await
        .map_err(OracleProviderError::Preimage)?;

    output_preimage[96..128].try_into().map_err(OracleProviderError::SliceConversion)
}
