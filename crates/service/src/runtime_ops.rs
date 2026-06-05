use log::{error, info};
use runtime::RuntimeManager;
use std::path::PathBuf;

use crate::state;

/// Download a channel-based runtime and install it.
///
/// This is a blocking operation — run it on a background thread.
pub fn download_channel_runtime_blocking(
    channel: runtime::Channel,
    progress: runtime::download::ProgressFn,
) -> std::result::Result<RuntimeManager, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

    rt.block_on(async {
        let _ = progress(0, 0); // signal start

        // Async download OUTSIDE the Manager lock
        let bundle_dir = runtime::download::download_channel_runtime(&channel, &progress)
            .await
            .map_err(|e| e.to_string())?;
        let cask = runtime::homebrew::fetch_cask(channel.cask_name())
            .await
            .map_err(|e| e.to_string())?;

        // Lock Manager briefly for registration + save
        let mgr = state::manager_write();
        mgr.register_channel_runtime(channel, cask.version, bundle_dir);
        Ok(mgr.clone_runtime())
    })
}

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
