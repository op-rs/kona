#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[macro_use]
extern crate tracing;

mod admin;
pub use admin::{AdminRpc, NetworkAdminQuery, SequencerAdminQuery};

mod config;
pub use config::RpcBuilder;

mod net;
pub use net::P2pRpc;

mod p2p;

mod response;
pub use response::SafeHeadResponse;

mod output;
pub use output::OutputResponse;

mod dev;
pub use dev::DevEngineRpc;

mod jsonrpsee;
pub use jsonrpsee::{
    AdminApiServer, DevEngineApiServer, MinerApiExtServer, OpAdminApiServer, OpP2PApiServer,
    RollupNodeApiServer, WsServer,
};

mod rollup;
pub use rollup::RollupRpc;

mod l1_watcher;
pub use l1_watcher::{L1State, L1WatcherQueries, L1WatcherQuerySender};

mod ws;
pub use ws::WsRPC;

/// Key for the rollup boost health status.
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// | Execution Mode | Healthy                       | PartialContent                       | Unhealthy                     |
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// | Enabled        | - Request-path: L2 succeeds   | - Request-path: builder fails/stale  | - Request-path: L2 fails      |
/// |                |   (get/new payload) → 200     |   while L2 succeeds → 206            |   (error from L2) → 503       |
/// |                | - Background: builder         | - Background: builder fetch fails or | - Background: never sets 503  |
/// |                |   latest-unsafe is fresh →    |   latest-unsafe is stale → 206       |                               |
/// |                |   200                         |                                      |                               |
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// | DryRun         | - Request-path: L2 succeeds   | - Never set in DryRun                | - Request-path: L2 fails      |
/// |                |   (always returns L2) → 200   |   (degrade only in Enabled)          |   (error from L2) → 503       |
/// |                | - Background: builder stale   |                                      | - Background: never sets 503  |
/// |                |   ignored (remains 200)       |                                      |                               |
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+
/// | Disabled       | - Request-path: L2 succeeds   | - Never set in Disabled              | - Request-path: L2 fails      |
/// |                |   (builder skipped) → 200     |   (degrade only in Enabled)          |   (error from L2) → 503       |
/// |                | - Background: N/A             |                                      | - Background: never sets 503  |
/// +----------------+-------------------------------+--------------------------------------+-------------------------------+

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum RollupBoostHealth {
    /// Rollup boost is healthy.
    Healthy,
    /// Rollup boost is partially healthy.
    PartialContent,
    /// Rollup boost is unhealthy.
    Unhealthy,
}

/// A healthcheck response for the RPC server.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct HealthzResponse {
    /// The application version.
    pub version: String,
    /// Rollup boost health.
    pub rollup_boost_health: RollupBoostHealth,
}
