use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use prefix::Manager;
use prefix::ProcessTracker;
use scan::IconCache;
use store::PrefixStore;
use tokio::sync::Mutex as TokioMutex;

// ── Global singleton ─────────────────────────────────────────────────────

#[derive(Debug)]
struct GlobalState {
    /// Manager is behind a tokio Mutex so its guard is `Send` and can be
    /// held across `.await` points (needed for async graphics backend ops).
    prefix_manager: TokioMutex<Manager>,
    /// ProcessTracker uses std Mutex — always locked/unlocked synchronously.
    prefix_store: Arc<PrefixStore>,
    process_tracker: Arc<std::sync::Mutex<ProcessTracker>>,
    icon_cache: Arc<IconCache>,
    wine_dir: PathBuf,
}

static GLOBAL: OnceLock<GlobalState> = OnceLock::new();

/// Initialize the global service state. Must be called once at startup.
pub fn init(
    wine_dir: PathBuf,
    icon_cache: Arc<IconCache>,
    prefix_store: Arc<PrefixStore>,
    process_tracker: Arc<std::sync::Mutex<ProcessTracker>>,
) {
    let manager = Manager::new(wine_dir.clone(), icon_cache.clone());
    GLOBAL
        .set(GlobalState {
            prefix_manager: TokioMutex::new(manager),
            prefix_store,
            process_tracker,
            icon_cache,
            wine_dir,
        })
        .expect("Global state already initialized");
}

fn global() -> &'static GlobalState {
    GLOBAL.get().expect("Global state not initialized")
}

// ── Public accessors ────────────────────────────────────────────────────

pub fn manager() -> &'static TokioMutex<Manager> {
    &global().prefix_manager
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
