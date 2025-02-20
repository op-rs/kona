#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(not(any(feature = "std", feature = "interop")), no_std)]

extern crate alloc;

mod graph;
pub use graph::MessageGraph;

mod traits;
pub use traits::InteropProvider;

#[cfg(feature = "interop")]
mod supervisor;
#[cfg(feature = "interop")]
pub use supervisor::{Supervisor, SupervisorClient, SupervisorError};

mod safety;
pub use safety::SafetyLevel;

mod errors;
pub use errors::{MessageGraphError, MessageGraphResult, SuperRootError, SuperRootResult};

mod root;
pub use root::{ChainRootInfo, OutputRootWithChain, SuperRoot, SuperRootResponse};

mod message;
pub use message::{
    extract_executing_messages, EnrichedExecutingMessage, ExecutingMessage, MessageIdentifier,
    RawMessagePayload,
};

mod derived;
pub use derived::DerivedIdPair;

mod constants;
pub use constants::{CROSS_L2_INBOX_ADDRESS, MESSAGE_EXPIRY_WINDOW, SUPER_ROOT_VERSION};

#[cfg(test)]
mod test_util;
