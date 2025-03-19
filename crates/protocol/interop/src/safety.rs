//! Message safety level for interoperability.
use derive_more::Display;
use core::str::FromStr;
use std::fmt;
/// The safety level of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
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

impl FromStr for SafetyLevel {
    type Err = SafetyLevelParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "finalized" => Ok(Self::Finalized),
            "safe" => Ok(Self::Safe),
            "local_safe" | "localsafe" => Ok(Self::LocalSafe),
            "cross_unsafe" | "crossunsafe" => Ok(Self::CrossUnsafe),
            "unsafe" => Ok(Self::Unsafe),
            "invalid" => Ok(Self::Invalid),
            _ => Err(SafetyLevelParseError),
        }
    }
}

/// Error when parsing SafetyLevel from string.
#[derive(Debug)]
pub struct SafetyLevelParseError;

impl fmt::Display for SafetyLevelParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid SafetyLevel")
    }
}

impl std::error::Error for SafetyLevelParseError {}

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
