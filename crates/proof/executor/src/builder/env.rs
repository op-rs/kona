//! Environment utility functions for [StatelessL2Builder].

use super::StatelessL2Builder;
use crate::{ExecutorError, ExecutorResult, TrieDBProvider, util::decode_holocene_eip_1559_params};
use alloy_consensus::{BlockHeader, Header, transaction::Recovered};
use alloy_eips::{Decodable2718, eip1559::BaseFeeParams, eip7840::BlobParams};
use alloy_evm::{EvmEnv, EvmFactory};
use alloy_primitives::Bytes;
use kona_genesis::RollupConfig;
use kona_mpt::TrieHinter;
use op_alloy_consensus::OpTxEnvelope;
use op_alloy_rpc_types_engine::OpPayloadAttributes;
use op_revm::OpSpecId;
use revm::{
    context::{BlockEnv, CfgEnv},
    context_interface::block::BlobExcessGasAndPrice,
};

impl<P, H, Evm> StatelessL2Builder<'_, P, H, Evm>
where
    P: TrieDBProvider,
    H: TrieHinter,
    Evm: EvmFactory,
{
    /// Returns the active [`EvmEnv`] for the executor.
    pub(crate) fn evm_env(
        &self,
        spec_id: OpSpecId,
        parent_header: &Header,
        payload_attrs: &OpPayloadAttributes,
        base_fee_params: &BaseFeeParams,
    ) -> ExecutorResult<EvmEnv<OpSpecId>> {
        let block_env =
            Self::prepare_block_env(spec_id, parent_header, payload_attrs, base_fee_params)?;
        let cfg_env = self.evm_cfg_env(payload_attrs.payload_attributes.timestamp);
        Ok(EvmEnv::new(cfg_env, block_env))
    }

    /// Returns the active [CfgEnv] for the executor.
    pub(crate) fn evm_cfg_env(&self, timestamp: u64) -> CfgEnv<OpSpecId> {
        CfgEnv::new()
            .with_chain_id(self.config.l2_chain_id)
            .with_spec(self.config.spec_id(timestamp))
    }

    /// Prepares a [BlockEnv] with the given [OpPayloadAttributes].
    pub(crate) fn prepare_block_env(
        spec_id: OpSpecId,
        parent_header: &Header,
        payload_attrs: &OpPayloadAttributes,
        base_fee_params: &BaseFeeParams,
    ) -> ExecutorResult<BlockEnv> {
        let blob_excess_gas_and_price = parent_header
            .maybe_next_block_excess_blob_gas(if spec_id.is_enabled_in(OpSpecId::ISTHMUS) {
                Some(BlobParams::prague())
            } else if spec_id.is_enabled_in(OpSpecId::ECOTONE) {
                Some(BlobParams::cancun())
            } else {
                None
            })
            .or_else(|| spec_id.is_enabled_in(OpSpecId::ECOTONE).then_some(0))
            .map(|e| BlobExcessGasAndPrice::new(e, spec_id.is_enabled_in(OpSpecId::ISTHMUS)));
        let next_block_base_fee =
            parent_header.next_block_base_fee(*base_fee_params).unwrap_or_default();

        Ok(BlockEnv {
            number: parent_header.number + 1,
            beneficiary: payload_attrs.payload_attributes.suggested_fee_recipient,
            timestamp: payload_attrs.payload_attributes.timestamp,
            gas_limit: payload_attrs.gas_limit.ok_or(ExecutorError::MissingGasLimit)?,
            basefee: next_block_base_fee,
            prevrandao: Some(payload_attrs.payload_attributes.prev_randao),
            blob_excess_gas_and_price,
            ..Default::default()
        })
    }

    /// Returns the active base fee parameters for the given payload attributes.
    pub(crate) fn active_base_fee_params(
        config: &RollupConfig,
        parent_header: &Header,
        payload_attrs: &OpPayloadAttributes,
    ) -> ExecutorResult<BaseFeeParams> {
        let base_fee_params =
            if config.is_holocene_active(payload_attrs.payload_attributes.timestamp) {
                // After Holocene activation, the base fee parameters are stored in the
                // `extraData` field of the parent header. If Holocene wasn't active in the
                // parent block, the default base fee parameters are used.
                config
                    .is_holocene_active(parent_header.timestamp)
                    .then(|| decode_holocene_eip_1559_params(parent_header))
                    .transpose()?
                    .unwrap_or(config.chain_op_config.as_canyon_base_fee_params())
            } else if config.is_canyon_active(payload_attrs.payload_attributes.timestamp) {
                // If the payload attribute timestamp is past canyon activation,
                // use the canyon base fee params from the rollup config.
                config.chain_op_config.as_canyon_base_fee_params()
            } else {
                // If the payload attribute timestamp is prior to canyon activation,
                // use the default base fee params from the rollup config.
                config.chain_op_config.as_base_fee_params()
            };

        Ok(base_fee_params)
    }

    /// Decodes and recovers the signature of an EIP-2718-encoded transaction.
    pub(crate) fn decode_and_recover_tx(tx: &Bytes) -> ExecutorResult<Recovered<OpTxEnvelope>> {
        let decoded =
            OpTxEnvelope::decode_2718(&mut tx.as_ref()).map_err(ExecutorError::RLPError)?;
        let signer = match decoded.as_ref() {
            OpTxEnvelope::Legacy(tx) => {
                tx.recover_signer().map_err(ExecutorError::SignatureError)?
            }
            OpTxEnvelope::Eip2930(tx) => {
                tx.recover_signer().map_err(ExecutorError::SignatureError)?
            }
            OpTxEnvelope::Eip1559(tx) => {
                tx.recover_signer().map_err(ExecutorError::SignatureError)?
            }
            OpTxEnvelope::Eip7702(tx) => {
                tx.recover_signer().map_err(ExecutorError::SignatureError)?
            }
            OpTxEnvelope::Deposit(tx) => tx.from,
        };
        Ok(Recovered::new_unchecked(decoded, signer))
    }
}
