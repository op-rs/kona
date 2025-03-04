//! Contains the accelerated precompile for the BLS12-381 curve G1 Point Addition.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/g1_add.rs

use crate::precompiles::utils::precompile_run;
use alloc::{string::ToString, vec::Vec};
use alloy_primitives::{Address, Bytes, address, keccak256};
use revm::{
    precompile::{Error as PrecompileError, Precompile, PrecompileResult, PrecompileWithAddress},
    primitives::PrecompileOutput,
};

/// The address of the BLS12-381 g1 addition check precompile.
///
/// See: <https://eips.ethereum.org/EIPS/eip-2537#constants>
const BLS12_G1_ADD_CHECK: Address = address!("0x000000000000000000000000000000000000000b");

/// Input length of G1 Addition operation.
const INPUT_LENGTH: usize = 256;

/// Base gas fee for the BLS12-381 g1 addition operation.
const G1_ADD_BASE_FEE: u64 = 375;

/// The address of the BLS12-381 g1 addition precompile.
pub(crate) const FPVM_BLS12_G1_ADD_ISTHMUS: PrecompileWithAddress =
    PrecompileWithAddress(BLS12_G1_ADD_CHECK, Precompile::Standard(fpvm_bls12_g1_add));

/// Performs an FPVM-accelerated BLS12-381 G1 addition check.
///
/// Notice, there is no input size limit for this precompile.
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
fn fpvm_bls12_g1_add(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if G1_ADD_BASE_FEE > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let input_len = input.len();
    if input_len != INPUT_LENGTH {
        return Err(PrecompileError::Other(alloc::format!(
            "G1 addition input length should be multiple of {INPUT_LENGTH}, was {input_len}"
        ))
        .into());
    }

    let result_data = kona_proof::block_on(precompile_run! {
        &[BLS12_G1_ADD_CHECK.as_ref(), input.as_ref()]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(G1_ADD_BASE_FEE, result_data.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_fpvm_bls12_g1_add_input_len() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH + 1]);
        let err = PrecompileError::Other(alloc::format!(
            "G1 addition input length should be multiple of {}, was {}",
            INPUT_LENGTH,
            INPUT_LENGTH + 1
        ));
        assert_eq!(fpvm_bls12_g1_add(&input, G1_ADD_BASE_FEE), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_g1_add_out_of_gas() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH * 2]);
        assert_eq!(
            fpvm_bls12_g1_add(&input, G1_ADD_BASE_FEE - 1),
            Err(PrecompileError::OutOfGas.into())
        );
    }
}
