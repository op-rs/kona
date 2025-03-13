//! Common types for the Networking Crate.

mod enr;
pub use enr::OpStackEnr;

mod peer;
pub use peer::{Peer, PeerConversionError};
