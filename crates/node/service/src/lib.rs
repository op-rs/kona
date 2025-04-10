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
    NodeMode, RollupNode, RollupNodeBuilder, RollupNodeError, RollupNodeService,
    SequencerNodeService, ValidatorNodeService,
};

mod actors;
pub use actors::{
    DerivationActor, DerivationError, EngineActor, EngineError, EngineLauncher,
    InboundDerivationMessage, L1WatcherRpc, L1WatcherRpcError, NetworkActor, NetworkActorError,
    NodeActor, RpcActor, RpcActorError,
};

mod sync_start;
pub use sync_start::{L2ForkchoiceState, SyncStartError, find_starting_forkchoice};
