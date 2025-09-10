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
use std::{sync::Arc, time::Instant};
use tokio::sync::mpsc;

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
    pub next_attributes: OpAttributesWithParent,
    /// Whether or not the payload was derived, or created by the sequencer.
    pub is_attributes_derived: bool,
    /// The ID of the last payload built that needs to be sealed.
    /// At startup, this is `None`
    pub last_payload_data: Option<LastPayloadData>,
    /// A channel to send the [`PayloadId`] of the payload that was built
    pub payload_id_tx: mpsc::Sender<PayloadId>,
}

/// The last payload data to seal and insert in the EL.
#[derive(Debug, Clone)]
pub struct LastPayloadData {
    /// The ID of the last payload built that needs to be sealed.
    pub payload_id: PayloadId,
    /// The payload attributes of the last payload built that needs to be sealed.
    /// This is used to re-attempt payload import with deposits only after holocene.
    pub payload_attributes: OpAttributesWithParent,
    /// A channel to send the built [`OpExecutionPayloadEnvelope`] to, after the block
    /// has been built, imported, and canonicalized.
    pub built_payload: mpsc::Sender<OpExecutionPayloadEnvelope>,
}

impl BuildTask {
    /// Creates a new block building task.
    pub const fn new(
        engine: Arc<EngineClient>,
        cfg: Arc<RollupConfig>,
        next_attributes: OpAttributesWithParent,
        is_attributes_derived: bool,
        last_payload_data: Option<LastPayloadData>,
        payload_id_tx: mpsc::Sender<PayloadId>,
    ) -> Self {
        Self {
            engine,
            cfg,
            next_attributes,
            is_attributes_derived,
            last_payload_data,
            payload_id_tx,
        }
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
    pub(crate) async fn start_build_next_attributes(
        cfg: &RollupConfig,
        engine_client: &EngineClient,
        next_attributes: OpAttributesWithParent,
        state: &EngineState,
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

        let fcu_start_time = Instant::now();

        // When inserting a payload, we advertise the parent's unsafe head as the current unsafe
        // head to build on top of.
        let new_forkchoice = state
            .sync_state
            .apply_update(EngineSyncStateUpdate {
                unsafe_head: Some(next_attributes.parent),
                ..Default::default()
            })
            .create_forkchoice_state();

        let forkchoice_version = EngineForkchoiceVersion::from_cfg(
            &cfg,
            next_attributes.inner.payload_attributes.timestamp,
        );
        let update = match forkchoice_version {
            EngineForkchoiceVersion::V3 => {
                engine_client
                    .fork_choice_updated_v3(new_forkchoice, Some(next_attributes.inner))
                    .await
            }
            EngineForkchoiceVersion::V2 => {
                engine_client
                    .fork_choice_updated_v2(new_forkchoice, Some(next_attributes.inner))
                    .await
            }
        }
        .map_err(|e| {
            error!(target: "engine_builder", "Forkchoice update failed: {}", e);
            BuildTaskError::EngineBuildError(EngineBuildError::AttributesInsertionFailed(e))
        })?;

        match update.payload_status.status {
            PayloadStatusEnum::Valid => {
                let fcu_duration = fcu_start_time.elapsed();
                debug!(
                    target: "engine_builder",
                    unsafe_hash = new_forkchoice.head_block_hash.to_string(),
                    safe_hash = new_forkchoice.safe_block_hash.to_string(),
                    finalized_hash = new_forkchoice.finalized_block_hash.to_string(),
                    ?fcu_duration,
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
        cfg: &RollupConfig,
        engine: &EngineClient,
        payload_id: PayloadId,
        payload_timestamp: u64,
    ) -> Result<OpExecutionPayloadEnvelope, BuildTaskError> {
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

    pub(crate) async fn seal_and_insert_payload(
        state: &mut EngineState,
        cfg: &RollupConfig,
        engine: &EngineClient,
        last_payload_data: LastPayloadData,
        is_attributes_derived: bool,
    ) -> Result<(), BuildTaskError> {
        // Get the last payload data.
        let LastPayloadData {
            payload_id,
            payload_attributes: last_payload_attributes,
            built_payload,
        } = last_payload_data;

        let payload_timestamp = last_payload_attributes.inner().payload_attributes.timestamp;
        let parent_beacon_block_root =
            last_payload_attributes.inner().payload_attributes.parent_beacon_block_root;

        // Fetch the payload just inserted from the EL and import it into the engine.
        let block_import_start_time = Instant::now();
        let new_payload =
            Self::fetch_payload(&cfg, &engine, payload_id.clone(), payload_timestamp).await?;

        let new_block_ref = L2BlockInfo::from_payload_and_genesis(
            new_payload.execution_payload.clone(),
            parent_beacon_block_root,
            &cfg.genesis,
        )
        .map_err(BuildTaskError::FromBlock)?;

        // Insert the new block into the engine.
        match InsertTask::new(
            Arc::new(engine.clone()),
            Arc::new(cfg.clone()),
            new_payload.clone(),
            is_attributes_derived,
        )
        .execute(state)
        .await
        {
            Err(InsertTaskError::UnexpectedPayloadStatus(e))
                if last_payload_attributes.is_deposits_only() =>
            {
                error!(target: "engine_builder", error = ?e, "Critical: Deposit-only payload import failed");
                return Err(BuildTaskError::DepositOnlyPayloadFailed);
            }
            // HOLOCENE: Re-attempt payload import with deposits only
            Err(InsertTaskError::UnexpectedPayloadStatus(e))
                if cfg.is_holocene_active(payload_timestamp) =>
            {
                warn!(target: "engine_builder", error = ?e, "Re-attempting payload import with deposits only.");

                // HOLOCENE: Re-attempt payload import with deposits only. We don't need to wait for
                // transactions to be included in the block so we can seal
                // immediately

                let deposits_only_attrs = last_payload_attributes.as_deposits_only();

                // Build a new payload with deposits only this time
                let payload_id = Self::start_build_next_attributes(
                    &cfg,
                    &engine,
                    deposits_only_attrs.clone(),
                    state,
                )
                .await?;

                return match Box::pin(Self::seal_and_insert_payload(
                    state,
                    cfg,
                    engine,
                    LastPayloadData {
                        payload_id,
                        payload_attributes: deposits_only_attrs,
                        built_payload,
                    },
                    is_attributes_derived,
                ))
                .await
                {
                    Ok(_) => {
                        info!(target: "engine_builder", "Successfully imported deposits-only payload. Flushing engine...");
                        Err(BuildTaskError::HoloceneInvalidFlush)
                    }
                    Err(_) => Err(BuildTaskError::DepositOnlyPayloadReattemptFailed),
                }
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

        info!(
            target: "engine_builder",
            l2_number = new_block_ref.block_info.number,
            l2_time = new_block_ref.block_info.timestamp,
            block_import_duration = ?block_import_duration,
            "Imported new {} block",
            if is_attributes_derived { "safe" } else { "unsafe" },
        );

        // If a channel was provided, send the built payload envelope to it.
        built_payload
            .send(new_payload)
            .await
            .map_err(Box::new)
            .map_err(BuildTaskError::MpscSendEnvelope)?;

        Ok(())
    }
}

#[async_trait]
impl EngineTaskExt for BuildTask {
    type Output = ();

    type Error = BuildTaskError;

    async fn execute(&self, state: &mut EngineState) -> Result<(), BuildTaskError> {
        // If there is last payload data, seal and insert the payload.
        if let Some(last_payload_data) = &self.last_payload_data {
            Self::seal_and_insert_payload(
                state,
                &self.cfg,
                &self.engine,
                last_payload_data.clone(),
                self.is_attributes_derived,
            )
            .await?;
        }

        debug!(
            target: "engine_builder",
            txs = self.next_attributes.inner().transactions.as_ref().map_or(0, |txs| txs.len()),
            is_deposits = self.next_attributes.is_deposits_only(),
            "Starting new build job"
        );

        // Start the build by sending an FCU call with the current forkchoice and the input
        // payload attributes.
        let payload_id = Self::start_build_next_attributes(
            &self.cfg,
            &self.engine,
            self.next_attributes.clone(),
            state,
        )
        .await?;

        // Send the payload ID being built to the channel.
        self.payload_id_tx
            .send(payload_id)
            .await
            .map_err(Box::new)
            .map_err(BuildTaskError::MpscSendId)?;

        Ok(())
    }
}
