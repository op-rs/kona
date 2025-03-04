//! Contains the accelerated precompile for the BLS12-381 curve G1 MSM.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/g1_msm.rs

use crate::precompiles::utils::{msm_required_gas, precompile_run};
use alloc::{string::ToString, vec::Vec};
use alloy_primitives::{Address, Bytes, address, keccak256};
use revm::{
    precompile::{Error as PrecompileError, Precompile, PrecompileResult, PrecompileWithAddress},
    primitives::PrecompileOutput,
};

/// The maximum input size for the BLS12-381 g1 msm operation after the Isthmus Hardfork.
///
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
const BLS12_MAX_G1_MSM_SIZE_ISTHMUS: usize = 513760;

/// The address of the BLS12-381 g1 msm check precompile.
///
/// See: <https://eips.ethereum.org/EIPS/eip-2537#constants>
const BLS12_G1_MSM_CHECK: Address = address!("0x000000000000000000000000000000000000000c");

/// Input length of g1 msm operation.
const INPUT_LENGTH: usize = 160;

/// Base gas fee for the BLS12-381 g1 msm operation.
const G1_MSM_BASE_FEE: u64 = 12000;

/// The address of the BLS12-381 g1 msm precompile.
pub(crate) const FPVM_BLS12_G1_MSM_ISTHMUS: PrecompileWithAddress =
    PrecompileWithAddress(BLS12_G1_MSM_CHECK, Precompile::Standard(fpvm_bls12_g1_msm_isthmus));

/// Discounts table for G1 MSM as a vector of pairs `[k, discount]`.
static DISCOUNT_TABLE: [u16; 128] = [
    1000, 949, 848, 797, 764, 750, 738, 728, 719, 712, 705, 698, 692, 687, 682, 677, 673, 669, 665,
    661, 658, 654, 651, 648, 645, 642, 640, 637, 635, 632, 630, 627, 625, 623, 621, 619, 617, 615,
    613, 611, 609, 608, 606, 604, 603, 601, 599, 598, 596, 595, 593, 592, 591, 589, 588, 586, 585,
    584, 582, 581, 580, 579, 577, 576, 575, 574, 573, 572, 570, 569, 568, 567, 566, 565, 564, 563,
    562, 561, 560, 559, 558, 557, 556, 555, 554, 553, 552, 551, 550, 549, 548, 547, 547, 546, 545,
    544, 543, 542, 541, 540, 540, 539, 538, 537, 536, 536, 535, 534, 533, 532, 532, 531, 530, 529,
    528, 528, 527, 526, 525, 525, 524, 523, 522, 522, 521, 520, 520, 519,
];

/// Performs an FPVM-accelerated BLS12-381 G1 msm check.
fn fpvm_bls12_g1_msm(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let input_len = input.len();
    if input_len == 0 || input_len % INPUT_LENGTH != 0 {
        return Err(PrecompileError::Other(alloc::format!(
            "G1MSM input length should be multiple of {}, was {}",
            INPUT_LENGTH,
            input_len
        ))
        .into());
    }

    let k = input_len / INPUT_LENGTH;
    let required_gas = msm_required_gas(k, &DISCOUNT_TABLE, G1_MSM_BASE_FEE);
    if required_gas > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let result_data = kona_proof::block_on(precompile_run! {
        &[BLS12_G1_MSM_CHECK.as_ref(), input.as_ref()]
    })
    .map_err(|e| PrecompileError::Other(e.to_string()))?;

    Ok(PrecompileOutput::new(G1_MSM_BASE_FEE, result_data.into()))
}

/// Performs an FPVM-accelerated `bls12` g1 msm check precompile call
/// after the Isthmus Hardfork.
fn fpvm_bls12_g1_msm_isthmus(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if input.len() > BLS12_MAX_G1_MSM_SIZE_ISTHMUS {
        return Err(PrecompileError::Other(alloc::format!(
            "G1MSM input length must be at most {}",
            BLS12_MAX_G1_MSM_SIZE_ISTHMUS
        ))
        .into());
    }

    fpvm_bls12_g1_msm(input, gas_limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_fpvm_bls12_g1_msm_isthmus_max_bytes() {
        let input = Bytes::from(vec![0u8; BLS12_MAX_G1_MSM_SIZE_ISTHMUS + 1]);
        let gas_limit = G1_MSM_BASE_FEE;
        let err = PrecompileError::Other(alloc::format!(
            "G1MSM input length must be at most {}",
            BLS12_MAX_G1_MSM_SIZE_ISTHMUS
        ));
        assert_eq!(fpvm_bls12_g1_msm_isthmus(&input, gas_limit), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_g1_msm_offset() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH + 1]);
        let gas_limit = G1_MSM_BASE_FEE;
        let err = PrecompileError::Other(alloc::format!(
            "G1MSM input length should be multiple of {}, was {}",
            INPUT_LENGTH,
            input.len(),
        ));
        assert_eq!(fpvm_bls12_g1_msm(&input, gas_limit), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_g1_msm_out_of_gas() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH * 2]);
        let gas_limit = G1_MSM_BASE_FEE - 1;
        assert_eq!(fpvm_bls12_g1_msm(&input, gas_limit), Err(PrecompileError::OutOfGas.into()));
    }
}
