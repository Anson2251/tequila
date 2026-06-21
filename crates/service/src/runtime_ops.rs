use log::{error, info};
use runtime::RuntimeManager;
use std::path::PathBuf;

use crate::state;

/// Import a runtime from a local directory.
pub fn import_runtime_from_path(
    source_path: &PathBuf,
    label: &str,
) -> std::result::Result<RuntimeManager, String> {
    let mgr = state::manager_write();
    let _ = mgr
        .import_runtime(source_path, label)
        .map_err(|e| e.to_string())?;
    mgr.save_runtime_state();
    Ok(mgr.clone_runtime())
}

/// Remove a runtime's directory from disk and unregister it.
pub fn remove_runtime_full(id: &str) -> std::result::Result<RuntimeManager, String> {
    if id == "wine-system" {
        return Err("Cannot remove system Wine runtime".to_string());
    }

    // Delete the runtime directory from disk
    let dir = runtime::download::runtimes_dir().join(id);
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            error!("[service] failed to remove runtime dir: {}", e);
        }
    }

    // Remove from the runtime manager and save
    let mgr = state::manager_write();
    mgr.remove_runtime(id);
    mgr.save_runtime_state();

    info!("[service] removed runtime '{}'", id);
    Ok(mgr.clone_runtime())
}

/// Set the default runtime and persist.
pub fn set_default_runtime(id: &str) -> std::result::Result<RuntimeManager, String> {
    let mgr = state::manager_write();
    mgr.set_default_runtime(id);
    mgr.save_runtime_state();
    Ok(mgr.clone_runtime())
}

/// Ensure the system Wine runtime is detected.
pub fn ensure_system_runtime() {
    let mgr = state::manager_write();
    mgr.write_runtime().ensure_system_runtime();
}
