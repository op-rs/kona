use alloy_primitives::ChainId;

/// A wrapper around `ChainId` that supports hex string (e.g. `"0x1"`) or numeric deserialization
/// for RPC inputs.
#[derive(Debug, Clone, Copy)]
pub struct HexChainId(pub ChainId);

impl serde::Serialize for HexChainId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        alloy_serde::quantity::serialize(&self.0, serializer)
    }
}

impl<'de> serde::Deserialize<'de> for HexChainId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = alloy_serde::quantity::deserialize(deserializer)?;
        Ok(Self(inner))
    }
}

impl From<HexChainId> for ChainId {
    fn from(value: HexChainId) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_from_hex_string() {
        let json = r#""0x1a""#;
        let parsed: HexChainId = serde_json::from_str(json).expect("should parse hex string");
        let chain_id: ChainId = parsed.into();
        assert_eq!(chain_id, 0x1a);
    }

    #[test]
    fn test_serialize_to_hex() {
        let value = HexChainId(26);
        let json = serde_json::to_string(&value).expect("should serialize");
        assert_eq!(json, r#""0x1a""#);
    }

    #[test]
    fn test_round_trip() {
        let original = HexChainId(12345);
        let json = serde_json::to_string(&original).unwrap();
        let parsed: HexChainId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.0, original.0);
    }
}
