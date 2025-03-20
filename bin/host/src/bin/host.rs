//! Main entrypoint for the host binary.

#![warn(missing_debug_implementations, missing_docs, unreachable_pub, rustdoc::all)]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand};
use kona_cli::{cli_styles, init_tracing_subscriber};
use serde::Serialize;
use tracing::info;
use tracing_subscriber::EnvFilter;

const ABOUT: &str = "
kona-host is a CLI application that runs the Kona pre-image server and client program. The host
can run in two modes: server mode and native mode. In server mode, the host runs the pre-image
server and waits for the client program in the parent process to request pre-images. In native
mode, the host runs the client program in a separate thread with the pre-image server in the
primary thread.
";

/// The host binary CLI application arguments.
#[derive(Parser, Serialize, Clone, Debug)]
#[command(about = ABOUT, version, styles = cli_styles())]
pub struct HostCli {
    /// Verbosity level (0-2)
    #[arg(long, short, action = ArgAction::Count)]
    pub v: u8,
    /// Host mode
    #[command(subcommand)]
    pub mode: HostMode,
}

/// Operation modes for the host binary.
#[derive(Subcommand, Serialize, Clone, Debug)]
#[allow(clippy::large_enum_variant)]
pub enum HostMode {
    /// Run the host in single-chain mode.
    #[cfg(feature = "single")]
    Single(kona_host::single::SingleChainHost),
    /// Run the host in super-chain (interop) mode.
    #[cfg(feature = "interop")]
    Super(kona_host::interop::InteropHost),
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let cfg = HostCli::parse();
    init_tracing_subscriber(cfg.v, None::<EnvFilter>)?;

    match cfg.mode {
        #[cfg(feature = "single")]
        HostMode::Single(cfg) => {
            cfg.start().await?;
        }
        #[cfg(feature = "interop")]
        HostMode::Super(cfg) => {
            cfg.start().await?;
        }
    }

    info!("Exiting host program.");
    Ok(())
}
