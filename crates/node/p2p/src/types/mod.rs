//! Common types for the Networking Crate.

mod enr;
pub use enr::{OP_CL_KEY, OpStackEnr};

mod peer;
pub use peer::{Peer, PeerConversionError};
