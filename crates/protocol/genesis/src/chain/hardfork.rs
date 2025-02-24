//! Contains the hardfork configuration for the chain.

/// Hardfork configuration.
///
/// See: <https://github.com/ethereum-optimism/superchain-registry/blob/8ff62ada16e14dd59d0fb94ffb47761c7fa96e01/ops/internal/config/chain.go#L102-L110>
#[derive(Debug, Copy, Clone, Default, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct HardForkConfig {
    /// Canyon hardfork activation time
    pub canyon_time: Option<u64>,
    /// Delta hardfork activation time
    pub delta_time: Option<u64>,
    /// Ecotone hardfork activation time
    pub ecotone_time: Option<u64>,
    /// Fjord hardfork activation time
    pub fjord_time: Option<u64>,
    /// Granite hardfork activation time
    pub granite_time: Option<u64>,
    /// Holocene hardfork activation time
    pub holocene_time: Option<u64>,
    /// Isthmus hardfork activation time
    pub isthmus_time: Option<u64>,
    /// Interop hardfork activation time
    pub interop_time: Option<u64>,
}

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use super::*;

    #[test]
    fn test_hardforks_deserialize_json() {
        let raw: &str = r#"
        {
            "canyon_time": 1699981200,
            "delta_time": 1703203200,
            "ecotone_time": 1708534800,
            "fjord_time": 1716998400,
            "granite_time": 1723478400,
            "holocene_time":1732633200
        }
        "#;

        let hardforks = HardForkConfig {
            canyon_time: Some(1699981200),
            delta_time: Some(1703203200),
            ecotone_time: Some(1708534800),
            fjord_time: Some(1716998400),
            granite_time: Some(1723478400),
            holocene_time: Some(1732633200),
            isthmus_time: None,
            interop_time: None,
        };

        let deserialized: HardForkConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(hardforks, deserialized);
    }

    #[test]
    fn test_hardforks_deserialize_new_field_fail_json() {
        let raw: &str = r#"
        {
            "canyon_time": 1704992401,
            "delta_time": 1708560000,
            "ecotone_time": 1710374401,
            "fjord_time": 1720627201,
            "granite_time": 1726070401,
            "holocene_time": 1736445601,
            "new_field": 0
        }
        "#;

        let err = serde_json::from_str::<HardForkConfig>(raw).unwrap_err();
        assert_eq!(err.classify(), serde_json::error::Category::Data);
    }

    #[test]
    fn test_hardforks_deserialize_toml() {
        let raw: &str = r#"
        canyon_time =  1699981200 # Tue 14 Nov 2023 17:00:00 UTC
        delta_time =   1703203200 # Fri 22 Dec 2023 00:00:00 UTC
        ecotone_time = 1708534800 # Wed 21 Feb 2024 17:00:00 UTC
        fjord_time =   1716998400 # Wed 29 May 2024 16:00:00 UTC
        granite_time = 1723478400 # Mon Aug 12 16:00:00 UTC 2024
        holocene_time = 1732633200 # Tue Nov 26 15:00:00 UTC 2024
        "#;

        let hardforks = HardForkConfig {
            canyon_time: Some(1699981200),
            delta_time: Some(1703203200),
            ecotone_time: Some(1708534800),
            fjord_time: Some(1716998400),
            granite_time: Some(1723478400),
            holocene_time: Some(1732633200),
            isthmus_time: None,
            interop_time: None,
        };

        let deserialized: HardForkConfig = toml::from_str(raw).unwrap();
        assert_eq!(hardforks, deserialized);
    }

    #[test]
    fn test_hardforks_deserialize_new_field_fail_toml() {
        let raw: &str = r#"
        canyon_time =  1699981200 # Tue 14 Nov 2023 17:00:00 UTC
        delta_time =   1703203200 # Fri 22 Dec 2023 00:00:00 UTC
        ecotone_time = 1708534800 # Wed 21 Feb 2024 17:00:00 UTC
        fjord_time =   1716998400 # Wed 29 May 2024 16:00:00 UTC
        granite_time = 1723478400 # Mon Aug 12 16:00:00 UTC 2024
        holocene_time = 1732633200 # Tue Nov 26 15:00:00 UTC 2024
        new_field_time = 1732633200 # Tue Nov 26 15:00:00 UTC 2024
        "#;
        toml::from_str::<HardForkConfig>(raw).unwrap_err();
    }
}
