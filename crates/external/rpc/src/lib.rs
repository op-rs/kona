#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

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

mod sync;
pub use sync::{L2BlockRef, SyncStatus};

#[cfg(feature = "jsonrpsee")]
mod api;
#[cfg(all(feature = "jsonrpsee", feature = "client"))]
pub use api::{
    EngineApiExtClient, MinerApiExtClient, OpAdminApiClient, OpP2PApiClient, RollupNodeClient,
    SupervisorApiClient,
};
#[cfg(feature = "jsonrpsee")]
pub use api::{
    EngineApiExtServer, MinerApiExtServer, OpAdminApiServer, OpP2PApiServer, RollupNodeServer,
    SupervisorApiServer,
};

#[cfg(feature = "interop")]
mod traits;
#[cfg(feature = "interop")]
pub use traits::{ExecutingMessageValidator, ExecutingMessageValidatorError};
