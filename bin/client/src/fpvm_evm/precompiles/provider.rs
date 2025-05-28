//! [`PrecompileProvider`] for FPVM-accelerated OP Stack precompiles.

use crate::fpvm_evm::precompiles::{
    ecrecover::ECRECOVER_ADDR, kzg_point_eval::KZG_POINT_EVAL_ADDR,
};
use alloc::{boxed::Box, string::String, vec, vec::Vec};
use alloy_primitives::{Address, Bytes};
use kona_preimage::{HintWriterClient, PreimageOracleClient};
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
pub struct OpFpvmPrecompiles<H, O> {
    /// The default [`EthPrecompiles`] provider.
    inner: EthPrecompiles,
    /// The accelerated precompiles for the current [`OpSpecId`].
    accelerated_precompiles: HashMap<Address, AcceleratedPrecompileFn<H, O>>,
    /// The [`OpSpecId`] of the precompiles.
    spec: OpSpecId,
    /// The inner [`HintWriterClient`].
    hint_writer: H,
    /// The inner [`PreimageOracleClient`].
    oracle_reader: O,
}

impl<H, O> OpFpvmPrecompiles<H, O>
where
    H: HintWriterClient + Clone + Send + Sync + 'static,
    O: PreimageOracleClient + Clone + Send + Sync + 'static,
{
    /// Create a new precompile provider with the given [`OpSpecId`].
    #[inline]
    pub fn new_with_spec(spec: OpSpecId, hint_writer: H, oracle_reader: O) -> Self {
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
            OpSpecId::BEDROCK | OpSpecId::REGOLITH | OpSpecId::CANYON => {
                accelerated_bedrock::<H, O>()
            }
            OpSpecId::ECOTONE | OpSpecId::FJORD => accelerated_ecotone::<H, O>(),
            OpSpecId::GRANITE | OpSpecId::HOLOCENE => accelerated_granite::<H, O>(),
            OpSpecId::ISTHMUS | OpSpecId::INTEROP | OpSpecId::OSAKA => {
                accelerated_isthmus::<H, O>()
            }
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

impl<CTX, H, O> PrecompileProvider<CTX> for OpFpvmPrecompiles<H, O>
where
    H: HintWriterClient + Clone + Send + Sync + 'static,
    O: PreimageOracleClient + Clone + Send + Sync + 'static,
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
        context: &mut CTX,
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

        use revm::context::LocalContextTr;
        let input = match &inputs.input {
            revm::interpreter::CallInput::Bytes(bytes) => bytes.clone(),
            revm::interpreter::CallInput::SharedBuffer(range) => context
                .local()
                .shared_memory_buffer_slice(range.clone())
                .map(|b| Bytes::from(b.to_vec()))
                .unwrap_or_default(),
        };

        // Priority:
        // 1. If the precompile has an accelerated version, use that.
        // 2. If the precompile is not accelerated, use the default version.
        // 3. If the precompile is not found, return None.
        let output = if let Some(accelerated) = self.accelerated_precompiles.get(address) {
            (accelerated)(&input, gas_limit, &self.hint_writer, &self.oracle_reader)
        } else if let Some(precompile) = self.inner.precompiles.get(address) {
            (*precompile)(&input, gas_limit)
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
type AcceleratedPrecompileFn<H, O> = fn(&[u8], u64, &H, &O) -> PrecompileResult;

/// A tuple type for accelerated precompiles with an associated [`Address`].
struct AcceleratedPrecompile<H, O> {
    /// The address of the precompile.
    address: Address,
    /// The precompile function.
    precompile: AcceleratedPrecompileFn<H, O>,
}

impl<H, O> AcceleratedPrecompile<H, O> {
    /// Create a new accelerated precompile.
    fn new(address: Address, precompile: AcceleratedPrecompileFn<H, O>) -> Self {
        Self { address, precompile }
    }
}

/// The accelerated precompiles for the bedrock spec.
fn accelerated_bedrock<H, O>() -> Vec<AcceleratedPrecompile<H, O>>
where
    H: HintWriterClient + Send + Sync,
    O: PreimageOracleClient + Send + Sync,
{
    vec![
        AcceleratedPrecompile::new(ECRECOVER_ADDR, super::ecrecover::fpvm_ec_recover::<H, O>),
        AcceleratedPrecompile::new(
            bn128::pair::ADDRESS,
            super::bn128_pair::fpvm_bn128_pair::<H, O>,
        ),
    ]
}

/// The accelerated precompiles for the ecotone spec.
fn accelerated_ecotone<H, O>() -> Vec<AcceleratedPrecompile<H, O>>
where
    H: HintWriterClient + Send + Sync,
    O: PreimageOracleClient + Send + Sync,
{
    let mut base = accelerated_bedrock::<H, O>();
    base.push(AcceleratedPrecompile::new(
        KZG_POINT_EVAL_ADDR,
        super::kzg_point_eval::fpvm_kzg_point_eval::<H, O>,
    ));
    base
}

/// The accelerated precompiles for the granite spec.
fn accelerated_granite<H, O>() -> Vec<AcceleratedPrecompile<H, O>>
where
    H: HintWriterClient + Send + Sync,
    O: PreimageOracleClient + Send + Sync,
{
    let mut base = accelerated_ecotone::<H, O>();
    base.push(AcceleratedPrecompile::new(
        bn128::pair::ADDRESS,
        super::bn128_pair::fpvm_bn128_pair_granite::<H, O>,
    ));
    base
}

/// The accelerated precompiles for the isthmus spec.
fn accelerated_isthmus<H, O>() -> Vec<AcceleratedPrecompile<H, O>>
where
    H: HintWriterClient + Send + Sync,
    O: PreimageOracleClient + Send + Sync,
{
    let mut base = accelerated_granite::<H, O>();
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::G1_ADD_ADDRESS,
        super::bls12_g1_add::fpvm_bls12_g1_add::<H, O>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::G1_MSM_ADDRESS,
        super::bls12_g1_msm::fpvm_bls12_g1_msm::<H, O>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::G2_ADD_ADDRESS,
        super::bls12_g2_add::fpvm_bls12_g2_add::<H, O>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::G2_MSM_ADDRESS,
        super::bls12_g2_msm::fpvm_bls12_g2_msm::<H, O>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::MAP_FP_TO_G1_ADDRESS,
        super::bls12_map_fp::fpvm_bls12_map_fp::<H, O>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::MAP_FP2_TO_G2_ADDRESS,
        super::bls12_map_fp2::fpvm_bls12_map_fp2::<H, O>,
    ));
    base.push(AcceleratedPrecompile::new(
        bls12_381_const::PAIRING_ADDRESS,
        super::bls12_pair::fpvm_bls12_pairing::<H, O>,
    ));
    base
}
