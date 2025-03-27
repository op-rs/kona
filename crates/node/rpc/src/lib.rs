#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[cfg(feature = "std")]
mod config;
#[cfg(feature = "std")]
pub use config::RpcConfig;

mod net;
pub use net::{
    Connectedness, Direction, GossipScores, PeerDump, PeerInfo, PeerScores, PeerStats,
    ReqRespScores, TopicScores,
};

mod response;
pub use response::SafeHeadResponse;

mod superchain;
pub use superchain::{
    ProtocolVersion, ProtocolVersionError, ProtocolVersionFormatV0, SuperchainSignal,
};

mod output;
pub use output::OutputResponse;

mod attributes;
pub use attributes::OpAttributesWithParent;

#[cfg(feature = "jsonrpsee")]
mod jsonrpsee;
#[cfg(all(feature = "jsonrpsee", feature = "interop", feature = "client"))]
pub use jsonrpsee::SupervisorApiClient;
#[cfg(all(feature = "jsonrpsee", feature = "interop"))]
pub use jsonrpsee::SupervisorApiServer;
#[cfg(all(feature = "jsonrpsee", feature = "client"))]
pub use jsonrpsee::{
    EngineApiExtClient, MinerApiExtClient, OpAdminApiClient, OpP2PApiClient, RollupNodeApiClient,
};
#[cfg(feature = "jsonrpsee")]
pub use jsonrpsee::{
    EngineApiExtServer, MinerApiExtServer, OpAdminApiServer, OpP2PApiServer, RollupNodeApiServer,
};

#[cfg(all(feature = "reqwest", feature = "interop"))]
pub mod reqwest;
#[cfg(all(feature = "reqwest", feature = "interop", feature = "client"))]
pub use reqwest::SupervisorClient;

#[cfg(feature = "interop")]
mod interop;
#[cfg(feature = "interop")]
pub use interop::{CheckAccessList, InteropTxValidator, InteropTxValidatorError};
