use runtime::RuntimeManager;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct Manager {
    pub(crate) wine_dir: PathBuf,
    pub(crate) scanner: scan::ApplicationScanner,
    pub(crate) runtime_manager: RuntimeManager,
}

impl fmt::Debug for Manager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Manager")
            .field("wine_dir", &self.wine_dir)
            .field("runtime_manager", &self.runtime_manager)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Manager {
    fn eq(&self, other: &Self) -> bool {
        self.wine_dir == other.wine_dir && self.runtime_manager == other.runtime_manager
    }
}

impl Manager {
    pub fn new(wine_dir: PathBuf, icon_cache: Arc<scan::IconCache>) -> Self {
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
            scanner: scan::ApplicationScanner::new(icon_cache),
            runtime_manager,
        }
    }

    pub fn wine_dir(&self) -> &PathBuf {
        &self.wine_dir
    }
    pub fn scanner(&self) -> &scan::ApplicationScanner {
        &self.scanner
    }
    pub fn runtime_manager(&self) -> &RuntimeManager {
        &self.runtime_manager
    }
    pub fn runtime_manager_mut(&mut self) -> &mut RuntimeManager {
        &mut self.runtime_manager
    }
}
