//! A task for importing a block that has already been started.
use super::SealTaskError;
use crate::{
    EngineClient, EngineGetPayloadVersion, EngineState, EngineTaskExt, InsertTask,
    InsertTaskError::{self},
    task_queue::build_and_seal,
};
use alloy_rpc_types_engine::{ExecutionPayload, PayloadId};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OpAttributesWithParent};
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::{OpExecutionPayload, OpExecutionPayloadEnvelope};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

/// Task for sealing blocks.
///
/// The [`SealTask`] handles the following parts of the block building workflow:
///
/// 1. **Payload Construction**: Retrieves the built payload using `engine_getPayload`
/// 2. **Block Import**: Imports the payload using [`InsertTask`] for canonicalization
///
/// ## Error Handling
///
/// The task delegates to [`InsertTaskError`] for payload import failures.
///
/// [`InsertTask`]: crate::InsertTask
/// [`InsertTaskError`]: crate::InsertTaskError
#[derive(Debug, Clone)]
pub struct SealTask {
    /// The engine API client.
    pub engine: Arc<EngineClient>,
    /// The [`RollupConfig`].
    pub cfg: Arc<RollupConfig>,
    /// The [`PayloadId`] being sealed.
    pub payload_id: PayloadId,
    /// The [`OpAttributesWithParent`] to instruct the execution layer to build.
    pub attributes: OpAttributesWithParent,
    /// Whether or not the payload was derived, or created by the sequencer.
    pub is_attributes_derived: bool,
    /// An optional channel to send the built [`OpExecutionPayloadEnvelope`] to, after the block
    /// has been built, imported, and canonicalized.
    pub payload_tx: Option<mpsc::Sender<OpExecutionPayloadEnvelope>>,
}

impl SealTask {
    /// Creates a new block building task.
    pub const fn new(
        engine: Arc<EngineClient>,
        cfg: Arc<RollupConfig>,
        payload_id: PayloadId,
        attributes: OpAttributesWithParent,
        is_attributes_derived: bool,
        payload_tx: Option<mpsc::Sender<OpExecutionPayloadEnvelope>>,
    ) -> Self {
        Self { engine, cfg, payload_id, attributes, is_attributes_derived, payload_tx }
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
    ) -> Result<OpExecutionPayloadEnvelope, SealTaskError> {
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
                    SealTaskError::GetPayloadFailed(e)
                })?;

                OpExecutionPayloadEnvelope {
                    parent_beacon_block_root: Some(payload.parent_beacon_block_root),
                    execution_payload: OpExecutionPayload::V4(payload.execution_payload),
                }
            }
            EngineGetPayloadVersion::V3 => {
                let payload = engine.get_payload_v3(payload_id).await.map_err(|e| {
                    error!(target: "engine_builder", "Payload fetch failed: {e}");
                    SealTaskError::GetPayloadFailed(e)
                })?;

                OpExecutionPayloadEnvelope {
                    parent_beacon_block_root: Some(payload.parent_beacon_block_root),
                    execution_payload: OpExecutionPayload::V3(payload.execution_payload),
                }
            }
            EngineGetPayloadVersion::V2 => {
                let payload = engine.get_payload_v2(payload_id).await.map_err(|e| {
                    error!(target: "engine_builder", "Payload fetch failed: {e}");
                    SealTaskError::GetPayloadFailed(e)
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

    /// Seals the block by waiting for the appropriate time, fetching the payload, and importing it.
    ///
    /// This function handles:
    /// 1. Computing and waiting for the seal time (with a buffer)
    /// 2. Fetching the execution payload from the EL
    /// 3. Importing the payload into the engine with Holocene fallback support
    /// 4. Sending the payload to the optional channel
    ///
    /// Returns the imported block info and the duration taken to import the block.
    async fn seal_block(
        &self,
        state: &mut EngineState,
    ) -> Result<(L2BlockInfo, Duration), SealTaskError> {
        // Fetch the payload just inserted from the EL and import it into the engine.
        let block_import_start_time = Instant::now();
        let new_payload = self
            .fetch_payload(&self.cfg, &self.engine, self.payload_id, self.attributes.clone())
            .await?;

        let new_block_ref = L2BlockInfo::from_payload_and_genesis(
            new_payload.execution_payload.clone(),
            self.attributes.inner().payload_attributes.parent_beacon_block_root,
            &self.cfg.genesis,
        )
        .map_err(SealTaskError::FromBlock)?;

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
            Err(InsertTaskError::UnexpectedPayloadStatus(e))
                if self.attributes.is_deposits_only() =>
            {
                error!(target: "engine_sealer", error = ?e, "Critical: Deposit-only payload import failed");
                return Err(SealTaskError::DepositOnlyPayloadFailed);
            }
            // HOLOCENE: Re-attempt payload import with deposits only
            Err(InsertTaskError::UnexpectedPayloadStatus(e))
                if self
                    .cfg
                    .is_holocene_active(self.attributes.inner().payload_attributes.timestamp) =>
            {
                warn!(target: "engine_sealer", error = ?e, "Re-attempting payload import with deposits only.");

                // HOLOCENE: Re-attempt payload import with deposits only
                // First build the deposits-only payload, then seal it
                let deposits_only_attrs = self.attributes.as_deposits_only();

                return match build_and_seal(
                    state,
                    self.engine.clone(),
                    self.cfg.clone(),
                    deposits_only_attrs.clone(),
                    self.is_attributes_derived,
                    None,
                )
                .await
                {
                    Ok(_) => {
                        info!(target: "engine_sealer", "Successfully imported deposits-only payload");
                        Err(SealTaskError::HoloceneInvalidFlush)
                    }
                    Err(_) => return Err(SealTaskError::DepositOnlyPayloadReattemptFailed),
                }
            }
            Err(e) => {
                error!(target: "engine_sealer", "Payload import failed: {e}");
                return Err(Box::new(e).into());
            }
            Ok(_) => {
                info!(target: "engine_sealer", "Successfully imported payload")
            }
        }

        let block_import_duration = block_import_start_time.elapsed();

        // If a channel was provided, send the built payload envelope to it.
        if let Some(tx) = &self.payload_tx {
            tx.send(new_payload).await.map_err(Box::new).map_err(SealTaskError::MpscSend)?;
        }

        Ok((new_block_ref, block_import_duration))
    }
}

#[async_trait]
impl EngineTaskExt for SealTask {
    type Output = ();

    type Error = SealTaskError;

    async fn execute(&self, state: &mut EngineState) -> Result<(), SealTaskError> {
        debug!(
            target: "engine_sealer",
            txs = self.attributes.inner().transactions.as_ref().map_or(0, |txs| txs.len()),
            is_deposits = self.attributes.is_deposits_only(),
            "Starting new seal job"
        );

        // Seal the block and import it into the engine.
        let (new_block_ref, block_import_duration) = self.seal_block(state).await?;

        info!(
            target: "engine_sealer",
            l2_number = new_block_ref.block_info.number,
            l2_time = new_block_ref.block_info.timestamp,
            block_import_duration = ?block_import_duration,
            "Built and imported new {} block",
            if self.is_attributes_derived { "safe" } else { "unsafe" },
        );

        Ok(())
    }
}
