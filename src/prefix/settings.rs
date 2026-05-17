use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::prefix::runtime::{Runtime, RuntimeManager};

/// Global Tequila settings stored at `$XDG_CONFIG_HOME/tequila/settings.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub runtimes: Vec<Runtime>,
    pub default_id: String,
}

impl Settings {
    /// Path to the settings file.
    pub fn path() -> PathBuf {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        config_dir.join("tequila").join("settings.json")
    }

    /// Load settings from disk. Returns None if the file doesn't exist.
    pub fn load() -> Option<Self> {
        let path = Self::path();
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save settings to disk, creating parent directories as needed.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)
    }
}

impl From<RuntimeManager> for Settings {
    fn from(rm: RuntimeManager) -> Self {
        Settings {
            runtimes: rm.runtimes,
            default_id: rm.default_id,
        }
    }
}

impl From<Settings> for RuntimeManager {
    fn from(s: Settings) -> Self {
        RuntimeManager {
            runtimes: s.runtimes,
            default_id: s.default_id,
        }
    }
}
