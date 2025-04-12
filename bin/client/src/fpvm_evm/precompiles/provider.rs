//! [`PrecompileProvider`] for FPVM-accelerated OP Stack precompiles.

use crate::fpvm_evm::precompiles::{
    ecrecover::ECRECOVER_ADDR, kzg_point_eval::KZG_POINT_EVAL_ADDR,
};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use alloy_primitives::{Address, Bytes};
use kona_preimage::{Channel, HintWriter, OracleReader};
use op_revm::{
    OpSpecId,
    precompiles::{fjord, granite, isthmus},
};
use revm::{
    context::{Cfg, ContextTr},
    handler::{EthPrecompiles, PrecompileProvider},
    interpreter::{Gas, InputsImpl, InstructionResult, InterpreterResult},
    precompile::{PrecompileError, PrecompileResult, Precompiles, bls12_381_const, bn128},
    primitives::{hardfork::SpecId, hash_map::HashMap},
};

/// The FPVM-accelerated precompiles.
#[derive(Debug)]
pub struct OpFpvmPrecompiles<C: Channel> {
    /// The default [`EthPrecompiles`] provider.
    inner: EthPrecompiles,
    /// The accelerated precompiles for the current [`OpSpecId`].
    accelerated_precompiles: HashMap<Address, AcceleratedPrecompileFn<C>>,
    /// The [`OpSpecId`] of the precompiles.
    spec: OpSpecId,
    /// The inner [`HintWriter`].
    hint_writer: HintWriter<C>,
    /// The inner [`OracleReader`].
    oracle_reader: OracleReader<C>,
}

impl<C: Channel + Send + Sync> OpFpvmPrecompiles<C> {
    /// Create a new precompile provider with the given [`OpSpecId`].
    #[inline]
    pub fn new_with_spec(
        spec: OpSpecId,
        hint_writer: HintWriter<C>,
        oracle_reader: OracleReader<C>,
    ) -> Self {
        let precompiles = match spec {
            spec @ (OpSpecId::BEDROCK |
            OpSpecId::REGOLITH |
            OpSpecId::CANYON |
            OpSpecId::ECOTONE) => Precompiles::new(spec.into_eth_spec().into()),
            OpSpecId::FJORD => fjord(),
            OpSpecId::GRANITE | OpSpecId::HOLOCENE => granite(),
            OpSpecId::ISTHMUS | OpSpecId::INTEROP | OpSpecId::OSAKA => isthmus(),
        };

        let accelerated_precompiles = match spec {
            OpSpecId::BEDROCK | OpSpecId::REGOLITH | OpSpecId::CANYON => accelerated_bedrock::<C>(),
            OpSpecId::ECOTONE | OpSpecId::FJORD => accelerated_ecotone::<C>(),
            OpSpecId::GRANITE | OpSpecId::HOLOCENE => accelerated_granite::<C>(),
            OpSpecId::ISTHMUS | OpSpecId::INTEROP | OpSpecId::OSAKA => accelerated_isthmus::<C>(),
        };

        Self {
            inner: EthPrecompiles { precompiles, spec: SpecId::default() },
            accelerated_precompiles: accelerated_precompiles
                .into_iter()
                .map(|p| (p.address, p.precompile))
                .collect(),
            spec,
            hint_writer,
            oracle_reader,
        }
    }
}

impl<CTX, C> PrecompileProvider<CTX> for OpFpvmPrecompiles<C>
where
    C: Channel + Clone + Send + Sync,
    CTX: ContextTr<Cfg: Cfg<Spec = OpSpecId>>,
{
    type Output = InterpreterResult;

    #[inline]
    fn set_spec(&mut self, spec: <CTX::Cfg as Cfg>::Spec) -> bool {
        if spec == self.spec {
            return false;
        }
        *self = Self::new_with_spec(spec, self.hint_writer.clone(), self.oracle_reader.clone());
        true
    }

    #[inline]
    fn run(
        &mut self,
        _context: &mut CTX,
        address: &Address,
        inputs: &InputsImpl,
        _is_static: bool,
        gas_limit: u64,
    ) -> Result<Option<Self::Output>, String> {
        let mut result = InterpreterResult {
            result: InstructionResult::Return,
            gas: Gas::new(gas_limit),
            output: Bytes::new(),
        };

        // Priority:
        // 1. If the precompile has an accelerated version, use that.
        // 2. If the precompile is not accelerated, use the default version.
        // 3. If the precompile is not found, return None.
        let output = if let Some(accelerated) = self.accelerated_precompiles.get(address) {
            (accelerated)(&inputs.input, gas_limit, &self.hint_writer, &self.oracle_reader)
        } else if let Some(precompile) = self.inner.precompiles.get(address) {
            (*precompile)(&inputs.input, gas_limit)
        } else {
            return Ok(None);
        };

        match output {
            Ok(output) => {
                let underflow = result.gas.record_cost(output.gas_used);
                assert!(underflow, "Gas underflow is not possible");
                result.result = InstructionResult::Return;
                result.output = output.bytes;
            }
            Err(PrecompileError::Fatal(e)) => return Err(e),
            Err(e) => {
                result.result = if e.is_oog() {
                    InstructionResult::PrecompileOOG
                } else {
                    InstructionResult::PrecompileError
                };
            }
        }

        Ok(Some(result))
    }

    #[inline]
    fn warm_addresses(&self) -> Box<impl Iterator<Item = Address>> {
        self.inner.warm_addresses()
    }

    #[inline]
    fn contains(&self, address: &Address) -> bool {
        self.inner.contains(address)
    }
}

/// A precompile function that can be accelerated by the FPVM.
type AcceleratedPrecompileFn<C> =
    fn(&[u8], u64, &HintWriter<C>, &OracleReader<C>) -> PrecompileResult;

/// A tuple type for accelerated precompiles with an associated [`Address`].
struct AcceleratedPrecompile<C> {
    /// The address of the precompile.
    address: Address,
    /// The precompile function.
    precompile: AcceleratedPrecompileFn<C>,
}

impl<C> AcceleratedPrecompile<C> {
    /// Create a new accelerated precompile.
    fn new(address: Address, precompile: AcceleratedPrecompileFn<C>) -> Self {
        Self { address, precompile }
    }
}

/// The accelerated precompiles for the bedrock spec.
fn accelerated_bedrock<C: Channel + Send + Sync>() -> Vec<AcceleratedPrecompile<C>> {
    vec![
        AcceleratedPrecompile::new(ECRECOVER_ADDR, super::ecrecover::fpvm_ec_recover::<C>),
        AcceleratedPrecompile::new(bn128::pair::ADDRESS, super::bn128_pair::fpvm_bn128_pair::<C>),
    ]
}

/// The accelerated precompiles for the ecotone spec.
fn accelerated_ecotone<C: Channel + Send + Sync>() -> Vec<AcceleratedPrecompile<C>> {
    let mut base = accelerated_bedrock::<C>();
    base.push(AcceleratedPrecompile::new(
        KZG_POINT_EVAL_ADDR,
        super::kzg_point_eval::fpvm_kzg_point_eval::<C>,
    ));
    base
}

/// The accelerated precompiles for the granite spec.
fn accelerated_granite<C: Channel + Send + Sync>() -> Vec<AcceleratedPrecompile<C>> {
    let mut base = accelerated_ecotone::<C>();
    base.push(AcceleratedPrecompile::new(
        bn128::pair::ADDRESS,
        super::bn128_pair::fpvm_bn128_pair_granite::<C>,
    ));
    base
}

/// The accelerated precompiles for the isthmus spec.
fn accelerated_isthmus<C: Channel + Send + Sync>() -> Vec<AcceleratedPrecompile<C>> {
    let mut base = accelerated_granite::<C>();
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::G1_ADD_ADDRESS,
        super::bls12_g1_add::fpvm_bls12_g1_add::<C>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::G1_MSM_ADDRESS,
        super::bls12_g1_msm::fpvm_bls12_g1_msm::<C>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::G2_ADD_ADDRESS,
        super::bls12_g2_add::fpvm_bls12_g2_add::<C>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::G2_MSM_ADDRESS,
        super::bls12_g2_msm::fpvm_bls12_g2_msm::<C>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::MAP_FP_TO_G1_ADDRESS,
        super::bls12_map_fp::fpvm_bls12_map_fp::<C>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::MAP_FP2_TO_G2_ADDRESS,
        super::bls12_map_fp2::fpvm_bls12_map_fp2::<C>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::PAIRING_ADDRESS,
        super::bls12_pair::fpvm_bls12_pairing::<C>,
    ));
    base
}
