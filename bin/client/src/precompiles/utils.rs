//! Utility functions for precompiles

// TODO: replace this with revm::precompiles::bls12_381::msm::msm_required_gas
//       once the `msm` module is public. As of v19.4.0 the `msm` module is private.
/// Implements the gas schedule for G1/G2 Multiscalar-multiplication assuming 30
/// MGas/second, see also: <https://eips.ethereum.org/EIPS/eip-2537#g1g2-multiexponentiation>
#[inline]
pub(crate) fn msm_required_gas(k: usize, discount_table: &[u16], multiplication_cost: u64) -> u64 {
    /// Amount used to calculate the multi-scalar-multiplication discount
    const MSM_MULTIPLIER: u64 = 1000;

    if k == 0 {
        return 0;
    }

    let index = core::cmp::min(k - 1, discount_table.len() - 1);
    let discount = discount_table[index] as u64;

    (k as u64 * discount * multiplication_cost) / MSM_MULTIPLIER
}
