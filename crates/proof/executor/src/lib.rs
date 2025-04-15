#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "test-utils"), no_std)]

extern crate alloc;

#[macro_use]
extern crate tracing;

mod db;
pub use db::{NoopTrieDBProvider, TrieDB, TrieDBProvider};

mod builder;
pub use builder::{BlockBuildingOutcome, StatelessL2Builder};

mod errors;
pub use errors::{ExecutorError, ExecutorResult, TrieDBError, TrieDBResult};

pub(crate) mod util;

pub(crate) mod constants;

#[cfg(feature = "test-utils")]
pub mod test_utils;
