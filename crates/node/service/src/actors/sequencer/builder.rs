//! Builder for [`SequencerActor`].

use crate::{
    UnsafePayloadGossipClient,
    actors::{
        BlockBuildingClient,
        sequencer::{Conductor, OriginSelector, SequencerActor, SequencerAdminQuery},
    },
};
use kona_derive::AttributesBuilder;
use kona_genesis::RollupConfig;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Builder for constructing a [`SequencerActor`].
#[derive(Debug)]
pub struct SequencerActorBuilder<
    AttributesBuilder_,
    BlockBuildingClient_,
    Conductor_,
    OriginSelector_,
    UnsafePayloadGossipClient_,
> where
    AttributesBuilder_: AttributesBuilder,
    BlockBuildingClient_: BlockBuildingClient,
    Conductor_: Conductor,
    OriginSelector_: OriginSelector,
    UnsafePayloadGossipClient_: UnsafePayloadGossipClient,
{
    /// Receiver for admin API requests.
    pub admin_api_rx: mpsc::Receiver<SequencerAdminQuery>,
    /// The attributes builder used for block building.
    pub attributes_builder: AttributesBuilder_,
    /// The struct used to build blocks.
    pub block_building_client: BlockBuildingClient_,
    /// The cancellation token, shared between all tasks.
    pub cancellation_token: CancellationToken,
    /// The optional conductor RPC client.
    pub conductor: Option<Conductor_>,
    /// Whether the sequencer is active.
    pub is_active: bool,
    /// Whether the sequencer is in recovery mode.
    pub in_recovery_mode: bool,
    /// The struct used to determine the next L1 origin.
    pub origin_selector: OriginSelector_,
    /// The rollup configuration.
    pub rollup_config: Arc<RollupConfig>,
    /// A client to asynchronously sign and gossip built payloads to the network actor.
    pub unsafe_payload_gossip_client: UnsafePayloadGossipClient_,
}

impl<
    AttributesBuilder_,
    BlockBuildingClient_,
    Conductor_,
    OriginSelector_,
    UnsafePayloadGossipClient_,
>
    SequencerActorBuilder<
        AttributesBuilder_,
        BlockBuildingClient_,
        Conductor_,
        OriginSelector_,
        UnsafePayloadGossipClient_,
    >
where
    AttributesBuilder_: AttributesBuilder,
    BlockBuildingClient_: BlockBuildingClient,
    Conductor_: Conductor,
    OriginSelector_: OriginSelector,
    UnsafePayloadGossipClient_: UnsafePayloadGossipClient,
{
    /// Creates a new empty [`SequencerActorBuilder`].
    pub fn new(
        admin_api_rx: mpsc::Receiver<SequencerAdminQuery>,
        attributes_builder: AttributesBuilder_,
        block_building_client: BlockBuildingClient_,
        origin_selector: OriginSelector_,
        rollup_config: Arc<RollupConfig>,
        unsafe_payload_gossip_client: UnsafePayloadGossipClient_,
    ) -> Self {
        Self {
            admin_api_rx,
            attributes_builder,
            block_building_client,
            cancellation_token: CancellationToken::new(),
            conductor: None,
            is_active: false,
            in_recovery_mode: false,
            origin_selector,
            rollup_config,
            unsafe_payload_gossip_client,
        }
    }

    /// Sets whether the sequencer is active.
    pub const fn with_active_status(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
        self
    }

    /// Sets whether the sequencer is in recovery mode.
    pub const fn with_recovery_mode_status(mut self, is_recovery_mode: bool) -> Self {
        self.in_recovery_mode = is_recovery_mode;
        self
    }

    /// Sets the rollup configuration.
    pub fn with_rollup_config(mut self, rollup_config: Arc<RollupConfig>) -> Self {
        self.rollup_config = rollup_config;
        self
    }

    /// Sets the admin API receiver.
    pub fn with_admin_api_receiver(
        mut self,
        admin_api_rx: mpsc::Receiver<SequencerAdminQuery>,
    ) -> Self {
        self.admin_api_rx = admin_api_rx;
        self
    }

    /// Sets the attributes builder.
    pub fn with_attributes_builder(mut self, attributes_builder: AttributesBuilder_) -> Self {
        self.attributes_builder = attributes_builder;
        self
    }

    /// Sets the conductor.
    pub fn with_conductor(mut self, conductor: Conductor_) -> Self {
        self.conductor = Some(conductor);
        self
    }

    /// Sets the origin selector.
    pub fn with_origin_selector(mut self, origin_selector: OriginSelector_) -> Self {
        self.origin_selector = origin_selector;
        self
    }

    /// Sets the block engine.
    pub fn with_block_building_client(
        mut self,
        block_building_client: BlockBuildingClient_,
    ) -> Self {
        self.block_building_client = block_building_client;
        self
    }

    /// Sets the cancellation token.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = token;
        self
    }

    /// Sets the gossip payload sender.
    pub fn with_unsafe_payload_gossip_client(
        mut self,
        gossip_client: UnsafePayloadGossipClient_,
    ) -> Self {
        self.unsafe_payload_gossip_client = gossip_client;
        self
    }

    /// Builds the [`SequencerActor`].  
    ///  
    /// This builder cannot fail because all required fields are provided  
    /// through `new()`. Optional fields (like `conductor`) may be unset.
    pub fn build(
        self,
    ) -> SequencerActor<
        AttributesBuilder_,
        BlockBuildingClient_,
        Conductor_,
        OriginSelector_,
        UnsafePayloadGossipClient_,
    > {
        SequencerActor {
            admin_api_rx: self.admin_api_rx,
            attributes_builder: self.attributes_builder,
            block_building_client: self.block_building_client,
            cancellation_token: self.cancellation_token,
            conductor: self.conductor,
            is_active: self.is_active,
            in_recovery_mode: self.in_recovery_mode,
            origin_selector: self.origin_selector,
            rollup_config: self.rollup_config,
            unsafe_payload_gossip_client: self.unsafe_payload_gossip_client,
        }
    }
}
