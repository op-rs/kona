//! Builder for [`SequencerActor`].

use crate::actors::{
    BlockBuildingClient,
    sequencer::{Conductor, OriginSelector, SequencerActor, SequencerAdminQuery},
};
use kona_derive::AttributesBuilder;
use kona_genesis::RollupConfig;
use op_alloy_rpc_types_engine::OpExecutionPayloadEnvelope;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Builder for constructing a [`SequencerActor`].
#[derive(Debug, Default)]
pub struct SequencerActorBuilder<AB, C, OS, BB>
where
    AB: AttributesBuilder,
    C: Conductor,
    OS: OriginSelector,
    BB: BlockBuildingClient,
{
    /// Receiver for admin API requests.
    pub admin_api_rx: Option<mpsc::Receiver<SequencerAdminQuery>>,
    /// The attributes builder used for block building.
    pub attributes_builder: Option<AB>,
    /// The struct used to build blocks.
    pub block_building_client: Option<BB>,
    /// The cancellation token, shared between all tasks.
    pub cancellation_token: Option<CancellationToken>,
    /// The optional conductor RPC client.
    pub conductor: Option<C>,
    /// A sender to asynchronously sign and gossip built payloads to the network actor.
    pub gossip_payload_tx: Option<mpsc::Sender<OpExecutionPayloadEnvelope>>,
    /// Whether the sequencer is active.
    pub is_active: Option<bool>,
    /// Whether the sequencer is in recovery mode.
    pub in_recovery_mode: Option<bool>,
    /// The struct used to determine the next L1 origin.
    pub origin_selector: Option<OS>,
    /// The rollup configuration.
    pub rollup_config: Option<Arc<RollupConfig>>,
}

impl<AB, C, OS, BB> SequencerActorBuilder<AB, C, OS, BB>
where
    AB: AttributesBuilder,
    C: Conductor,
    OS: OriginSelector,
    BB: BlockBuildingClient,
{
    /// Creates a new empty [`SequencerActorBuilder`].
    pub const fn new() -> Self {
        Self {
            admin_api_rx: None,
            attributes_builder: None,
            block_building_client: None,
            cancellation_token: None,
            conductor: None,
            gossip_payload_tx: None,
            is_active: None,
            in_recovery_mode: None,
            origin_selector: None,
            rollup_config: None,
        }
    }

    /// Sets whether the sequencer is active.
    pub const fn with_active_status(mut self, is_active: bool) -> Self {
        self.is_active = Some(is_active);
        self
    }

    /// Sets whether the sequencer is in recovery mode.
    pub const fn with_recovery_mode_status(mut self, is_recovery_mode: bool) -> Self {
        self.in_recovery_mode = Some(is_recovery_mode);
        self
    }

    /// Sets the rollup configuration.
    pub fn with_rollup_config(mut self, rollup_config: Arc<RollupConfig>) -> Self {
        self.rollup_config = Some(rollup_config);
        self
    }

    /// Sets the admin API receiver.
    pub fn with_admin_api_receiver(
        mut self,
        admin_api_rx: mpsc::Receiver<SequencerAdminQuery>,
    ) -> Self {
        self.admin_api_rx = Some(admin_api_rx);
        self
    }

    /// Sets the attributes builder.
    pub fn with_attributes_builder(mut self, attributes_builder: AB) -> Self {
        self.attributes_builder = Some(attributes_builder);
        self
    }

    /// Sets the conductor.
    pub fn with_conductor(mut self, conductor: C) -> Self {
        self.conductor = Some(conductor);
        self
    }

    /// Sets the origin selector.
    pub fn with_origin_selector(mut self, origin_selector: OS) -> Self {
        self.origin_selector = Some(origin_selector);
        self
    }

    /// Sets the block engine.
    pub fn with_block_building_client(mut self, block_building_client: BB) -> Self {
        self.block_building_client = Some(block_building_client);
        self
    }

    /// Sets the cancellation token.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = Some(token);
        self
    }

    /// Sets the gossip payload sender.
    pub fn with_gossip_payload_sender(
        mut self,
        gossip_payload_tx: mpsc::Sender<OpExecutionPayloadEnvelope>,
    ) -> Self {
        self.gossip_payload_tx = Some(gossip_payload_tx);
        self
    }

    /// Builds the [`SequencerActor`].
    ///
    /// # Panics
    ///
    /// Panics if any required field is not set.
    pub fn build(self) -> Result<SequencerActor<AB, C, OS, BB>, String> {
        Ok(SequencerActor {
            admin_api_rx: self.admin_api_rx.expect("admin_api_rx is required"),
            attributes_builder: self.attributes_builder.expect("attributes_builder is required"),
            block_building_client: self
                .block_building_client
                .expect("block_building_client is required"),
            cancellation_token: self.cancellation_token.expect("cancellation is required"),
            conductor: self.conductor,
            gossip_payload_tx: self.gossip_payload_tx.expect("gossip_payload_tx is required"),
            is_active: self.is_active.expect("initial active status not set"),
            in_recovery_mode: self.in_recovery_mode.expect("initial recovery mode status not set"),
            origin_selector: self.origin_selector.expect("origin_selector is required"),
            rollup_config: self.rollup_config.expect("rollup_config is required"),
        })
    }
}
