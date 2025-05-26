use op_alloy_consensus::interop::SafetyLevel;
use reth_db::DatabaseError;
use reth_db_api::table::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// Key representing a particular head reference type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HeadRefKey {
    /// Latest unverified or unsafe head.
    Unsafe,

    /// Head block considered safe via local verification.
    LocalSafe,

    /// Head block considered unsafe via cross-chain sync.
    CrossUnsafe,

    /// Head block considered safe.
    Safe,

    /// Finalized head block.
    Finalized,

    /// Invalid head reference.
    Invalid,
}

/// Implementation of [`Encode`] for [`HeadRefKey`].
impl Encode for HeadRefKey {
    type Encoded = [u8; 1];

    fn encode(self) -> Self::Encoded {
        match self {
            Self::Unsafe => [0],
            Self::LocalSafe => [1],
            Self::CrossUnsafe => [2],
            Self::Safe => [3],
            Self::Finalized => [4],
            Self::Invalid => [255],
        }
    }
}

/// Implementation of [`Decode`] for [`HeadRefKey`].
impl Decode for HeadRefKey {
    fn decode(value: &[u8]) -> Result<Self, DatabaseError> {
        match value {
            [0] => Ok(Self::Unsafe),
            [1] => Ok(Self::LocalSafe),
            [2] => Ok(Self::CrossUnsafe),
            [3] => Ok(Self::Safe),
            [4] => Ok(Self::Finalized),
            [255] => Ok(Self::Invalid),
            _ => Err(DatabaseError::Decode),
        }
    }
}

/// Converts from [`HeadRefKey`] (internal storage reference) to [`SafetyLevel`] (public API format).
///
/// Performs a lossless and direct mapping from head reference level to safety level.
impl From<HeadRefKey> for SafetyLevel {
    fn from(key: HeadRefKey) -> Self {
        match key {
            HeadRefKey::Unsafe => {SafetyLevel::Unsafe}
            HeadRefKey::LocalSafe => {SafetyLevel::LocalSafe}
            HeadRefKey::CrossUnsafe => {SafetyLevel::CrossUnsafe}
            HeadRefKey::Safe => {SafetyLevel::Safe}
            HeadRefKey::Finalized => {SafetyLevel::Finalized}
            HeadRefKey::Invalid => {SafetyLevel::Invalid}
        }
    }
}

/// Converts from [`SafetyLevel`] (public API format) to [`HeadRefKey`] (internal storage reference).
///
/// Performs a direct  mapping from safety level to head reference key.
impl From<SafetyLevel> for HeadRefKey {
    fn from(key: SafetyLevel) -> Self {
        match key {
            SafetyLevel::Unsafe => {HeadRefKey::Unsafe}
            SafetyLevel::LocalSafe => {HeadRefKey::LocalSafe}
            SafetyLevel::CrossUnsafe => {HeadRefKey::CrossUnsafe}
            SafetyLevel::Safe => {HeadRefKey::Safe}
            SafetyLevel::Finalized => {HeadRefKey::Finalized}
            SafetyLevel::Invalid => {HeadRefKey::Invalid}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_head_ref_key_encode_decode() {
        let cases = vec![
            (HeadRefKey::Unsafe, [0]),
            (HeadRefKey::LocalSafe, [1]),
            (HeadRefKey::CrossUnsafe, [2]),
            (HeadRefKey::Safe, [3]),
            (HeadRefKey::Finalized, [4]),
            (HeadRefKey::Invalid, [255]),
        ];

        for (key, expected_encoding) in &cases {
            // Test encoding
            let encoded = key.clone().encode();
            assert_eq!(encoded, *expected_encoding, "Encoding failed for {:?}", key);

            // Test decoding
            let decoded = HeadRefKey::decode(&encoded).expect("Decoding should succeed");
            assert_eq!(decoded, *key, "Decoding mismatch for {:?}", key);
        }
    }
    #[test]
    fn test_round_trip_conversion() {
        for level in [
            SafetyLevel::Unsafe,
            SafetyLevel::LocalSafe,
            SafetyLevel::CrossUnsafe,
            SafetyLevel::Safe,
            SafetyLevel::Finalized,
            SafetyLevel::Invalid,
        ] {
            let round_trip = SafetyLevel::from(HeadRefKey::from(level));
            assert_eq!(round_trip, level, "Round-trip failed for {:?}", level);
        }

        for key in [
            HeadRefKey::Unsafe,
            HeadRefKey::LocalSafe,
            HeadRefKey::CrossUnsafe,
            HeadRefKey::Safe,
            HeadRefKey::Finalized,
            HeadRefKey::Invalid,
        ] {
            let round_trip = HeadRefKey::from(SafetyLevel::from(key));
            assert_eq!(round_trip, key, "Round-trip failed for {:?}", key);
        }
    }
}