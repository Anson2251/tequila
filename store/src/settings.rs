use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use runtime::{Runtime, RuntimeManager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub runtimes: Vec<Runtime>,
    pub default_id: String,
}

impl Settings {
    pub fn path() -> PathBuf {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        config_dir.join("tequila").join("settings.json")
    }

    pub fn load() -> Option<Self> {
        let path = Self::path();
        if !path.exists() { return None; }
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)
    }
}

impl From<RuntimeManager> for Settings {
    fn from(rm: RuntimeManager) -> Self {
        Settings { runtimes: rm.runtimes, default_id: rm.default_id }
    }
}

impl From<Settings> for RuntimeManager {
    fn from(s: Settings) -> Self {
        RuntimeManager { runtimes: s.runtimes, default_id: s.default_id }
    }
}
