//! A task for building a new block and importing it.
use super::BuildTaskError;
use crate::{
    EngineClient, EngineForkchoiceVersion, EngineGetPayloadVersion, EngineState, EngineTaskExt,
    InsertTask,
    InsertTaskError::{self},
    state::EngineSyncStateUpdate,
    task_queue::tasks::build::error::EngineBuildError,
};
use alloy_rpc_types_engine::{ExecutionPayload, PayloadId, PayloadStatusEnum};
use async_trait::async_trait;
use kona_genesis::RollupConfig;
use kona_protocol::{L2BlockInfo, OpAttributesWithParent};
use op_alloy_provider::ext::engine::OpEngineApi;
use op_alloy_rpc_types_engine::{OpExecutionPayload, OpExecutionPayloadEnvelope};
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::{sync::mpsc, time::sleep};

/// Task for building new blocks with automatic forkchoice synchronization.
///
/// The [`BuildTask`] handles the complete block building workflow, including:
///
/// 1. **Automatic Forkchoice Updates**: Performs initial `engine_forkchoiceUpdated` call with
///    payload attributes to initiate block building on the execution layer
/// 2. **Payload Construction**: Retrieves the built payload using `engine_getPayload`
/// 3. **Block Import**: Imports the payload using [`InsertTask`] for canonicalization
///
/// ## Forkchoice Integration
///
/// Unlike previous versions where forkchoice updates required separate tasks,
/// `BuildTask` now handles forkchoice synchronization automatically as part of
/// the block building process. This eliminates the need for explicit forkchoice
/// management and ensures atomic block building operations.
///
/// ## Error Handling
///
/// The task uses [`EngineBuildError`] for build-specific failures during the forkchoice
/// update phase, and delegates to [`InsertTaskError`] for payload import failures.
///
/// [`InsertTask`]: crate::InsertTask
/// [`EngineBuildError`]: crate::EngineBuildError
/// [`InsertTaskError`]: crate::InsertTaskError
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

    /// Starts the block building process by sending an initial `engine_forkchoiceUpdate` call with
    /// the payload attributes to build.
    ///
    /// ## Observed [PayloadStatusEnum] Variants
    /// The `engine_forkchoiceUpdate` payload statuses that this function observes are below. Any
    /// other [PayloadStatusEnum] variant is considered a failure.
    ///
    /// ### Success (`VALID`)
    /// If the build is successful, the [PayloadId] is returned for sealing and the external
    /// actor is notified of the successful forkchoice update.
    ///
    /// ### Failure (`INVALID`)
    /// If the forkchoice update fails, the external actor is notified of the failure.
    ///
    /// ### Syncing (`SYNCING`)
    /// If the EL is syncing, the payload attributes are buffered and the function returns early.
    /// This is a temporary state, and the function should be called again later.
    async fn start_build(
        &self,
        state: &EngineState,
        engine_client: &EngineClient,
        attributes_envelope: OpAttributesWithParent,
    ) -> Result<PayloadId, BuildTaskError> {
        // Sanity check if the head is behind the finalized head. If it is, this is a critical
        // error.
        if state.sync_state.unsafe_head().block_info.number <
            state.sync_state.finalized_head().block_info.number
        {
            return Err(BuildTaskError::EngineBuildError(EngineBuildError::FinalizedAheadOfUnsafe(
                state.sync_state.unsafe_head().block_info.number,
                state.sync_state.finalized_head().block_info.number,
            )));
        }

        // When inserting a payload, we advertise the parent's unsafe head as the current unsafe
        // head to build on top of.
        let new_forkchoice = state
            .sync_state
            .apply_update(EngineSyncStateUpdate {
                unsafe_head: Some(attributes_envelope.parent),
                ..Default::default()
            })
            .create_forkchoice_state();

        let forkchoice_version = EngineForkchoiceVersion::from_cfg(
            &self.cfg,
            attributes_envelope.inner.payload_attributes.timestamp,
        );
        let update = match forkchoice_version {
            EngineForkchoiceVersion::V3 => {
                engine_client
                    .fork_choice_updated_v3(new_forkchoice, Some(attributes_envelope.inner))
                    .await
            }
            EngineForkchoiceVersion::V2 => {
                engine_client
                    .fork_choice_updated_v2(new_forkchoice, Some(attributes_envelope.inner))
                    .await
            }
        }
        .map_err(|e| {
            error!(target: "engine_builder", "Forkchoice update failed: {}", e);
            BuildTaskError::EngineBuildError(EngineBuildError::AttributesInsertionFailed(e))
        })?;

        match update.payload_status.status {
            PayloadStatusEnum::Valid => {
                debug!(
                    target: "engine_builder",
                    unsafe_hash = new_forkchoice.head_block_hash.to_string(),
                    safe_hash = new_forkchoice.safe_block_hash.to_string(),
                    finalized_hash = new_forkchoice.finalized_block_hash.to_string(),
                    "Forkchoice update with attributes successful"
                );
            }
            PayloadStatusEnum::Invalid { validation_error } => {
                error!(target: "engine_builder", "Forkchoice update failed: {}", validation_error);
                return Err(BuildTaskError::EngineBuildError(EngineBuildError::InvalidPayload(
                    validation_error,
                )));
            }
            PayloadStatusEnum::Syncing => {
                warn!(target: "engine_builder", "Forkchoice update failed temporarily: EL is syncing");
                return Err(BuildTaskError::EngineBuildError(EngineBuildError::EngineSyncing));
            }
            s => {
                // Other codes are never returned by `engine_forkchoiceUpdate`
                return Err(BuildTaskError::EngineBuildError(
                    EngineBuildError::UnexpectedPayloadStatus(s),
                ));
            }
        }

        // Fetch the payload ID from the FCU. If no payload ID was returned, something went wrong -
        // the block building job on the EL should have been initiated.
        update
            .payload_id
            .ok_or(BuildTaskError::EngineBuildError(EngineBuildError::MissingPayloadId))
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
            is_deposits = self.attributes.is_deposits_only(),
            "Starting new build job"
        );

        // Start the build by sending an FCU call with the current forkchoice and the input
        // payload attributes.
        let fcu_start_time = Instant::now();
        let payload_id = self.start_build(state, &self.engine, self.attributes.clone()).await?;

        let fcu_duration = fcu_start_time.elapsed();

        // Compute the time of the next block.
        let next_block = Duration::from_secs(
            self.attributes.parent().block_info.timestamp.saturating_add(self.cfg.block_time),
        );

        // Compute the time left to seal the next block.
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| BuildTaskError::ClockWentBackwards)?;

        // Add a buffer to the time left to seal the next block.
        const SEALING_BUFFER: Duration = Duration::from_millis(50);

        let time_left_to_seal = next_block.saturating_sub(now).saturating_sub(SEALING_BUFFER);

        // Wait for the time left to seal the next block.
        if !time_left_to_seal.is_zero() {
            sleep(time_left_to_seal).await;
        }

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
            Err(InsertTaskError::UnexpectedPayloadStatus(e))
                if self.attributes.is_deposits_only() =>
            {
                error!(target: "engine_builder", error = ?e, "Critical: Deposit-only payload import failed");
                return Err(BuildTaskError::DepositOnlyPayloadFailed);
            }
            // HOLOCENE: Re-attempt payload import with deposits only
            Err(InsertTaskError::UnexpectedPayloadStatus(e))
                if self
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
                return Err(BuildTaskError::HoloceneInvalidFlush);
            }
            Err(e) => {
                error!(target: "engine_builder", "Payload import failed: {e}");
                return Err(Box::new(e).into());
            }
            Ok(_) => {
                info!(target: "engine_builder", "Successfully imported payload")
            }
        }

        let block_import_duration = block_import_start_time.elapsed();

        // If a channel was provided, send the built payload envelope to it.
        if let Some(tx) = &self.payload_tx {
            tx.send(new_payload).await.map_err(Box::new).map_err(BuildTaskError::MpscSend)?;
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
