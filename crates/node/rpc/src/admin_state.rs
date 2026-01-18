//! Admin State Persistence
//!
//! This module provides functionality to persist admin API state changes
//! so they survive node restarts.

use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
};

/// The persisted admin state.
///
/// This struct represents the state that can be modified via the admin API
/// and should persist across node restarts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdminState {
    /// Whether the sequencer is active (started/stopped).
    #[serde(default)]
    pub sequencer_active: bool,
    /// Whether the sequencer is in recovery mode.
    #[serde(default)]
    pub recovery_mode: bool,
}

impl AdminState {
    /// Creates a new [`AdminState`] with the given values.
    pub const fn new(sequencer_active: bool, recovery_mode: bool) -> Self {
        Self { sequencer_active, recovery_mode }
    }
}

/// Error type for admin state persistence operations.
#[derive(Debug, thiserror::Error)]
pub enum AdminStatePersistenceError {
    /// IO error while reading or writing the state file.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Loads the admin state from the given file path.
///
/// If the file does not exist, returns `None`.
/// If the file exists but cannot be parsed, returns an error.
pub fn load_admin_state(path: &Path) -> Result<Option<AdminState>, AdminStatePersistenceError> {
    if !path.exists() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(path)?;
    let state: AdminState = serde_json::from_str(&contents)?;
    Ok(Some(state))
}

/// Saves the admin state to the given file path.
///
/// Creates parent directories if they don't exist.
pub fn save_admin_state(path: &Path, state: &AdminState) -> Result<(), AdminStatePersistenceError> {
    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let contents = serde_json::to_string_pretty(state)?;
    std::fs::write(path, contents)?;
    Ok(())
}

/// A helper struct that wraps an optional persistence path and provides
/// convenient methods for loading and saving admin state.
#[derive(Debug, Clone, Default)]
pub struct AdminStatePersistence {
    /// The path to the admin state file, if persistence is enabled.
    path: Option<PathBuf>,
}

impl AdminStatePersistence {
    /// Creates a new [`AdminStatePersistence`] with the given path.
    pub const fn new(path: Option<PathBuf>) -> Self {
        Self { path }
    }

    /// Returns whether persistence is enabled.
    pub const fn is_enabled(&self) -> bool {
        self.path.is_some()
    }

    /// Loads the admin state from the configured path.
    ///
    /// Returns `None` if persistence is disabled or the file doesn't exist.
    pub fn load(&self) -> Result<Option<AdminState>, AdminStatePersistenceError> {
        match &self.path {
            Some(path) => load_admin_state(path),
            None => Ok(None),
        }
    }

    /// Saves the admin state to the configured path.
    ///
    /// Does nothing if persistence is disabled.
    pub fn save(&self, state: &AdminState) -> Result<(), AdminStatePersistenceError> {
        if let Some(path) = &self.path {
            save_admin_state(path, state)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_admin_state_default() {
        let state = AdminState::default();
        assert!(!state.sequencer_active);
        assert!(!state.recovery_mode);
    }

    #[test]
    fn test_admin_state_serialization() {
        let state = AdminState::new(true, false);
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AdminState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_admin_state(Path::new("/nonexistent/path/state.json"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_save_and_load_admin_state() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("admin_state.json");

        let state = AdminState::new(true, true);
        save_admin_state(&path, &state).unwrap();

        let loaded = load_admin_state(&path).unwrap();
        assert_eq!(loaded, Some(state));
    }

    #[test]
    fn test_save_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("state.json");

        let state = AdminState::new(false, true);
        save_admin_state(&path, &state).unwrap();

        assert!(path.exists());
        let loaded = load_admin_state(&path).unwrap();
        assert_eq!(loaded, Some(state));
    }

    #[test]
    fn test_persistence_disabled() {
        let persistence = AdminStatePersistence::new(None);
        assert!(!persistence.is_enabled());
        assert!(persistence.load().unwrap().is_none());

        // Save should succeed (no-op) when disabled
        let state = AdminState::new(true, true);
        assert!(persistence.save(&state).is_ok());
    }

    #[test]
    fn test_persistence_enabled() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("admin_state.json");
        let persistence = AdminStatePersistence::new(Some(path));

        assert!(persistence.is_enabled());

        let state = AdminState::new(true, false);
        persistence.save(&state).unwrap();

        let loaded = persistence.load().unwrap();
        assert_eq!(loaded, Some(state));
    }
}
