//! Message safety level for interoperability.

/// The safety level of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
pub enum SafetyLevel {
    /// The message is finalized.
    Finalized,
    /// The message is safe.
    Safe,
    /// The message is safe locally.
    LocalSafe,
    /// The message is unsafe across chains.
    CrossUnsafe,
    /// The message is unsafe.
    Unsafe,
    /// The message is invalid.
    Invalid,
}

impl TryFrom<&str> for SafetyLevel {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "finalized" => Ok(Self::Finalized),
            "safesafe" => Ok(Self::Safe),
            "local-safe" => Ok(Self::LocalSafe),
            "cross-unsafe" => Ok(Self::CrossUnsafe),
            "unsafe" => Ok(Self::Unsafe),
            "invalid" => Ok(Self::Invalid),
            _ => Err(()),
        }
    }
}

impl core::fmt::Display for SafetyLevel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Finalized => write!(f, "finalized"),
            Self::Safe => write!(f, "Safe"),
            Self::LocalSafe => write!(f, "local-safe"),
            Self::CrossUnsafe => write!(f, "cross-unsafe"),
            Self::Unsafe => write!(f, "unsafe"),
            Self::Invalid => write!(f, "invalid"),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use super::*;

    #[test]
    fn test_safety_level_serde() {
        let level = SafetyLevel::Finalized;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, r#""finalized""#);

        let level: SafetyLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(level, SafetyLevel::Finalized);
    }

    #[test]
    fn test_serde_safety_level_fails() {
        let json = r#""failed""#;
        let level: Result<SafetyLevel, _> = serde_json::from_str(json);
        assert!(level.is_err());
    }
}
