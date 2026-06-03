pub mod config_ops;
pub mod launch;
pub mod runtime_ops;
pub mod sync;
pub mod terminal;

use base::{PrefixConfig, WinePrefix, error::Result};
use prefix::Manager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// High-level application service that owns the business logic layer.
///
/// All UI components should delegate to this service rather than
/// performing I/O, spawning processes, or orchestrating multi-step
/// workflows directly.
#[derive(Clone)]
pub struct AppService {
    prefix_manager: Manager,
    prefix_store: Arc<prefix::PrefixStore>,
    process_tracker: Arc<Mutex<prefix::ProcessTracker>>,
}

impl AppService {
    pub fn new(
        wine_dir: PathBuf,
        icon_cache: Arc<scan::IconCache>,
        prefix_store: Arc<prefix::PrefixStore>,
        process_tracker: Arc<Mutex<prefix::ProcessTracker>>,
    ) -> Self {
        let prefix_manager = Manager::new(wine_dir, icon_cache);
        Self {
            prefix_manager,
            prefix_store,
            process_tracker,
        }
    }

    /// Create from an existing Manager (e.g. when the caller already has one).
    pub fn from_manager(
        prefix_manager: Manager,
        prefix_store: Arc<prefix::PrefixStore>,
        process_tracker: Arc<Mutex<prefix::ProcessTracker>>,
    ) -> Self {
        Self {
            prefix_manager,
            prefix_store,
            process_tracker,
        }
    }

    /// Read-only access to the prefix manager.
    pub fn prefix_manager(&self) -> &Manager {
        &self.prefix_manager
    }

    /// Mutable access to the prefix manager.
    pub fn prefix_manager_mut(&mut self) -> &mut Manager {
        &mut self.prefix_manager
    }

    /// Shared access to the persistent prefix store.
    pub fn prefix_store(&self) -> &Arc<prefix::PrefixStore> {
        &self.prefix_store
    }

    /// Shared access to the process tracker.
    pub fn process_tracker(&self) -> &Arc<Mutex<prefix::ProcessTracker>> {
        &self.process_tracker
    }

    /// Scan for all Wine prefixes on disk.
    pub fn scan_prefixes(&self) -> Vec<WinePrefix> {
        match self.prefix_manager.scan_prefixes() {
            Ok(p) => p,
            Err(e) => {
                log::error!("[service] error scanning prefixes: {}", e);
                Vec::new()
            }
        }
    }

    /// Delete a prefix from disk and remove it from the list.
    pub fn delete_prefix(&self, prefix_path: &PathBuf, prefixes: &mut Vec<WinePrefix>) -> bool {
        if let Err(e) = self.prefix_manager.delete_prefix(prefix_path) {
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
        self.prefix_manager.update_config(prefix_path, config)
    }

    /// Check if the prefix store has scan results for the given path.
    pub fn has_scanned_prefix(&self, prefix_path: &str) -> bool {
        self.prefix_store.has_scanned_prefix(prefix_path)
    }

    /// Resolve the runtime display name for a prefix config.
    pub fn resolve_runtime_display_name(&self, config: &PrefixConfig) -> String {
        config
            .wine_version
            .as_ref()
            .and_then(|id| self.prefix_manager.runtime_manager().get(id))
            .map(|r| format!("{} ({})", r.name, r.wine_version))
            .unwrap_or_else(|| {
                config
                    .wine_version
                    .as_deref()
                    .unwrap_or("Unknown")
                    .to_string()
            })
    }
}
