//! The [`SequencerActor`].

use super::{
    DelayedL1OriginSelectorProvider, L1OriginSelector, L1OriginSelectorError, SequencerConfig,
};
use crate::{CancellableContext, NodeActor, actors::sequencer::conductor::ConductorClient};
use alloy_provider::RootProvider;
use async_trait::async_trait;
use kona_derive::{AttributesBuilder, PipelineErrorKind, StatefulAttributesBuilder};
use kona_genesis::RollupConfig;
use kona_protocol::{BlockInfo, L2BlockInfo, OpAttributesWithParent};
use kona_providers_alloy::{AlloyChainProvider, AlloyL2ChainProvider};
use kona_rpc::SequencerAdminQuery;
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
    /// Watch channel to observe the unsafe head of the engine.
    pub unsafe_head_rx: watch::Receiver<L2BlockInfo>,
    /// Channel to receive admin queries from the sequencer actor.
    pub admin_query_rx: mpsc::Receiver<SequencerAdminQuery>,
}

/// The state of the [`SequencerActor`].
#[derive(Debug)]
pub(super) struct SequencerActorState<AB: AttributesBuilder> {
    /// The [`RollupConfig`] for the chain being sequenced.
    pub cfg: Arc<RollupConfig>,
    /// The [`AttributesBuilder`].
    pub builder: AB,
    /// The [`L1OriginSelector`].
    pub origin_selector: L1OriginSelector<DelayedL1OriginSelectorProvider>,
    /// The ticker for building new blocks.
    pub build_ticker: tokio::time::Interval,
    /// The conductor RPC client.
    pub conductor: Option<ConductorClient>,
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
}

/// A trait for building [`AttributesBuilder`]s.
pub trait AttributesBuilderConfig {
    /// The type of [`AttributesBuilder`] to build.
    type AB: AttributesBuilder;

    /// Builds the [`AttributesBuilder`].
    fn build(self) -> Self::AB;
}

impl SequencerActorState<StatefulAttributesBuilder<AlloyChainProvider, AlloyL2ChainProvider>> {
    fn new(
        seq_builder: SequencerBuilder,
        l1_head_watcher: watch::Receiver<Option<BlockInfo>>,
    ) -> Self {
        let SequencerConfig {
            sequencer_stopped,
            sequencer_recovery_mode,
            conductor_rpc_url,
            l1_conf_delay,
        } = seq_builder.seq_cfg.clone();

        let cfg = seq_builder.rollup_cfg.clone();
        let l1_provider = DelayedL1OriginSelectorProvider::new(
            seq_builder.l1_provider.clone(),
            l1_head_watcher,
            l1_conf_delay,
        );
        let conductor = conductor_rpc_url.map(ConductorClient::new_http);

        let builder = seq_builder.build();
        let build_ticker = tokio::time::interval(Duration::from_secs(cfg.block_time));

        let origin_selector = L1OriginSelector::new(cfg.clone(), l1_provider);

        Self {
            cfg,
            builder,
            origin_selector,
            build_ticker,
            conductor,
            is_active: !sequencer_stopped,
            is_recovery_mode: sequencer_recovery_mode,
        }
    }
}

const DERIVATION_PROVIDER_CACHE_SIZE: usize = 1024;

/// The builder for the [`SequencerActor`].
#[derive(Debug)]
pub struct SequencerBuilder {
    /// The [`SequencerConfig`].
    pub seq_cfg: SequencerConfig,
    /// The [`RollupConfig`] for the chain being sequenced.
    pub rollup_cfg: Arc<RollupConfig>,
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
    /// Channel to send admin queries to the sequencer actor.
    pub admin_query_tx: mpsc::Sender<SequencerAdminQuery>,
}

/// The communication context used by the [`SequencerActor`].
#[derive(Debug)]
pub struct SequencerContext {
    /// The cancellation token, shared between all tasks.
    pub cancellation: CancellationToken,
    /// Watch channel to observe the L1 head of the chain.
    pub l1_head_rx: watch::Receiver<Option<BlockInfo>>,
    /// Sender to request the engine to reset.
    pub reset_request_tx: mpsc::Sender<()>,
    /// Sender to request the execution layer to build a payload attributes on top of the
    /// current unsafe head.
    pub build_request_tx:
        mpsc::Sender<(OpAttributesWithParent, mpsc::Sender<OpExecutionPayloadEnvelope>)>,
    /// A sender to asynchronously sign and gossip built [`OpExecutionPayloadEnvelope`]s to the
    /// network actor.
    pub gossip_payload_tx: mpsc::Sender<OpExecutionPayloadEnvelope>,
}

impl CancellableContext for SequencerContext {
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
    /// An error occurred while selecting the next L1 origin.
    #[error(transparent)]
    L1OriginSelector(#[from] L1OriginSelectorError),
    /// A channel was unexpectedly closed.
    #[error("Channel closed unexpectedly")]
    ChannelClosed,
}

impl<AB: AttributesBuilderConfig> SequencerActor<AB> {
    /// Creates a new instance of the [`SequencerActor`].
    pub fn new(state: AB) -> (SequencerInboundData, Self) {
        let (unsafe_head_tx, unsafe_head_rx) = watch::channel(L2BlockInfo::default());
        let (admin_query_tx, admin_query_rx) = mpsc::channel(1024);
        let actor = Self { builder: state, unsafe_head_rx, admin_query_rx };

        (SequencerInboundData { unsafe_head_tx, admin_query_tx }, actor)
    }
}

impl<AB: AttributesBuilder> SequencerActorState<AB> {
    /// Starts the build job for the next L2 block, on top of the current unsafe head.
    async fn build_block(
        &mut self,
        ctx: &mut SequencerContext,
        unsafe_head_rx: &mut watch::Receiver<L2BlockInfo>,
        in_recovery_mode: bool,
    ) -> Result<(), SequencerActorError> {
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
                return Ok(());
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
            if let Err(err) = ctx.reset_request_tx.send(()).await {
                error!(target: "sequencer", ?err, "Failed to reset engine");
                ctx.cancellation.cancel();
                return Err(SequencerActorError::ChannelClosed);
            }
            return Ok(());
        }

        info!(
            target: "sequencer",
            parent_num = unsafe_head.block_info.number,
            l1_origin_num = l1_origin.number,
            "Started sequencing new block"
        );

        // Build the payload attributes for the next block.
        let _attributes_build_start = Instant::now();
        let mut attributes =
            match self.builder.prepare_payload_attributes(unsafe_head, l1_origin.id()).await {
                Ok(attrs) => attrs,
                Err(PipelineErrorKind::Temporary(_)) => {
                    return Ok(());
                    // Do nothing and allow a retry.
                }
                Err(PipelineErrorKind::Reset(_)) => {
                    if let Err(err) = ctx.reset_request_tx.send(()).await {
                        error!(target: "sequencer", ?err, "Failed to reset engine");
                        ctx.cancellation.cancel();
                        return Err(SequencerActorError::ChannelClosed);
                    }

                    warn!(
                        target: "sequencer",
                        "Resetting engine due to pipeline error while preparing payload attributes"
                    );
                    return Ok(());
                }
                Err(err @ PipelineErrorKind::Critical(_)) => {
                    error!(target: "sequencer", ?err, "Failed to prepare payload attributes");
                    ctx.cancellation.cancel();
                    return Err(err.into());
                }
            };

        if in_recovery_mode {
            attributes.no_tx_pool = Some(true);
        }

        // If the next L2 block is beyond the sequencer drift threshold, we must produce an empty
        // block.
        attributes.no_tx_pool = (attributes.payload_attributes.timestamp >
            l1_origin.timestamp + self.cfg.max_sequencer_drift(l1_origin.timestamp))
        .then_some(true);

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

        // Do not include transactions in the first Interop block.
        if self.cfg.is_first_interop_block(attributes.payload_attributes.timestamp) {
            info!(target: "sequencer", "Sequencing interop upgrade block");
            attributes.no_tx_pool = Some(true);
        }

        let attrs_with_parent = OpAttributesWithParent::new(attributes, unsafe_head, None, false);

        // Log the attributes build duration, if metrics are enabled.
        kona_macros::set!(
            gauge,
            crate::Metrics::SEQUENCER_ATTRIBUTES_BUILDER_DURATION,
            _attributes_build_start.elapsed()
        );

        // Create a new channel to receive the built payload.
        let (payload_tx, payload_rx) = mpsc::channel(1);

        // Send the built attributes to the engine to be built.
        let _build_request_start = Instant::now();
        if let Err(err) = ctx.build_request_tx.send((attrs_with_parent, payload_tx)).await {
            error!(target: "sequencer", ?err, "Failed to send built attributes to engine");
            ctx.cancellation.cancel();
            return Err(SequencerActorError::ChannelClosed);
        }

        let payload = self.try_wait_for_payload(ctx, payload_rx).await?;

        // Log the block building job duration, if metrics are enabled.
        kona_macros::set!(
            gauge,
            crate::Metrics::SEQUENCER_BLOCK_BUILDING_JOB_DURATION,
            _build_request_start.elapsed()
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

        let now =
            SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();
        let then = payload.execution_payload.timestamp() + self.cfg.block_time;
        if then.saturating_sub(now) <= self.cfg.block_time {
            warn!(
                target: "sequencer",
                "Next block timestamp is more than a block time away from now, building immediately"
            );
            self.build_ticker.reset_immediately();
        }

        self.schedule_gossip(ctx, payload).await
    }

    /// Waits for the next payload to be built and returns it, if there is a payload receiver
    /// present.
    async fn try_wait_for_payload(
        &mut self,
        ctx: &mut SequencerContext,
        mut payload_rx: mpsc::Receiver<OpExecutionPayloadEnvelope>,
    ) -> Result<OpExecutionPayloadEnvelope, SequencerActorError> {
        payload_rx.recv().await.ok_or_else(|| {
            error!(target: "sequencer", "Failed to receive built payload");
            ctx.cancellation.cancel();
            SequencerActorError::ChannelClosed
        })
    }

    /// Schedules a built [`OpExecutionPayloadEnvelope`] to be signed and gossipped.
    async fn schedule_gossip(
        &mut self,
        ctx: &mut SequencerContext,
        payload: OpExecutionPayloadEnvelope,
    ) -> Result<(), SequencerActorError> {
        // Send the payload to the P2P layer to be signed and gossipped.
        if let Err(err) = ctx.gossip_payload_tx.send(payload).await {
            error!(target: "sequencer", ?err, "Failed to send payload to be signed and gossipped");
            ctx.cancellation.cancel();
            return Err(SequencerActorError::ChannelClosed);
        }

        Ok(())
    }

    /// Schedules the initial engine reset request and waits for the unsafe head to be updated.
    async fn schedule_initial_reset(
        &mut self,
        ctx: &mut SequencerContext,
        unsafe_head_rx: &mut watch::Receiver<L2BlockInfo>,
    ) -> Result<(), SequencerActorError> {
        // Schedule a reset of the engine, in order to initialize the engine state.
        if let Err(err) = ctx.reset_request_tx.send(()).await {
            error!(target: "sequencer", ?err, "Failed to send reset request to engine");
            ctx.cancellation.cancel();
            return Err(SequencerActorError::ChannelClosed);
        }

        // Wait for the reset request to be processed before starting the block building loop.
        //
        // We know that the reset has concluded when the unsafe head watch channel is updated.
        if unsafe_head_rx.changed().await.is_err() {
            error!(target: "sequencer", "Failed to receive unsafe head update after reset request");
            ctx.cancellation.cancel();
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
}

#[async_trait]
impl NodeActor for SequencerActor<SequencerBuilder> {
    type Error = SequencerActorError;
    type OutboundData = SequencerContext;
    type Builder = SequencerBuilder;
    type InboundData = SequencerInboundData;

    fn build(config: Self::Builder) -> (Self::InboundData, Self) {
        Self::new(config)
    }

    async fn start(mut self, mut ctx: Self::OutboundData) -> Result<(), Self::Error> {
        let mut state = SequencerActorState::new(self.builder, ctx.l1_head_rx.clone());

        // Initialize metrics, if configured.
        #[cfg(feature = "metrics")]
        state.update_metrics();

        // Reset the engine state prior to beginning block building.
        state.schedule_initial_reset(&mut ctx, &mut self.unsafe_head_rx).await?;

        loop {
            select! {
                // We are using a biased select here to ensure that the admin queries are given priority over the block building task.
                // This is important to limit the occurrence of race conditions where a stopped query is received when a sequencer is building a new block.
                biased;
                _ = ctx.cancellation.cancelled() => {
                    info!(
                        target: "sequencer",
                        "Received shutdown signal. Exiting sequencer task."
                    );
                    return Ok(());
                }
                // Handle admin queries.
                Some(admin_query) = self.admin_query_rx.recv(), if !self.admin_query_rx.is_closed() => {
                    let is_sequencer_active = state.is_active;

                    if let Err(e) = state.handle_admin_query(admin_query, &mut self.unsafe_head_rx).await {
                        error!(target: "sequencer", err = ?e, "Failed to handle admin query");
                    }

                    // Reset the build ticker if the sequencer's activity state has changed.
                    if is_sequencer_active != state.is_active {
                        state.build_ticker.reset_immediately();
                    }

                    // Update metrics, if configured.
                    #[cfg(feature = "metrics")]
                    state.update_metrics();
                }
                // The sequencer must be active to build new blocks.
                _ = state.build_ticker.tick(), if state.is_active => {
                    state.build_block(&mut ctx, &mut self.unsafe_head_rx, state.is_recovery_mode).await?;
                }
            }
        }
    }
}
