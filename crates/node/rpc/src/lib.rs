#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod config;
pub use config::RpcConfig;

mod launcher;
pub use launcher::{RpcLauncher, RpcLauncherError};

mod net;
pub use net::NetworkRpc;

mod p2p;

mod response;
pub use response::SafeHeadResponse;

mod output;
pub use output::OutputResponse;

mod jsonrpsee;
#[cfg(feature = "client")]
pub use jsonrpsee::SupervisorApiClient;
#[cfg(feature = "client")]
pub use jsonrpsee::{MinerApiExtClient, OpAdminApiClient, OpP2PApiClient, RollupNodeApiClient};
pub use jsonrpsee::{
    MinerApiExtServer, OpAdminApiServer, OpP2PApiServer, RollupNodeApiServer, SupervisorApiServer,
};

#[cfg(feature = "reqwest")]
pub mod reqwest;
#[cfg(all(feature = "reqwest", feature = "client"))]
pub use reqwest::SupervisorClient;

mod interop;
pub use interop::{CheckAccessList, InteropTxValidator, InteropTxValidatorError};

mod rollup;
pub use rollup::RollupRpc;

mod l1_watcher;
pub use l1_watcher::{L1State, L1WatcherQueries, L1WatcherQuerySender};
