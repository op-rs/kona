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
pub use service::{
    InteropMode, NodeMode, RollupNode, RollupNodeBuilder, RollupNodeError, RollupNodeService,
};

mod actors;
pub use actors::{
    CancellableContext, DerivationActor, DerivationContext, DerivationError,
    DerivationOutboundChannels, DerivationState, EngineActor, EngineActorState, EngineContext,
    EngineError, EngineLauncher, EngineOutboundData, InboundDerivationMessage,
    InboundEngineMessage, L1WatcherRpc, L1WatcherRpcContext, L1WatcherRpcError,
    L1WatcherRpcOutboundChannels, L1WatcherRpcState, L2Finalizer, NetworkActor, NetworkActorError,
    NetworkContext, NetworkOutboundData, NodeActor, RpcActor, RpcActorError, RpcContext,
    RuntimeActor, RuntimeContext, RuntimeOutboundData, RuntimeState, SupervisorActor,
    SupervisorActorContext, SupervisorActorError, SupervisorExt, SupervisorOutboundData,
    SupervisorRpcServerExt,
};

mod metrics;
pub use metrics::Metrics;
