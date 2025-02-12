#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(not(any(test, feature = "arbitrary")), no_std)]

extern crate alloc;

mod graph;
pub use graph::MessageGraph;

mod message;
pub use message::{
    extract_executing_messages, EnrichedExecutingMessage, ExecutingMessage, MessageIdentifier,
    RawMessagePayload,
};

mod constants;
pub use constants::{CROSS_L2_INBOX_ADDRESS, MESSAGE_EXPIRY_WINDOW, SUPER_ROOT_VERSION};

mod traits;
pub use traits::InteropProvider;

mod errors;
pub use errors::{MessageGraphError, MessageGraphResult, SuperRootError, SuperRootResult};

mod super_root;
pub use super_root::{OutputRootWithChain, SuperRoot};

#[cfg(test)]
mod test_util;
