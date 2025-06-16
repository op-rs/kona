#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod clap;
pub use clap::cli_styles;

#[cfg(feature = "secrets")]
mod secrets;
#[cfg(feature = "secrets")]
pub use secrets::{KeypairError, ParseKeyError, SecretKeyLoader};

pub mod backtrace;

pub mod log;

mod tracing;
pub use tracing::{init_test_tracing, init_tracing_subscriber};

mod prometheus;
pub use prometheus::init_prometheus_server;

pub mod sigsegv_handler;

pub mod metrics_args;
