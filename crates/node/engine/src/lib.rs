#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

extern crate alloc;

pub mod client;
pub use client::EngineClient;

mod versions;
pub use versions::{EngineForkchoiceVersion, EngineGetPayloadVersion, EngineNewPayloadVersion};

mod sync;
pub use sync::{SyncConfig, SyncMode, SyncStatus};

pub mod controller;
pub use controller::EngineController;

pub mod controller_builder;
pub use controller_builder::ControllerBuilder;

pub mod error;
pub use error::EngineUpdateError;

pub mod state;
pub use state::EngineState;

pub mod state_builder;
pub use state_builder::StateBuilder;
