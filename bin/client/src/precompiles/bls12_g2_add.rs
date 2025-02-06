//! Contains the accelerated precompile for the BLS12-381 curve G2 Point Addition.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/g2_add.rs

use crate::precompiles::utils::precompile_run;
use alloc::{string::ToString, vec::Vec};
use alloy_primitives::{address, keccak256, Address, Bytes};
use revm::{
    precompile::{Error as PrecompileError, Precompile, PrecompileResult, PrecompileWithAddress},
    primitives::PrecompileOutput,
};

/// The address of the BLS12-381 g2 addition check precompile.
///
/// See: <https://eips.ethereum.org/EIPS/eip-2537#constants>
const BLS12_G2_ADD_CHECK: Address = address!("0x000000000000000000000000000000000000000d");

/// Input length of g2 addition operation.
const INPUT_LENGTH: usize = 512;

/// Base gas fee for the BLS12-381 g2 addition operation.
const G2_ADD_BASE_FEE: u64 = 600;

/// The address of the BLS12-381 g2 addition precompile.
pub(crate) const FPVM_BLS12_G2_ADD_ISTHMUS: PrecompileWithAddress =
    PrecompileWithAddress(BLS12_G2_ADD_CHECK, Precompile::Standard(fpvm_bls12_g2_add));

/// Performs an FPVM-accelerated BLS12-381 G2 addition check.
///
/// Notice, there is no input size limit for this precompile.
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
fn fpvm_bls12_g2_add(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if G2_ADD_BASE_FEE > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let input_len = input.len();
    if input_len != INPUT_LENGTH {
        return Err(PrecompileError::Other(alloc::format!(
            "G2 addition input length should be multiple of {INPUT_LENGTH}, was {input_len}"
        ))
        .into());
    }

    let result_data = kona_proof::block_on(precompile_run! {
        &[BLS12_G2_ADD_CHECK.as_ref(), input.as_ref()]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(G2_ADD_BASE_FEE, result_data.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_fpvm_bls12_g2_add_input_len() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH + 1]);
        let err = PrecompileError::Other(alloc::format!(
            "G2 addition input length should be multiple of {}, was {}",
            INPUT_LENGTH,
            INPUT_LENGTH + 1
        ));
        assert_eq!(fpvm_bls12_g2_add(&input, G2_ADD_BASE_FEE), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_g2_add_out_of_gas() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH * 2]);
        assert_eq!(
            fpvm_bls12_g2_add(&input, G2_ADD_BASE_FEE - 1),
            Err(PrecompileError::OutOfGas.into())
        );
    }
}
