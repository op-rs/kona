#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "metrics"), no_std)]

extern crate alloc;

#[macro_use]
extern crate tracing;

/// Required types and traits for kona's derivation pipeline.
pub mod prelude {
    pub use crate::{
        attributes::*, errors::*, pipeline::*, sources::*, stages::*, traits::*, types::*,
    };
}

pub mod attributes;
pub mod errors;
pub mod metrics;
pub mod pipeline;
pub mod sources;
pub mod stages;
pub mod traits;
pub mod types;

#[cfg(feature = "test-utils")]
pub mod test_utils;
