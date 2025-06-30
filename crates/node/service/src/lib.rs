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
    AttributesBuilderConfig, CancellableContext, DerivationActor, DerivationBuilder,
    DerivationContext, DerivationError, DerivationInboundChannels, DerivationState, EngineActor,
    EngineBuilder, EngineContext, EngineError, EngineInboundData, InboundDerivationMessage,
    L1OriginSelector, L1OriginSelectorError, L1WatcherRpc, L1WatcherRpcContext, L1WatcherRpcError,
    L1WatcherRpcInboundChannels, L1WatcherRpcState, L2Finalizer, NetworkActor, NetworkActorError,
    NetworkContext, NetworkInboundData, NodeActor, PipelineBuilder, RpcActor, RpcActorError,
    RpcContext, RuntimeActor, RuntimeContext, RuntimeState, SequencerActor, SequencerActorError,
    SequencerBuilder, SequencerContext, SequencerInboundData, SupervisorActor,
    SupervisorActorContext, SupervisorActorError, SupervisorExt, SupervisorInboundData,
    SupervisorRpcServerExt,
};

mod metrics;
pub use metrics::Metrics;
