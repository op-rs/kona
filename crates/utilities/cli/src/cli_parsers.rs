//! Parser functions for CLI arguments.

use alloy_primitives::{Bytes, hex};

/// Parse a string slice into [Bytes].
pub fn parse_bytes(s: &str) -> Result<Bytes, String> {
    hex::decode(s).map_err(|e| format!("Invalid hex string: {}", e)).map(Bytes::from)
}
