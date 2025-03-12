#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod graph;
pub use graph::MessageGraph;

mod traits;
pub use traits::InteropProvider;

mod safety;
pub use safety::SafetyLevel;

mod errors;
pub use errors::{
    InvalidInboxEntry, MessageGraphError, MessageGraphResult, SuperRootError, SuperRootResult,
};

mod root;
pub use root::{ChainRootInfo, OutputRootWithChain, SuperRoot, SuperRootResponse};

mod message;
pub use message::{
    EnrichedExecutingMessage, ExecutingDescriptor, ExecutingMessage, MessageIdentifier,
    RawMessagePayload, extract_executing_messages, parse_log_to_executing_message,
    parse_logs_to_executing_msgs,
};

mod access_list;
pub use access_list::{
    parse_access_list_item_to_inbox_entries, parse_access_list_items_to_inbox_entries,
};
mod derived;
pub use derived::DerivedIdPair;

mod constants;
pub use constants::{CROSS_L2_INBOX_ADDRESS, MESSAGE_EXPIRY_WINDOW, SUPER_ROOT_VERSION};

#[cfg(test)]
mod test_util;
