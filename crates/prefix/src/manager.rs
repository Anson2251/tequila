use runtime::RuntimeManager;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use store::PrefixStore;

#[derive(Clone)]
pub struct Manager {
    pub(crate) wine_dir: PathBuf,
    pub(crate) scanner: Arc<scan::ApplicationScanner>,
    pub(crate) runtime_manager: Arc<RwLock<RuntimeManager>>,
    pub(crate) store: Arc<PrefixStore>,
}

impl fmt::Debug for Manager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Manager")
            .field("wine_dir", &self.wine_dir)
            .finish_non_exhaustive()
    }
}

impl Manager {
    pub fn new(
        wine_dir: PathBuf,
        icon_cache: Arc<scan::IconCache>,
        store: Arc<PrefixStore>,
    ) -> Self {
        let mut runtime_manager = RuntimeManager::new();
        if let Some(settings) = store::Settings::load() {
            let mut rm: RuntimeManager = settings.into();
            rm.ensure_system_runtime();
            runtime_manager = rm;
        } else {
            runtime_manager.ensure_system_runtime();
        }
        Self {
            wine_dir,
            scanner: Arc::new(scan::ApplicationScanner::new(icon_cache)),
            runtime_manager: Arc::new(RwLock::new(runtime_manager)),
            store,
        }
    }

    pub fn wine_dir(&self) -> &PathBuf {
        &self.wine_dir
    }

    pub fn scanner(&self) -> &Arc<scan::ApplicationScanner> {
        &self.scanner
    }

    pub fn runtime_manager(&self) -> &Arc<RwLock<RuntimeManager>> {
        &self.runtime_manager
    }

    pub fn store(&self) -> &Arc<PrefixStore> {
        &self.store
    }

    /// Convenience: lock the runtime manager for reading.
    pub fn read_runtime(&self) -> std::sync::RwLockReadGuard<'_, RuntimeManager> {
        self.runtime_manager.read().unwrap()
    }

    /// Convenience: lock the runtime manager for writing.
    pub fn write_runtime(&self) -> std::sync::RwLockWriteGuard<'_, RuntimeManager> {
        self.runtime_manager.write().unwrap()
    }

    /// Convenience: clone the runtime manager (read lock + clone).
    pub fn clone_runtime(&self) -> RuntimeManager {
        self.runtime_manager.read().unwrap().clone()
    }
}
