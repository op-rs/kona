//! Protocol constants for the executor.

use alloy_primitives::{B256, b256};

/// The version byte for the Holocene extra data.
pub(crate) const HOLOCENE_EXTRA_DATA_VERSION: u8 = 0x00;

/// Empty SHA-256 hash.
pub(crate) const SHA256_EMPTY: B256 =
    b256!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
