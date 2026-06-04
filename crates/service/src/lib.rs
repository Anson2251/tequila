pub mod config_ops;
pub mod launch;
pub mod runtime_ops;
pub mod state;
pub mod sync;
pub mod terminal;

use base::{PrefixConfig, WinePrefix, error::Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// High-level application service that owns the business logic layer.
///
/// This is a **singleton handle** — all instances delegate to a single
/// global state initialized via [`AppService::init_global`].
///
/// All UI components should delegate to this service rather than
/// performing I/O, spawning processes, or orchestrating multi-step
/// workflows directly.
#[derive(Clone, Copy, Debug)]
pub struct AppService;

impl AppService {
    /// Initialize the global service singleton. Must be called once at startup
    /// before any other `AppService` method is used.
    pub fn init_global(
        wine_dir: PathBuf,
        icon_cache: Arc<scan::IconCache>,
        prefix_store: Arc<prefix::PrefixStore>,
        process_tracker: Arc<Mutex<prefix::ProcessTracker>>,
    ) {
        state::init(wine_dir, icon_cache, prefix_store, process_tracker);
    }

    /// Convenience accessor for the global instance.
    pub fn global() -> Self {
        AppService
    }

    // ── Internal state access (locked) ─────────────────────────────────

    /// Lock the prefix manager for any access (read or write) — **sync**.
    ///
    /// Blocks the current thread until the lock is acquired.
    /// Prefer [`prefix_manager_async`](Self::prefix_manager_async) when calling
    /// from an async context to avoid blocking the executor thread.
    pub fn prefix_manager(&self) -> tokio::sync::MutexGuard<'_, prefix::Manager> {
        state::manager().blocking_lock()
    }

    /// Lock the prefix manager — **async** version, suitable for `.await`
    /// contexts (e.g. `glib::spawn_local`).
    pub async fn prefix_manager_async(&self) -> tokio::sync::MutexGuard<'_, prefix::Manager> {
        state::manager().lock().await
    }

    /// Alias for code clarity — same as [`prefix_manager`](Self::prefix_manager).
    pub fn prefix_manager_mut(&self) -> tokio::sync::MutexGuard<'_, prefix::Manager> {
        state::manager().blocking_lock()
    }

    /// Async alias for mutable semantics.
    pub async fn prefix_manager_mut_async(&self) -> tokio::sync::MutexGuard<'_, prefix::Manager> {
        state::manager().lock().await
    }

    /// Shared access to the persistent prefix store.
    pub fn prefix_store(&self) -> &Arc<prefix::PrefixStore> {
        state::prefix_store()
    }

    /// Shared access to the process tracker.
    pub fn process_tracker(&self) -> &Arc<Mutex<prefix::ProcessTracker>> {
        state::process_tracker()
    }

    // ── High-level operations ──────────────────────────────────────────

    /// Scan for all Wine prefixes on disk.
    pub fn scan_prefixes(&self) -> Vec<WinePrefix> {
        match self.prefix_manager().scan_prefixes() {
            Ok(p) => p,
            Err(e) => {
                log::error!("[service] error scanning prefixes: {}", e);
                Vec::new()
            }
        }
    }

    /// Delete a prefix from disk and remove it from the list.
    pub fn delete_prefix(&self, prefix_path: &PathBuf, prefixes: &mut Vec<WinePrefix>) -> bool {
        if let Err(e) = self.prefix_manager().delete_prefix(prefix_path) {
            log::error!("[service] failed to delete prefix: {}", e);
            return false;
        }
        if let Some(idx) = prefixes.iter().position(|p| p.path == *prefix_path) {
            prefixes.remove(idx);
            log::info!(
                "[service] deleted prefix: {}",
                prefix_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
            );
            return true;
        }
        true
    }

    /// Save a config update for a prefix.
    pub fn update_config(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<()> {
        self.prefix_manager().update_config(prefix_path, config)
    }

    /// Check if the prefix store has scan results for the given path.
    pub fn has_scanned_prefix(&self, prefix_path: &str) -> bool {
        self.prefix_store().has_scanned_prefix(prefix_path)
    }

    /// Resolve the runtime display name for a prefix config.
    pub fn resolve_runtime_display_name(&self, config: &PrefixConfig) -> String {
        let rt = config.wine_version.as_ref().and_then(|id| {
            let mgr = self.prefix_manager();
            mgr.runtime_manager().get(id).cloned()
        });
        match rt {
            Some(r) => format!("{} ({})", r.name, r.wine_version),
            None => config
                .wine_version
                .as_deref()
                .unwrap_or("Unknown")
                .to_string(),
        }
    }
}
