use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use prefix::Manager;
use prefix::ProcessTracker;
use scan::IconCache;
use store::PrefixStore;

// ── Global singleton ─────────────────────────────────────────────────────

struct GlobalState {
    /// Manager is behind an RwLock.  Read access is lock-free for most
    /// operations; writes (config save, runtime management) are exclusive.
    prefix_manager: RwLock<Manager>,
    prefix_store: Arc<PrefixStore>,
    process_tracker: Arc<Mutex<ProcessTracker>>,
    icon_cache: Arc<IconCache>,
    wine_dir: PathBuf,
}

static GLOBAL: OnceLock<GlobalState> = OnceLock::new();

/// Initialize the global service state. Must be called once at startup.
pub fn init(
    wine_dir: PathBuf,
    icon_cache: Arc<IconCache>,
    prefix_store: Arc<PrefixStore>,
    process_tracker: Arc<Mutex<ProcessTracker>>,
) {
    let manager = Manager::new(wine_dir.clone(), icon_cache.clone());
    GLOBAL
        .set(GlobalState {
            prefix_manager: RwLock::new(manager),
            prefix_store,
            process_tracker,
            icon_cache,
            wine_dir,
        })
        .unwrap_or_else(|_| panic!("Global state already initialized"));
}

fn global() -> &'static GlobalState {
    GLOBAL
        .get()
        .unwrap_or_else(|| panic!("Global state not initialized"))
}

// ── Public accessors ────────────────────────────────────────────────────

/// Read access to the prefix manager — can be held across `.await`.
pub fn manager_read() -> std::sync::RwLockReadGuard<'static, Manager> {
    global()
        .prefix_manager
        .read()
        .unwrap_or_else(|_| panic!("RwLock poisoned"))
}

/// Write access to the prefix manager.
pub fn manager_write() -> std::sync::RwLockWriteGuard<'static, Manager> {
    global()
        .prefix_manager
        .write()
        .unwrap_or_else(|_| panic!("RwLock poisoned"))
}

pub fn prefix_store() -> &'static Arc<PrefixStore> {
    &global().prefix_store
}

pub fn process_tracker() -> &'static Arc<std::sync::Mutex<ProcessTracker>> {
    &global().process_tracker
}

pub fn icon_cache() -> &'static Arc<IconCache> {
    &global().icon_cache
}

pub fn wine_dir() -> &'static PathBuf {
    &global().wine_dir
}
