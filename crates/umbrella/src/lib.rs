#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/square.png",
    html_favicon_url = "https://raw.githubusercontent.com/op-rs/kona/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/op-rs/kona/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "rpc")]
#[doc(inline)]
pub use kona_rpc as rpc;

#[cfg(feature = "mpt")]
#[doc(inline)]
pub use kona_mpt as mpt;

#[cfg(feature = "proof2")]
#[doc(inline)]
pub use kona_proof as proof;

#[cfg(feature = "executor")]
#[doc(inline)]
pub use kona_executor as executor;

#[cfg(feature = "preimage")]
#[doc(inline)]
pub use kona_preimage as preimage;

#[cfg(feature = "proof-interop")]
#[doc(inline)]
pub use kona_proof_interop as proof_interop;

#[cfg(feature = "derive")]
#[doc(inline)]
pub use kona_derive as derive;

#[cfg(feature = "driver")]
#[doc(inline)]
pub use kona_driver as driver;

#[cfg(feature = "registry")]
#[doc(inline)]
pub use kona_registry as registry;

#[cfg(feature = "genesis")]
#[doc(inline)]
pub use kona_genesis as genesis;

#[cfg(feature = "interop")]
#[doc(inline)]
pub use kona_interop as interop;

#[cfg(feature = "protocol2")]
#[doc(inline)]
pub use kona_protocol as protocol;

#[cfg(feature = "net")]
#[doc(inline)]
pub use kona_net as net;

#[cfg(feature = "alloy")]
#[doc(inline)]
pub use kona_providers_alloy as alloy;

#[cfg(feature = "local")]
#[doc(inline)]
pub use kona_providers_local as local;

#[cfg(feature = "std-fpvm")]
#[doc(inline)]
pub use kona_std_fpvm as std_fpvm;
