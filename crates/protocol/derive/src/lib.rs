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

mod attributes;
pub use attributes::StatefulAttributesBuilder;

mod errors;
pub use errors::{
    BatchDecompressionError, BlobDecodingError, BlobProviderError, BuilderError,
    PipelineEncodingError, PipelineError, PipelineErrorKind, ResetError,
};

pub mod pipeline;
pub mod sources;
pub mod stages;
pub mod traits;
pub mod types;

pub mod metrics;
pub use metrics::Metrics;

#[cfg(feature = "test-utils")]
pub mod test_utils;
