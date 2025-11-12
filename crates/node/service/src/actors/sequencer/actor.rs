//! The [`SequencerActor`].

use super::{
    DelayedL1OriginSelectorProvider, L1OriginSelector, L1OriginSelectorError, SequencerConfig,
};
use crate::{
    actors::{
        sequencer::{
            conductor::{Conductor, ConductorClient},
            origin_selector::OriginSelector,
            rpc::{QueuedSequencerAdminAPIClient, SequencerAdminQuery},
        }, BlockEngine,
    }, CancellableContext, NodeActor
};
use alloy_primitives::B256;
use alloy_provider::RootProvider;
use alloy_rpc_types_engine::PayloadId;
use async_trait::async_trait;
use derive_more::Constructor;
use kona_derive::{AttributesBuilder, PipelineErrorKind, StatefulAttributesBuilder};
use kona_engine::{BuildError, SealError};
use kona_genesis::{L1ChainConfig, RollupConfig};
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use kona_providers_alloy::{AlloyChainProvider, AlloyL2ChainProvider};
use kona_rpc::{SequencerAdminAPIClient, SequencerAdminAPIError};
use op_alloy_network::Optimism;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    select,
    sync::{mpsc, watch},
};
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture};

/// The [`SequencerActor`] is responsible for building L2 blocks on top of the current unsafe head
/// and scheduling them to be signed and gossipped by the P2P layer, extending the L2 chain with new
/// blocks.
#[derive(Debug)]
pub struct SequencerActor<AB: AttributesBuilderConfig> {
    /// The [`AttributesBuilderConfig`].
    pub builder: AB,

    /// Receiver for admin API requests.
    pub admin_api_rx: mpsc::Receiver<SequencerAdminQuery>,

    /// Watch channel to observe the unsafe head of the engine.
    pub unsafe_head_rx: watch::Receiver<L2BlockInfo>,
}

/// The handle to a block that has been started but not sealed.
#[derive(Debug, Constructor)]
pub(super) struct UnsealedPayloadHandle {
    /// The [`PayloadId`] of the unsealed payload.
    pub payload_id: PayloadId,
    /// The [`OpAttributesWithParent`] used to start block building.
    pub attributes_with_parent: OpAttributesWithParent,
}

/// The return payload of the `seal_last_and_start_next` function. This allows the sequencer
/// to make an informed decision about when to seal and build the next block.
#[derive(Debug, Constructor)]
pub(super) struct SealLastStartNextResult {
    /// The [`UnsealedPayloadHandle`] that was built.
    pub unsealed_payload_handle: Option<UnsealedPayloadHandle>,
    /// How long it took to execute the seal operation.
    pub seconds_to_seal: u64,
}

/// The state of the [`SequencerActor`].
#[derive(Debug)]
pub struct SequencerActorState {
    /// The [`RollupConfig`] for the chain being sequenced.
    pub cfg: Arc<RollupConfig>,
    /// Whether the sequencer is active. This is used inside communications between the sequencer
    /// and the op-conductor to activate/deactivate the sequencer when leader election occurs.
    ///
    /// ## Default value
    /// At startup, the sequencer is active.
    pub is_active: bool,
    /// Whether the sequencer is in recovery mode.
    ///
    /// ## Default value
    /// At startup, the sequencer is _NOT_ in recovery mode.
    pub is_recovery_mode: bool,
    /// The attributes builder used for block building.
    pub attributes_builder: Box<dyn AttributesBuilder + Sync>,
    /// The optional conductor RPC client.
    pub conductor: Option<Box<dyn Conductor + Sync>>,
    /// The struct used to determine the next L1 origin, given some inputs.
    pub origin_selector: Box<dyn OriginSelector + Sync>,
    /// The struct used to build blocks.
    pub block_engine: Box<dyn BlockEngine + Sync>,
    /// The cancellation token, shared between all tasks.
    pub cancellation: CancellationToken,
    /// Sender to request the engine to reset.
    pub reset_request_tx: mpsc::Sender<()>,
    /// A sender to asynchronously sign and gossip built [`OpExecutionPayloadEnvelope`]s to the
    /// network actor.
    pub gossip_payload_tx: mpsc::Sender<OpExecutionPayloadEnvelope>,
}

/// A trait for building [`AttributesBuilder`]s.
pub trait AttributesBuilderConfig {
    /// The type of [`AttributesBuilder`] to build.
    type AB: AttributesBuilder;

    /// Builds the [`AttributesBuilder`].
    fn build(self) -> Self::AB;
}

impl SequencerActorState {
    /// Creates a new `SequencerActorState` with injected dependencies.
    /// This is the primary constructor that accepts all dependencies for full testability.
    pub fn new(
        cfg: Arc<RollupConfig>,
        is_active: bool,
        is_recovery_mode: bool,
        attributes_builder: Box<dyn AttributesBuilder + Sync>,
        conductor: Option<Box<dyn Conductor + Sync>>,
        origin_selector: Box<dyn OriginSelector + Sync>,
        block_engine: Box<dyn BlockEngine + Sync>,
        cancellation: CancellationToken,
        reset_request_tx: mpsc::Sender<()>,
        gossip_payload_tx: mpsc::Sender<OpExecutionPayloadEnvelope>,
    ) -> Self {
        Self {
            cfg,
            is_active,
            is_recovery_mode,
            attributes_builder,
            conductor,
            origin_selector,
            block_engine,
            cancellation,
            reset_request_tx,
            gossip_payload_tx,
        }
    }
}

const DERIVATION_PROVIDER_CACHE_SIZE: usize = 1024;

/// The builder for the [`SequencerActor`].
#[derive(Clone, Debug)]
pub struct SequencerBuilder {
    /// The [`SequencerConfig`].
    pub seq_cfg: SequencerConfig,
    /// The [`RollupConfig`] for the chain being sequenced.
    pub rollup_cfg: Arc<RollupConfig>,
    /// The [`L1ChainConfig`] for the chain being sequenced.
    pub l1_config: Arc<L1ChainConfig>,
    /// The L1 provider.
    pub l1_provider: RootProvider,
    /// Whether to trust the L1 RPC.
    pub l1_trust_rpc: bool,
    /// The L2 provider.
    pub l2_provider: RootProvider<Optimism>,
    /// Whether to trust the L2 RPC.
    pub l2_trust_rpc: bool,
}

impl AttributesBuilderConfig for SequencerBuilder {
    type AB = StatefulAttributesBuilder<AlloyChainProvider, AlloyL2ChainProvider>;

    fn build(self) -> Self::AB {
        let l1_derivation_provider = AlloyChainProvider::new_with_trust(
            self.l1_provider.clone(),
            DERIVATION_PROVIDER_CACHE_SIZE,
            self.l1_trust_rpc,
        );
        let l2_derivation_provider = AlloyL2ChainProvider::new_with_trust(
            self.l2_provider.clone(),
            self.rollup_cfg.clone(),
            DERIVATION_PROVIDER_CACHE_SIZE,
            self.l2_trust_rpc,
        );
        StatefulAttributesBuilder::new(
            self.rollup_cfg,
            self.l1_config,
            l2_derivation_provider,
            l1_derivation_provider,
        )
    }
}

/// The inbound channels for the [`SequencerActor`].
/// These channels are used by external actors to send messages to the sequencer actor.
#[derive(Debug)]
pub struct SequencerInboundData {
    /// Watch channel to observe the unsafe head of the engine.
    pub unsafe_head_tx: watch::Sender<L2BlockInfo>,
    /// Admin API client for external actors to send admin queries.
    pub admin_api_client: Box<dyn SequencerAdminAPIClient>,
}

/// The communication context used by the [`SequencerActor`].
#[derive(Debug)]
pub struct SequencerInitContext {
    /// The cancellation token, shared between all tasks.
    pub cancellation: CancellationToken,
    /// Watch channel to observe the L1 head of the chain.
    pub l1_head_rx: watch::Receiver<Option<BlockInfo>>,
    /// Sender to request the engine to reset.
    pub reset_request_tx: mpsc::Sender<()>,
    /// [`BlockEngine`] reference through which to build and seal blocks.
    pub block_engine: Box<dyn BlockEngine>,
    /// A sender to asynchronously sign and gossip built [`OpExecutionPayloadEnvelope`]s to the
    /// network actor.
    pub gossip_payload_tx: mpsc::Sender<OpExecutionPayloadEnvelope>,
}

impl CancellableContext for SequencerInitContext {
    fn cancelled(&self) -> WaitForCancellationFuture<'_> {
        self.cancellation.cancelled()
    }
}

/// An error produced by the [`SequencerActor`].
#[derive(Debug, thiserror::Error)]
pub enum SequencerActorError {
    /// An error occurred while building payload attributes.
    #[error(transparent)]
    AttributesBuilder(#[from] PipelineErrorKind),
    /// A channel was unexpectedly closed.
    #[error("Channel closed unexpectedly")]
    ChannelClosed,
    /// An error occurred while selecting the next L1 origin.
    #[error(transparent)]
    L1OriginSelector(#[from] L1OriginSelectorError),
    /// An error occurred while attempting to seal a payload.
    #[error(transparent)]
    SealError(#[from] SealError),
    /// An error occurred while attempting to build a payload.
    #[error(transparent)]
    BuildError(#[from] BuildError),
}

impl<AB: AttributesBuilderConfig> SequencerActor<AB> {
    /// Creates a new instance of the [`SequencerActor`].
    pub fn new(
        state: AB,
        unsafe_head_rx: watch::Receiver<L2BlockInfo>,
        admin_api_rx: mpsc::Receiver<SequencerAdminQuery>,
    ) -> Self {
        Self { builder: state, unsafe_head_rx, admin_api_rx }
    }
}

impl SequencerActorState {
    /// Seals and commits the last pending block, if one exists and starts the build job for the
    /// next L2 block, on top of the current unsafe head.
    ///
    /// If a new block was started, it will return the associated [`UnsealedPayloadHandle`] so
    /// that it may be sealed and committed in a future call to this function.
    async fn seal_last_and_start_next(
        &mut self,
        unsafe_head_rx: &mut watch::Receiver<L2BlockInfo>,
        in_recovery_mode: bool,
        payload_to_seal: Option<&UnsealedPayloadHandle>,
    ) -> Result<SealLastStartNextResult, SequencerActorError> {
        let mut seconds_to_seal = 0u64;
        if let Some(to_seal) = payload_to_seal {
            let seal_start = Instant::now();
            self.seal_and_commit_payload_if_applicable(to_seal).await?;
            seconds_to_seal = seal_start.elapsed().as_secs();
        }

        let unsealed_payload =
            self.build_unsealed_payload(unsafe_head_rx, in_recovery_mode).await?;

        Ok(SealLastStartNextResult::new(unsealed_payload, seconds_to_seal))
    }

    /// Sends a seal request to seal the provided [`UnsealedPayloadHandle`], committing and
    /// gossiping the resulting block, if one is built.
    async fn seal_and_commit_payload_if_applicable(
        &mut self,
        unsealed_payload_handle: &UnsealedPayloadHandle,
    ) -> Result<(), SequencerActorError> {
        let _seal_request_start = Instant::now();

        // Send the seal request to the engine to seal the unsealed block.
        let payload = self.block_engine.seal_and_canonicalize_block(
            unsealed_payload_handle.payload_id,
            unsealed_payload_handle.attributes_with_parent.clone(),
        ).await?;

        // Log the block building seal task duration, if metrics are enabled.
        kona_macros::set!(
            gauge,
            crate::Metrics::SEQUENCER_BLOCK_BUILDING_SEAL_TASK_DURATION,
            _seal_request_start.elapsed()
        );

        // If the conductor is available, commit the payload to it.
        if let Some(conductor) = &self.conductor {
            let _conductor_commitment_start = Instant::now();
            if let Err(err) = conductor.commit_unsafe_payload(&payload).await {
                error!(target: "sequencer", ?err, "Failed to commit unsafe payload to conductor");
            }

            kona_macros::set!(
                gauge,
                crate::Metrics::SEQUENCER_CONDUCTOR_COMMITMENT_DURATION,
                _conductor_commitment_start.elapsed()
            );
        }

        self.schedule_gossip(payload).await
    }

    async fn build_unsealed_payload(
        &mut self,
        unsafe_head_rx: &mut watch::Receiver<L2BlockInfo>,
        in_recovery_mode: bool,
    ) -> Result<Option<UnsealedPayloadHandle>, SequencerActorError> {
        let unsafe_head = *unsafe_head_rx.borrow();
        let l1_origin = match self
            .origin_selector
            .next_l1_origin(unsafe_head, self.is_recovery_mode)
            .await
        {
            Ok(l1_origin) => l1_origin,
            Err(err) => {
                warn!(
                    target: "sequencer",
                    ?err,
                    "Temporary error occurred while selecting next L1 origin. Re-attempting on next tick."
                );
                return Ok(None);
            }
        };

        if unsafe_head.l1_origin.hash != l1_origin.parent_hash &&
            unsafe_head.l1_origin.hash != l1_origin.hash
        {
            warn!(
                target: "sequencer",
                l1_origin = ?l1_origin,
                unsafe_head_hash = %unsafe_head.l1_origin.hash,
                unsafe_head_l1_origin = ?unsafe_head.l1_origin,
                "Cannot build new L2 block on inconsistent L1 origin, resetting engine"
            );
            if let Err(err) = self.reset_request_tx.send(()).await {
                error!(target: "sequencer", ?err, "Failed to reset engine");
                return Err(SequencerActorError::ChannelClosed);
            }
            return Ok(None);
        }

        info!(
            target: "sequencer",
            parent_num = unsafe_head.block_info.number,
            l1_origin_num = l1_origin.number,
            "Started sequencing new block"
        );

        // Build the payload attributes for the next block.
        let _attributes_build_start = Instant::now();

        let attrs_with_parent =
            match self.build_attributes(in_recovery_mode, unsafe_head, l1_origin).await? {
                Some(attrs) => attrs,
                None => {
                    // Temporary error or reset - retry on next tick.
                    return Ok(None);
                }
            };

        // Log the attributes build duration, if metrics are enabled.
        kona_macros::set!(
            gauge,
            crate::Metrics::SEQUENCER_ATTRIBUTES_BUILDER_DURATION,
            _attributes_build_start.elapsed()
        );

        // Send the built attributes to the engine to be built.
        let _build_request_start = Instant::now();

        let payload_id = self.block_engine.start_build_block(attrs_with_parent.clone()).await?;

        Ok(Some(UnsealedPayloadHandle::new(payload_id, attrs_with_parent)))
    }

    /// Builds the OpAttributesWithParent for the next block to build. If None is returned, it
    /// indicates that no attributes could be built at this time but future attempts may be made.
    async fn build_attributes(
        &mut self,
        in_recovery_mode: bool,
        unsafe_head: L2BlockInfo,
        l1_origin: BlockInfo,
    ) -> Result<Option<OpAttributesWithParent>, SequencerActorError> {
        let mut attributes = match self
            .attributes_builder
            .prepare_payload_attributes(unsafe_head, l1_origin.id())
            .await
        {
            Ok(attrs) => attrs,
            Err(PipelineErrorKind::Temporary(_)) => {
                // Temporary error - retry on next tick.
                return Ok(None);
            }
            Err(PipelineErrorKind::Reset(_)) => {
                if let Err(err) = self.reset_request_tx.send(()).await {
                    error!(target: "sequencer", ?err, "Failed to reset engine");
                    return Err(SequencerActorError::ChannelClosed);
                }

                warn!(
                    target: "sequencer",
                    "Resetting engine due to pipeline error while preparing payload attributes"
                );
                return Ok(None);
            }
            Err(err @ PipelineErrorKind::Critical(_)) => {
                error!(target: "sequencer", ?err, "Failed to prepare payload attributes");
                return Err(err.into());
            }
        };

        // Set the no_tx_pool flag to false by default (since we're building with the sequencer).
        attributes.no_tx_pool = Some(false);

        if in_recovery_mode {
            warn!(target: "sequencer", "Sequencer is in recovery mode, producing empty block");
            attributes.no_tx_pool = Some(true);
        }

        // If the next L2 block is beyond the sequencer drift threshold, we must produce an empty
        // block.
        if attributes.payload_attributes.timestamp >
            l1_origin.timestamp + self.cfg.max_sequencer_drift(l1_origin.timestamp)
        {
            attributes.no_tx_pool = Some(true);
        }

        // Do not include transactions in the first Ecotone block.
        if self.cfg.is_first_ecotone_block(attributes.payload_attributes.timestamp) {
            info!(target: "sequencer", "Sequencing ecotone upgrade block");
            attributes.no_tx_pool = Some(true);
        }

        // Do not include transactions in the first Fjord block.
        if self.cfg.is_first_fjord_block(attributes.payload_attributes.timestamp) {
            info!(target: "sequencer", "Sequencing fjord upgrade block");
            attributes.no_tx_pool = Some(true);
        }

        // Do not include transactions in the first Granite block.
        if self.cfg.is_first_granite_block(attributes.payload_attributes.timestamp) {
            info!(target: "sequencer", "Sequencing granite upgrade block");
            attributes.no_tx_pool = Some(true);
        }

        // Do not include transactions in the first Holocene block.
        if self.cfg.is_first_holocene_block(attributes.payload_attributes.timestamp) {
            info!(target: "sequencer", "Sequencing holocene upgrade block");
            attributes.no_tx_pool = Some(true);
        }

        // Do not include transactions in the first Isthmus block.
        if self.cfg.is_first_isthmus_block(attributes.payload_attributes.timestamp) {
            info!(target: "sequencer", "Sequencing isthmus upgrade block");
            attributes.no_tx_pool = Some(true);
        }

        // Do not include transactions in the first Jovian block.
        // See: `<https://github.com/ethereum-optimism/specs/blob/main/specs/protocol/jovian/derivation.md#activation-block-rules>`
        if self.cfg.is_first_jovian_block(attributes.payload_attributes.timestamp) {
            info!(target: "sequencer", "Sequencing jovian upgrade block");
            attributes.no_tx_pool = Some(true);
        }

        // Do not include transactions in the first Interop block.
        if self.cfg.is_first_interop_block(attributes.payload_attributes.timestamp) {
            info!(target: "sequencer", "Sequencing interop upgrade block");
            attributes.no_tx_pool = Some(true);
        }

        let attrs_with_parent = OpAttributesWithParent::new(attributes, unsafe_head, None, false);
        Ok(Some(attrs_with_parent))
    }

    /// Schedules a built [`OpExecutionPayloadEnvelope`] to be signed and gossipped.
    async fn schedule_gossip(
        &mut self,
        payload: OpExecutionPayloadEnvelope,
    ) -> Result<(), SequencerActorError> {
        // Send the payload to the P2P layer to be signed and gossipped.
        if let Err(err) = self.gossip_payload_tx.send(payload).await {
            error!(target: "sequencer", ?err, "Failed to send payload to be signed and gossipped");
            return Err(SequencerActorError::ChannelClosed);
        }

        Ok(())
    }

    /// Schedules the initial engine reset request and waits for the unsafe head to be updated.
    async fn schedule_initial_reset(
        &mut self,
        unsafe_head_rx: &mut watch::Receiver<L2BlockInfo>,
    ) -> Result<(), SequencerActorError> {
        // Schedule a reset of the engine, in order to initialize the engine state.
        if let Err(err) = self.reset_request_tx.send(()).await {
            error!(target: "sequencer", ?err, "Failed to send reset request to engine");
            return Err(SequencerActorError::ChannelClosed);
        }

        // Wait for the reset request to be processed before starting the block building loop.
        //
        // We know that the reset has concluded when the unsafe head watch channel is updated.
        if unsafe_head_rx.changed().await.is_err() {
            error!(target: "sequencer", "Failed to receive unsafe head update after reset request");
            return Err(SequencerActorError::ChannelClosed);
        }

        Ok(())
    }

    /// Updates the metrics for the sequencer actor.
    #[cfg(feature = "metrics")]
    fn update_metrics(&self) {
        let state_flags: [(&str, String); 2] = [
            ("active", self.is_active.to_string()),
            ("recovery", self.is_recovery_mode.to_string()),
        ];

        let gauge = metrics::gauge!(crate::Metrics::SEQUENCER_STATE, &state_flags);
        gauge.set(1);
    }

    async fn is_sequencer_active(&self) -> Result<bool, SequencerAdminAPIError> {
        Ok(self.is_active)
    }

    async fn is_conductor_enabled(&self) -> Result<bool, SequencerAdminAPIError> {
        Ok(self.conductor.is_some())
    }

    async fn start_sequencer(&mut self) -> Result<(), SequencerAdminAPIError> {
        if self.is_active {
            info!(target: "sequencer", "received request to start sequencer, but it is already started");
            return Ok(());
        }

        info!(target: "sequencer", "Starting sequencer");
        self.is_active = true;

        // Update metrics, if configured.
        #[cfg(feature = "metrics")]
        self.update_metrics();

        Ok(())
    }

    async fn stop_sequencer(&mut self) -> Result<B256, SequencerAdminAPIError> {
        info!(target: "sequencer", "Stopping sequencer");
        self.is_active = false;

        // Update metrics, if configured.
        #[cfg(feature = "metrics")]
        self.update_metrics();

        // TODO: THIS!
        //unsafe_head.borrow().hash()
        // TODO: remove this.
        Ok(B256::new([0; 32]))
    }

    async fn set_recovery_mode(&mut self, is_active: bool) -> Result<(), SequencerAdminAPIError> {
        self.is_recovery_mode = is_active;
        info!(target: "sequencer", is_active, "Updated recovery mode");

        // Update metrics, if configured.
        #[cfg(feature = "metrics")]
        self.update_metrics();

        Ok(())
    }

    async fn override_leader(&mut self) -> Result<(), SequencerAdminAPIError> {
        if let Some(conductor) = self.conductor.as_mut() {
            if let Err(e) = conductor.override_leader().await {
                error!(target: "sequencer::rpc", "Failed to override leader: {}", e);
                return Err(SequencerAdminAPIError::LeaderOverrideError);
            }
            info!(target: "sequencer", "Overrode leader via the conductor service");
        }

        // Update metrics, if configured.
        #[cfg(feature = "metrics")]
        self.update_metrics();

        Ok(())
    }
}

#[async_trait]
impl NodeActor for SequencerActor<SequencerBuilder> {
    type Error = SequencerActorError;
    type BuildData = SequencerInboundData;
    type InitData = SequencerInitContext;
    type StartData = SequencerActorState;
    type Builder = SequencerBuilder;

    fn build(config: Self::Builder) -> (Self::BuildData, Self) {
        let (unsafe_head_tx, unsafe_head_rx) = watch::channel(L2BlockInfo::default());

        // Create the admin API channel
        let (admin_api_tx, admin_api_rx) = mpsc::channel(1024);
        let admin_api = QueuedSequencerAdminAPIClient::new(admin_api_tx);

        (
            SequencerInboundData { unsafe_head_tx, admin_api_client: Box::new(admin_api) },
            Self::new(config, unsafe_head_rx, admin_api_rx),
        )
    }

    fn init(&self, ctx: Self::InitData) -> Self::StartData {
        let SequencerConfig {
            sequencer_stopped,
            sequencer_recovery_mode,
            conductor_rpc_url,
            l1_conf_delay,
        } = self.builder.seq_cfg.clone();

        let cfg = self.builder.rollup_cfg.clone();

        // Build the L1 provider with delay
        let l1_provider = DelayedL1OriginSelectorProvider::new(
            self.builder.l1_provider.clone(),
            ctx.l1_head_rx.clone(),
            l1_conf_delay,
        );

        // Build the origin selector
        let origin_selector = Box::new(L1OriginSelector::new(cfg.clone(), l1_provider))
            as Box<dyn OriginSelector + Sync>;

        // Build the attributes builder
        let attributes_builder =
            Box::new(self.builder.clone().build()) as Box<dyn AttributesBuilder + Sync>;

        // Build the conductor if configured
        let conductor = conductor_rpc_url
            .map(ConductorClient::new_http)
            .map(|c| Box::new(c) as Box<dyn Conductor + Sync>);

        // Create and return the state with all dependencies injected
        SequencerActorState::new(
            cfg,
            !sequencer_stopped,
            sequencer_recovery_mode,
            attributes_builder,
            conductor,
            origin_selector,
            ctx.block_engine,
            ctx.cancellation,
            ctx.reset_request_tx,
            ctx.gossip_payload_tx,
        )
    }

    async fn start(mut self, mut state: Self::StartData) -> Result<(), Self::Error> {
        let block_time = self.builder.rollup_cfg.block_time;
        let mut build_ticker = tokio::time::interval(Duration::from_secs(block_time));

        // Initialize metrics, if configured.
        #[cfg(feature = "metrics")]
        state.update_metrics();

        // Reset the engine state prior to beginning block building.
        state.schedule_initial_reset(&mut self.unsafe_head_rx).await?;

        let mut next_payload_to_seal: Option<UnsealedPayloadHandle> = None;
        let mut last_seconds_to_seal = 0u64;
        loop {
            select! {
                // We are using a biased select here to ensure that the admin queries are given priority over the block building task.
                // This is important to limit the occurrence of race conditions where a stopped query is received when a sequencer is building a new block.
                biased;
                _ = state.cancellation.cancelled() => {
                    info!(
                        target: "sequencer",
                        "Received shutdown signal. Exiting sequencer task."
                    );
                    return Ok(());
                }
                Some(query) = self.admin_api_rx.recv() => {
                    match query {
                        SequencerAdminQuery::SequencerActive(tx) => {
                            if tx.send(state.is_sequencer_active().await).is_err() {
                                warn!(target: "sequencer", "Failed to send response for is_sequencer_active query");
                            }
                        },
                        SequencerAdminQuery::StartSequencer(tx) => {
                            if tx.send(state.start_sequencer().await).is_err() {
                                warn!(target: "sequencer", "Failed to send response for start_sequencer query");
                            }
                            build_ticker.reset_immediately();
                        },
                        SequencerAdminQuery::StopSequencer(tx) => {
                            if tx.send(state.stop_sequencer().await).is_err() {
                                warn!(target: "sequencer", "Failed to send response for stop_sequencer query");
                            }
                        },
                        SequencerAdminQuery::ConductorEnabled(tx) => {
                            if tx.send(state.is_conductor_enabled().await).is_err() {
                                warn!(target: "sequencer", "Failed to send response for is_conductor_enabled query");
                            }
                        },
                        SequencerAdminQuery::SetRecoveryMode(is_active, tx) => {
                            if tx.send(state.set_recovery_mode(is_active).await).is_err() {
                                warn!(target: "sequencer", is_active = is_active, "Failed to send response for set_recovery_mode query");
                            }
                        },
                        SequencerAdminQuery::OverrideLeader(tx) => {
                            if tx.send(state.override_leader().await).is_err() {
                                warn!(target: "sequencer", "Failed to send response for override_leader query");
                            }
                        },
                    }
                }
                // The sequencer must be active to build new blocks.
                _ = build_ticker.tick(), if state.is_active => {

                    match state.seal_last_and_start_next(&mut self.unsafe_head_rx, state.is_recovery_mode, next_payload_to_seal.as_ref()).await {
                        Ok(res) => {
                            next_payload_to_seal = res.unsealed_payload_handle;
                            last_seconds_to_seal = res.seconds_to_seal;
                        },
                        Err(SequencerActorError::SealError(SealError::HoloceneRetry)) => {
                            next_payload_to_seal = None;
                        },
                        Err(SequencerActorError::SealError(SealError::ConsiderRebuild)) => {
                            next_payload_to_seal = None;
                        },
                        Err(SequencerActorError::SealError(SealError::EngineError)) => {
                            error!(target: "sequencer", "Critical engine error occurred");
                            state.cancellation.cancel();
                            return Err(SequencerActorError::SealError(SealError::EngineError));
                        },
                        Err(other_err) => {
                            error!(target: "sequencer", err = ?other_err, "Unexpected error building or sealing payload");
                            state.cancellation.cancel();
                            return Err(other_err);
                        }
                    }

                    if let Some(ref payload) = next_payload_to_seal {
                        let next_block_seconds = payload.attributes_with_parent.parent().block_info.timestamp.saturating_add(block_time);
                        // next block time is last + block_time - time it takes to seal.
                        let next_block_time = UNIX_EPOCH + Duration::from_secs(next_block_seconds) - Duration::from_secs(last_seconds_to_seal);
                        match next_block_time.duration_since(SystemTime::now()) {
                            Ok(duration) => build_ticker.reset_after(duration),
                            Err(_) => build_ticker.reset_immediately(),
                        };
                    } else {
                        build_ticker.reset_immediately();
                    }
                }
            }
        }
    }
}
