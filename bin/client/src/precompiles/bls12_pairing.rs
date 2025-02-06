//! Contains the accelerated precompile for the BLS12-381 curve.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/pairing.rs

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

/// The max pairing size for BLS12-381 input given a 20M gas limit.
const BLS12_MAX_PAIRING_SIZE_ISTHMUS: usize = 235_008;

/// The address of the BLS12-381 pairing check precompile.
const BLS12_PAIRING_CHECK: Address = address!("0x000000000000000000000000000000000000000f");

/// Input length of pairing operation.
const INPUT_LENGTH: usize = 384;

/// Multiplier gas fee for BLS12-381 pairing operation.
const PAIRING_MULTIPLIER_BASE: u64 = 32600;

/// Offset gas fee for BLS12-381 pairing operation.
const PAIRING_OFFSET_BASE: u64 = 37700;

/// The address of the BLS12-381 pairing precompile.
pub(crate) const FPVM_BLS12_PAIRING_ISTHMUS: PrecompileWithAddress =
    PrecompileWithAddress(BLS12_PAIRING_CHECK, Precompile::Standard(fpvm_bls12_pairing_isthmus));

/// Performs an FPVM-accelerated BLS12-381 pairing check.
fn fpvm_bls12_pairing(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let input_len = input.len();
    if input_len % INPUT_LENGTH != 0 {
        return Err(PrecompileError::Other(alloc::format!(
            "Pairing input length should be multiple of {INPUT_LENGTH}, was {input_len}"
        ))
        .into());
    }

    let k = input_len / INPUT_LENGTH;
    let required_gas: u64 = PAIRING_MULTIPLIER_BASE * k as u64 + PAIRING_OFFSET_BASE;
    if required_gas > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let result_data = kona_proof::block_on(async move {
        // Write the hint for the ecrecover precompile run.
        let hint_data = &[BLS12_PAIRING_CHECK.as_ref(), input.as_ref()];
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

    Ok(PrecompileOutput::new(required_gas, result_data.into()))
}

/// Performs an FPVM-accelerated `bls12` pairing check precompile call
/// after the Isthmus Hardfork.
fn fpvm_bls12_pairing_isthmus(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if input.len() > BLS12_MAX_PAIRING_SIZE_ISTHMUS {
        return Err(PrecompileError::Other(alloc::format!(
            "Pairing input length must be at most {}",
            BLS12_MAX_PAIRING_SIZE_ISTHMUS
        ))
        .into());
    }

    fpvm_bls12_pairing(input, gas_limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_fpvm_bls12_pairing_isthmus_max_bytes() {
        let input = Bytes::from(vec![0u8; BLS12_MAX_PAIRING_SIZE_ISTHMUS + 1]);
        let gas_limit = PAIRING_MULTIPLIER_BASE;
        let err = PrecompileError::Other("Pairing input length must be at most 235008".to_string());
        assert_eq!(fpvm_bls12_pairing_isthmus(&input, gas_limit), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_offset() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH + 1]);
        let gas_limit = PAIRING_OFFSET_BASE;
        let err = PrecompileError::Other(
            "Pairing input length should be multiple of 384, was 385".to_string(),
        );
        assert_eq!(fpvm_bls12_pairing(&input, gas_limit), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_out_of_gas() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH * 2]);
        let gas_limit = PAIRING_MULTIPLIER_BASE - 1;
        assert_eq!(fpvm_bls12_pairing(&input, gas_limit), Err(PrecompileError::OutOfGas.into()));
    }
}
