mod app_ops;
pub mod desktop;
mod launch_ops;
mod manager;
pub mod prefix;
mod prefix_ops;
mod process_tracker;
mod runtime_ops;
mod wine_processes;

pub use manager::Manager;
pub use prefix::Prefix;
pub use prefix::prefix_label;
pub use prefix::resolve_or_extract_icon;
pub use prefix_ops::TQL_EXTENSION;
pub use process_tracker::ProcessTracker;
pub use wine_processes::apply_runtime_env;

// Re-exports from sub-crates for UI convenience
pub use base::config;
pub use base::{
    self, GraphicsBackend, GraphicsConfig, PrefixConfig, PrefixError, PrefixInfo,
    RegisteredExecutable, WinePrefix,
};
pub use registry;
pub use registry::keys;
pub use registry::{RegEditor, RegistryEditor, WineRegistry};
pub use runtime;
pub use runtime::download;
pub use runtime::{Runtime, RuntimeManager, RuntimeSource};
pub use scan::{ApplicationScanner, IconCache};
pub use store::{PrefixStore, Settings};

// ── GitHub API client ────────────────────────────────────────────────

use std::sync::Arc;

/// Return a [`GitHubClient`] initialised from the current settings.
///
/// A new client is created on every call so that API-key changes are
/// picked up immediately.  The struct is lightweight (`Option<String>`),
/// so the allocation overhead is negligible.
pub fn github_client() -> Arc<runtime::github::GitHubClient> {
    let api_key = store::Settings::load().and_then(|s| s.github_api_key);
    Arc::new(runtime::github::GitHubClient::new(api_key))
}
