//! Contains the accelerated precompile for the BLS12-381 curve G2 MSM.
//!
//! BLS12-381 is introduced in [EIP-2537](https://eips.ethereum.org/EIPS/eip-2537).
//!
//! For constants and logic, see the [revm implementation].
//!
//! [revm implementation]: https://github.com/bluealloy/revm/blob/main/crates/precompile/src/bls12_381/g2_msm.rs

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

/// Amount used to calculate the multi-scalar-multiplication discount
const MSM_MULTIPLIER: u64 = 1000;

/// Implements the gas schedule for G1/G2 Multiscalar-multiplication assuming 30
/// MGas/second, see also: <https://eips.ethereum.org/EIPS/eip-2537#g1g2-multiexponentiation>
#[inline]
fn msm_required_gas(k: usize, discount_table: &[u16], multiplication_cost: u64) -> u64 {
    if k == 0 {
        return 0;
    }

    let index = core::cmp::min(k - 1, discount_table.len() - 1);
    let discount = discount_table[index] as u64;

    (k as u64 * discount * multiplication_cost) / MSM_MULTIPLIER
}

/// The maximum input size for the BLS12-381 g2 msm operation after the Isthmus Hardfork.
///
/// See: <https://specs.optimism.io/protocol/isthmus/exec-engine.html#evm-changes>
const BLS12_MAX_G2_MSM_SIZE_ISTHMUS: usize = 488448;

/// The address of the BLS12-381 g2 msm check precompile.
///
/// See: <https://eips.ethereum.org/EIPS/eip-2537#constants>
const BLS12_G2_MSM_CHECK: Address = address!("0x000000000000000000000000000000000000000e");

/// Input length of g2 msm operation.
const INPUT_LENGTH: usize = 288;

/// Base gas fee for the BLS12-381 g2 msm operation.
const G2_MSM_BASE_FEE: u64 = 22500;

/// The address of the BLS12-381 g2 msm precompile.
pub(crate) const FPVM_BLS12_G2_MSM_ISTHMUS: PrecompileWithAddress =
    PrecompileWithAddress(BLS12_G2_MSM_CHECK, Precompile::Standard(fpvm_bls12_g2_msm_isthmus));

// Discounts table for G2 MSM as a vector of pairs `[k, discount]`:
static DISCOUNT_TABLE: [u16; 128] = [
    1000, 1000, 923, 884, 855, 832, 812, 796, 782, 770, 759, 749, 740, 732, 724, 717, 711, 704,
    699, 693, 688, 683, 679, 674, 670, 666, 663, 659, 655, 652, 649, 646, 643, 640, 637, 634, 632,
    629, 627, 624, 622, 620, 618, 615, 613, 611, 609, 607, 606, 604, 602, 600, 598, 597, 595, 593,
    592, 590, 589, 587, 586, 584, 583, 582, 580, 579, 578, 576, 575, 574, 573, 571, 570, 569, 568,
    567, 566, 565, 563, 562, 561, 560, 559, 558, 557, 556, 555, 554, 553, 552, 552, 551, 550, 549,
    548, 547, 546, 545, 545, 544, 543, 542, 541, 541, 540, 539, 538, 537, 537, 536, 535, 535, 534,
    533, 532, 532, 531, 530, 530, 529, 528, 528, 527, 526, 526, 525, 524, 524,
];

/// Performs an FPVM-accelerated BLS12-381 G2 msm check.
fn fpvm_bls12_g2_msm(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    let input_len = input.len();
    if input_len == 0 || input_len % INPUT_LENGTH != 0 {
        return Err(PrecompileError::Other(alloc::format!(
            "G2MSM input length should be multiple of {}, was {}",
            INPUT_LENGTH,
            input_len
        ))
        .into());
    }

    let k = input_len / INPUT_LENGTH;
    let required_gas = msm_required_gas(k, &DISCOUNT_TABLE, G2_MSM_BASE_FEE);
    if required_gas > gas_limit {
        return Err(PrecompileError::OutOfGas.into());
    }

    let result_data = kona_proof::block_on(async move {
        // Write the hint for the ecrecover precompile run.
        let hint_data = &[BLS12_G2_MSM_CHECK.as_ref(), input.as_ref()];
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

    Ok(PrecompileOutput::new(G2_MSM_BASE_FEE, result_data.into()))
}

/// Performs an FPVM-accelerated `bls12` g2 msm check precompile call
/// after the Isthmus Hardfork.
fn fpvm_bls12_g2_msm_isthmus(input: &Bytes, gas_limit: u64) -> PrecompileResult {
    if input.len() > BLS12_MAX_G2_MSM_SIZE_ISTHMUS {
        return Err(PrecompileError::Other(alloc::format!(
            "G2MSM input length must be at most {}",
            BLS12_MAX_G2_MSM_SIZE_ISTHMUS
        ))
        .into());
    }

    fpvm_bls12_g2_msm(input, gas_limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_fpvm_bls12_g2_msm_isthmus_max_bytes() {
        let input = Bytes::from(vec![0u8; BLS12_MAX_G2_MSM_SIZE_ISTHMUS + 1]);
        let gas_limit = G2_MSM_BASE_FEE;
        let err = PrecompileError::Other(alloc::format!(
            "G2MSM input length must be at most {}",
            BLS12_MAX_G2_MSM_SIZE_ISTHMUS
        ));
        assert_eq!(fpvm_bls12_g2_msm_isthmus(&input, gas_limit), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_g2_msm_offset() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH + 1]);
        let gas_limit = G2_MSM_BASE_FEE;
        let err = PrecompileError::Other(alloc::format!(
            "G2MSM input length should be multiple of {}, was {}",
            INPUT_LENGTH,
            input.len(),
        ));
        assert_eq!(fpvm_bls12_g2_msm(&input, gas_limit), Err(err.into()));
    }

    #[test]
    fn test_fpvm_bls12_g2_msm_out_of_gas() {
        let input = Bytes::from(vec![0u8; INPUT_LENGTH * 2]);
        let gas_limit = G2_MSM_BASE_FEE - 1;
        assert_eq!(fpvm_bls12_g2_msm(&input, gas_limit), Err(PrecompileError::OutOfGas.into()));
    }
}
