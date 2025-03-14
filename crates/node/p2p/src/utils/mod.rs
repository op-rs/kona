//! Utility methods for P2P.

mod secret_key;
pub use secret_key::{KeypairError, ParseKeyError, get_keypair, parse_key};
