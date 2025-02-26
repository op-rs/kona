//! Contains the accelerated precompile for the BLS12-381 curve FP2 to G2 Mapping.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/map_fp_to_g1.rs

use crate::precompiles::utils::precompile_run;
use alloc::{string::ToString, vec::Vec};
use alloy_primitives::{keccak256, Address, Bytes};
use revm::{
    precompile::{
        bls12_381::map_fp2_to_g2::PRECOMPILE, Error as PrecompileError, Precompile,
        PrecompileResult, PrecompileWithAddress,
    },
    primitives::PrecompileOutput,
};

/// The address of the BLS12-381 map fp2 to g2 check precompile.
///
/// See: <https://eips.ethereum.org/EIPS/eip-2537#constants>
const BLS12_MAP_FP2_CHECK: Address = PRECOMPILE.0;

/// Base gas fee for the BLS12-381 map fp2 to g2 operation.
const MAP_FP2_BASE_FEE: u64 = 23800;

/// The padded FP2 length.
const PADDED_FP2_LENGTH: usize = 128;

/// The address of the BLS12-381 map fp2 to g2 precompile.
pub(crate) const FPVM_BLS12_MAP_FP2_ISTHMUS: PrecompileWithAddress =
    PrecompileWithAddress(BLS12_MAP_FP2_CHECK, Precompile::Standard(fpvm_bls12_map_fp2));

/// Performs an FPVM-accelerated BLS12-381 map fp2 check.
///
/// Notice, there is no input size limit for this precompile.
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
fn fpvm_bls12_map_fp2(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if MAP_FP2_BASE_FEE > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    if input.len() != PADDED_FP2_LENGTH {
        return Err(PrecompileError::Other(alloc::format!(
            "MAP_FP2_TO_G2 input should be {PADDED_FP2_LENGTH} bytes, was {}",
            input.len()
        ))
        .into());
    }

    let result_data = kona_proof::block_on(precompile_run! {
        &[BLS12_MAP_FP2_CHECK.as_ref(), input.as_ref()]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(MAP_FP2_BASE_FEE, result_data.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_fpvm_bls12_map_fp2_offset() {
        let input = Bytes::from(vec![0u8; PADDED_FP2_LENGTH + 1]);
        let gas_limit = MAP_FP2_BASE_FEE;
        let err = PrecompileError::Other(alloc::format!(
            "MAP_FP2_TO_G2 input should be {} bytes, was {}",
            PADDED_FP2_LENGTH,
            input.len(),
        ));
        assert_eq!(fpvm_bls12_map_fp2(&input, gas_limit), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_map_fp_out_of_gas() {
        let input = Bytes::from(vec![0u8; PADDED_FP2_LENGTH]);
        let gas_limit = MAP_FP2_BASE_FEE - 1;
        assert_eq!(fpvm_bls12_map_fp2(&input, gas_limit), Err(PrecompileError::OutOfGas.into()));
    }
}
