//! Main entrypoint for the host binary.

#![warn(missing_debug_implementations, missing_docs, unreachable_pub, rustdoc::all)]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use crate::cli::{init_tracing_subscriber, HostCli, HostMode};
use anyhow::Result;
use clap::Parser;
use kona_host::DetachedHostOrchestrator;
use tracing::info;

pub mod cli;
pub mod eth;
pub mod interop;
pub mod single;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cfg = HostCli::parse();
    init_tracing_subscriber(cfg.v)?;

    match cfg.mode {
        HostMode::Single(cfg) => {
            cfg.run().await?;
        }
        HostMode::Super(cfg) => {
            cfg.run().await?;
        }
    }

    info!("Exiting host program.");
    Ok(())
}
