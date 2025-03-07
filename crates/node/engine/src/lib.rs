#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
extern crate tracing;

mod actor;
pub use actor::{EngineActor, EngineActorError, EngineActorMessage, EngineEvent};

mod tasks;
pub use tasks::{
    EngineTask, ForkchoiceTask, ForkchoiceTaskError, ForkchoiceTaskExt, ForkchoiceTaskInput,
    ForkchoiceTaskOut, InsertTask, InsertTaskError, InsertTaskExt, InsertTaskInput, InsertTaskOut,
};

mod client;
pub use client::EngineClient;

mod versions;
pub use versions::{EngineForkchoiceVersion, EngineGetPayloadVersion, EngineNewPayloadVersion};

mod sync;
pub use sync::{SyncConfig, SyncMode, SyncStatus};

mod state;
pub use state::{EngineState, StateBuilder};

pub(crate) mod macros;
