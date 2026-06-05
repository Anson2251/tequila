use runtime::{Runtime, RuntimeManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub runtimes: Vec<Runtime>,
    pub default_id: String,
    /// Optional GitHub Personal Access Token used to authenticate API
    /// requests when fetching runtime/graphics release information.
    ///
    /// Without this token, GitHub's unauthenticated rate limit (~60 req/h)
    /// applies.  With a token the limit is raised to 5000 req/h, making
    /// the app significantly more reliable when fetching release lists or
    /// checking for updates.
    ///
    /// Stored in plaintext in the settings JSON file.  Users can generate
    /// a token at https://github.com/settings/tokens (no scopes needed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_api_key: Option<String>,
}

impl Settings {
    pub fn path() -> PathBuf {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        config_dir.join("tequila").join("settings.json")
    }

    pub fn load() -> Option<Self> {
        let path = Self::path();
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

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
        // Preserve any existing `github_api_key` from the on-disk settings,
        // so saving a `RuntimeManager` back to disk doesn't lose the key.
        let existing_key = Self::load().and_then(|s| s.github_api_key);
        Settings {
            runtimes: rm.runtimes,
            default_id: rm.default_id,
            github_api_key: existing_key,
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
