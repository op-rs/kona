#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use clap::Parser;

fn main() {
    kona_cli::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        // We accept the risk that another process may set RUST_BACKTRACE at the same time.
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    }

    if let Err(err) = kona_node::Cli::parse().run() {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}
