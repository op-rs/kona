#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
extern crate tracing;

mod service;
pub use service::{InteropMode, NodeMode, RollupNode, RollupNodeBuilder, RollupNodeService};

mod actors;
pub use actors::{
    AttributesBuilderConfig, CancellableContext, ConductorClient, ConductorError,
    DelayedL1OriginSelectorProvider, DerivationActor, DerivationBuilder, DerivationContext,
    DerivationError, DerivationInboundChannels, DerivationState, EngineActor, EngineBuilder,
    EngineContext, EngineError, EngineInboundData, InboundDerivationMessage, L1OriginSelector,
    L1OriginSelectorError, L1OriginSelectorProvider, L1WatcherRpc, L1WatcherRpcContext,
    L1WatcherRpcError, L1WatcherRpcInboundChannels, L1WatcherRpcState, L2Finalizer, NetworkActor,
    NetworkActorError, NetworkBuilder, NetworkBuilderError, NetworkConfig, NetworkContext,
    NetworkDriver, NetworkDriverError, NetworkHandler, NetworkInboundData, NodeActor,
    PipelineBuilder, RpcActor, RpcActorError, RpcContext, SequencerActor, SequencerActorError,
    SequencerBuilder, SequencerConfig, SequencerContext, SequencerInboundData,
};

mod metrics;
pub use metrics::Metrics;
