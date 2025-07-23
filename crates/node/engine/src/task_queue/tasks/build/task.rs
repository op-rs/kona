//! A task for building a new block and importing it.
use super::BuildTaskError;
use crate::{
    EngineClient, EngineGetPayloadVersion, EngineState, EngineTaskExt, ForkchoiceTask,
    ForkchoiceTaskError, InsertTask,
    InsertTaskError::{self},
    state::EngineSyncStateUpdate,
};
use alloy_rpc_types_engine::{ExecutionPayload, PayloadId};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OpAttributesWithParent};
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::{OpExecutionPayload, OpExecutionPayloadEnvelope};
use std::{sync::Arc, time::Instant};
use tokio::sync::mpsc;

/// The [`BuildTask`] is responsible for building new blocks and importing them via the engine API.
#[derive(Debug, Clone)]
pub struct BuildTask {
    /// The engine API client.
    pub engine: Arc<EngineClient>,
    /// The [`RollupConfig`].
    pub cfg: Arc<RollupConfig>,
    /// The [`OpAttributesWithParent`] to instruct the execution layer to build.
    pub attributes: OpAttributesWithParent,
    /// Whether or not the payload was derived, or created by the sequencer.
    pub is_attributes_derived: bool,
    /// An optional channel to send the built [`OpExecutionPayloadEnvelope`] to, after the block
    /// has been built, imported, and canonicalized.
    pub payload_tx: Option<mpsc::Sender<OpExecutionPayloadEnvelope>>,
}

impl BuildTask {
    /// Creates a new block building task.
    pub const fn new(
        engine: Arc<EngineClient>,
        cfg: Arc<RollupConfig>,
        attributes: OpAttributesWithParent,
        is_attributes_derived: bool,
        payload_tx: Option<mpsc::Sender<OpExecutionPayloadEnvelope>>,
    ) -> Self {
        Self { engine, cfg, attributes, is_attributes_derived, payload_tx }
    }

    /// Fetches the execution payload from the EL.
    ///
    /// ## Engine Method Selection
    /// The method used to fetch the payload from the EL is determined by the payload timestamp. The
    /// method used to import the payload into the engine is determined by the payload version.
    ///
    /// - `engine_getPayloadV2` is used for payloads with a timestamp before the Ecotone fork.
    /// - `engine_getPayloadV3` is used for payloads with a timestamp after the Ecotone fork.
    /// - `engine_getPayloadV4` is used for payloads with a timestamp after the Isthmus fork.
    async fn fetch_payload(
        &self,
        cfg: &RollupConfig,
        engine: &EngineClient,
        payload_id: PayloadId,
        payload_attrs: OpAttributesWithParent,
    ) -> Result<OpExecutionPayloadEnvelope, BuildTaskError> {
        let payload_timestamp = payload_attrs.inner().payload_attributes.timestamp;

        debug!(
            target: "engine_builder",
            payload_id = payload_id.to_string(),
            l2_time = payload_timestamp,
            "Inserting payload"
        );

        let get_payload_version = EngineGetPayloadVersion::from_cfg(cfg, payload_timestamp);
        let payload_envelope = match get_payload_version {
            EngineGetPayloadVersion::V4 => {
                let payload = engine.get_payload_v4(payload_id).await.map_err(|e| {
                    error!(target: "engine_builder", "Payload fetch failed: {e}");
                    BuildTaskError::GetPayloadFailed(e)
                })?;

                OpExecutionPayloadEnvelope {
                    parent_beacon_block_root: Some(payload.parent_beacon_block_root),
                    execution_payload: OpExecutionPayload::V4(payload.execution_payload),
                }
            }
            EngineGetPayloadVersion::V3 => {
                let payload = engine.get_payload_v3(payload_id).await.map_err(|e| {
                    error!(target: "engine_builder", "Payload fetch failed: {e}");
                    BuildTaskError::GetPayloadFailed(e)
                })?;

                OpExecutionPayloadEnvelope {
                    parent_beacon_block_root: Some(payload.parent_beacon_block_root),
                    execution_payload: OpExecutionPayload::V3(payload.execution_payload),
                }
            }
            EngineGetPayloadVersion::V2 => {
                let payload = engine.get_payload_v2(payload_id).await.map_err(|e| {
                    error!(target: "engine_builder", "Payload fetch failed: {e}");
                    BuildTaskError::GetPayloadFailed(e)
                })?;

                OpExecutionPayloadEnvelope {
                    parent_beacon_block_root: None,
                    execution_payload: match payload.execution_payload.into_payload() {
                        ExecutionPayload::V1(payload) => OpExecutionPayload::V1(payload),
                        ExecutionPayload::V2(payload) => OpExecutionPayload::V2(payload),
                        _ => unreachable!("the response should be a V1 or V2 payload"),
                    },
                }
            }
        };

        Ok(payload_envelope)
    }
}

#[async_trait]
impl EngineTaskExt for BuildTask {
    type Output = ();

    type Error = BuildTaskError;

    async fn execute(&self, state: &mut EngineState) -> Result<(), BuildTaskError> {
        debug!(
            target: "engine_builder",
            txs = self.attributes.inner().transactions.as_ref().map_or(0, |txs| txs.len()),
            "Starting new build job"
        );

        // Start the build by sending an FCU call with the current forkchoice and the input
        // payload attributes.
        let fcu_start_time = Instant::now();
        let payload_id = ForkchoiceTask::new(
            self.engine.clone(),
            self.cfg.clone(),
            EngineSyncStateUpdate {
                unsafe_head: Some(self.attributes.parent),
                ..Default::default()
            },
            Some(self.attributes.clone()),
        )
        .execute(state)
        .await?
        .ok_or(BuildTaskError::MissingPayloadId)?;
        let fcu_duration = fcu_start_time.elapsed();

        // Fetch the payload just inserted from the EL and import it into the engine.
        let block_import_start_time = Instant::now();
        let new_payload = self
            .fetch_payload(&self.cfg, &self.engine, payload_id, self.attributes.clone())
            .await?;

        let new_block_ref = L2BlockInfo::from_payload_and_genesis(
            new_payload.execution_payload.clone(),
            self.attributes.inner().payload_attributes.parent_beacon_block_root,
            &self.cfg.genesis,
        )
        .map_err(BuildTaskError::FromBlock)?;

        // Insert the new block into the engine.
        match InsertTask::new(
            Arc::clone(&self.engine),
            self.cfg.clone(),
            new_payload.clone(),
            self.is_attributes_derived,
        )
        .execute(state)
        .await
        {
            Err(InsertTaskError::ForkchoiceUpdateFailed(
                ForkchoiceTaskError::InvalidPayloadStatus(e),
            )) if self.attributes.is_deposits_only() => {
                error!(target: "engine_builder", error = ?e, "Critical: Deposit-only payload import failed");
                return Err(BuildTaskError::DepositOnlyPayloadFailed)
            }
            // HOLOCENE: Re-attempt payload import with deposits only
            Err(InsertTaskError::ForkchoiceUpdateFailed(
                ForkchoiceTaskError::InvalidPayloadStatus(e),
            )) if self
                .cfg
                .is_holocene_active(self.attributes.inner().payload_attributes.timestamp) =>
            {
                warn!(target: "engine_builder", error = ?e, "Re-attempting payload import with deposits only.");
                // HOLOCENE: Re-attempt payload import with deposits only
                match Self::new(
                    self.engine.clone(),
                    self.cfg.clone(),
                    self.attributes.as_deposits_only(),
                    self.is_attributes_derived,
                    self.payload_tx.clone(),
                )
                .execute(state)
                .await
                {
                    Ok(_) => {
                        info!(target: "engine_builder", "Successfully imported deposits-only payload")
                    }
                    Err(_) => return Err(BuildTaskError::DepositOnlyPayloadReattemptFailed),
                }
                return Err(BuildTaskError::HoloceneInvalidFlush)
            }
            Err(e) => {
                error!(target: "engine_builder", "Payload import failed: {e}");
                return Err(e.into())
            }
            Ok(_) => {
                info!(target: "engine_builder", "Successfully imported payload")
            }
        }

        let block_import_duration = block_import_start_time.elapsed();

        // If a channel was provided, send the built payload envelope to it.
        if let Some(tx) = &self.payload_tx {
            tx.send(new_payload).await.map_err(BuildTaskError::MpscSend)?;
        }

        info!(
            target: "engine_builder",
            l2_number = new_block_ref.block_info.number,
            l2_time = new_block_ref.block_info.timestamp,
            fcu_duration = ?fcu_duration,
            block_import_duration = ?block_import_duration,
            "Built and imported new {} block",
            if self.is_attributes_derived { "safe" } else { "unsafe" },
        );

        Ok(())
    }
}
