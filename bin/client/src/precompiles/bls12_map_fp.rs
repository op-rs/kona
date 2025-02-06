//! Contains the accelerated precompile for the BLS12-381 curve FP to G1 Mapping.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/map_fp_to_g1.rs

use crate::{HINT_WRITER, ORACLE_READER};
use alloc::{string::ToString, vec::Vec};
use alloy_primitives::{address, keccak256, Address, Bytes};
use kona_preimage::{
    errors::PreimageOracleError, PreimageKey, PreimageKeyType, PreimageOracleClient,
};
use kona_proof::{errors::OracleProviderError, HintType};
use revm::{
    precompile::{Error as PrecompileError, Precompile, PrecompileResult, PrecompileWithAddress},
    primitives::PrecompileOutput,
};

/// The address of the BLS12-381 map fp to g1 check precompile.
///
/// See: <https://eips.ethereum.org/EIPS/eip-2537#constants>
const BLS12_MAP_FP_CHECK: Address = address!("0x0000000000000000000000000000000000000010");

/// Base gas fee for the BLS12-381 map fp to g1 operation.
const MAP_FP_BASE_FEE: u64 = 5500;

/// The padded FP length.
const PADDED_FP_LENGTH: usize = 64;

/// The address of the BLS12-381 map fp to g1 precompile.
pub(crate) const FPVM_BLS12_MAP_FP_ISTHMUS: PrecompileWithAddress =
    PrecompileWithAddress(BLS12_MAP_FP_CHECK, Precompile::Standard(fpvm_bls12_map_fp));

/// Performs an FPVM-accelerated BLS12-381 map fp check.
///
/// Notice, there is no input size limit for this precompile.
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
fn fpvm_bls12_map_fp(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if MAP_FP_BASE_FEE > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    if input.len() != PADDED_FP_LENGTH {
        return Err(PrecompileError::Other(alloc::format!(
            "MAP_FP_TO_G1 input should be {PADDED_FP_LENGTH} bytes, was {}",
            input.len()
        ))
        .into());
    }

    let result_data = kona_proof::block_on(async move {
        // Write the hint for the ecrecover precompile run.
        let hint_data = &[BLS12_MAP_FP_CHECK.as_ref(), input.as_ref()];
        HintType::L1Precompile.with_data(hint_data).send(&HINT_WRITER).await?;

        // Construct the key hash for the ecrecover precompile run.
        let raw_key_data = hint_data.iter().copied().flatten().copied().collect::<Vec<u8>>();
        let key_hash = keccak256(&raw_key_data);

        // Fetch the result of the ecrecover precompile run from the host.
        let result_data = ORACLE_READER
            .get(PreimageKey::new(*key_hash, PreimageKeyType::Precompile))
            .await
            .map_err(OracleProviderError::Preimage)?;

        // Ensure we've received valid result data.
        if result_data.is_empty() {
            return Err(OracleProviderError::Preimage(PreimageOracleError::Other(
                "Invalid result data".to_string(),
            )));
        }

        // Ensure we've not received an error from the host.
        if result_data[0] == 0 {
            return Err(OracleProviderError::Preimage(PreimageOracleError::Other(
                "Error executing ecrecover precompile in host".to_string(),
            )));
        }

        // Return the result data.
        Ok(result_data[1..].to_vec())
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(MAP_FP_BASE_FEE, result_data.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_fpvm_bls12_map_fp_offset() {
        let input = Bytes::from(vec![0u8; PADDED_FP_LENGTH + 1]);
        let gas_limit = MAP_FP_BASE_FEE;
        let err = PrecompileError::Other(alloc::format!(
            "MAP_FP_TO_G1 input should be {} bytes, was {}",
            PADDED_FP_LENGTH,
            input.len(),
        ));
        assert_eq!(fpvm_bls12_map_fp(&input, gas_limit), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_map_fp_out_of_gas() {
        let input = Bytes::from(vec![0u8; PADDED_FP_LENGTH]);
        let gas_limit = MAP_FP_BASE_FEE - 1;
        assert_eq!(fpvm_bls12_map_fp(&input, gas_limit), Err(PrecompileError::OutOfGas.into()));
    }
}
