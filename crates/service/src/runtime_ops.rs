use log::{error, info};
use runtime::RuntimeManager;
use std::path::PathBuf;

use crate::AppService;

/// Download a channel-based runtime and install it.
///
/// This is a blocking operation — run it on a background thread.
pub fn download_channel_runtime_blocking(
    service: &mut AppService,
    channel: runtime::Channel,
    progress: runtime::download::ProgressFn,
) -> std::result::Result<RuntimeManager, String> {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create tokio runtime: {}", e))?;

    rt.block_on(async {
        let _ = progress(0, 0); // signal start
        service
            .prefix_manager_mut()
            .download_channel_runtime(channel, progress)
            .await
            .map_err(|e| e.to_string())?;

        // Return the updated runtime manager
        Ok(service.prefix_manager().runtime_manager().clone())
    })
}

/// Import a runtime from a local directory.
pub fn import_runtime_from_path(
    service: &mut AppService,
    source_path: &PathBuf,
    label: &str,
) -> std::result::Result<RuntimeManager, String> {
    let _ = service
        .prefix_manager_mut()
        .import_runtime(source_path, label)
        .map_err(|e| e.to_string())?;
    service.prefix_manager().save_runtime_state();
    Ok(service.prefix_manager().runtime_manager().clone())
}

/// Remove a runtime's directory from disk and unregister it.
pub fn remove_runtime_full(
    service: &mut AppService,
    id: &str,
) -> std::result::Result<RuntimeManager, String> {
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
    service.prefix_manager_mut().remove_runtime(id);
    service.prefix_manager().save_runtime_state();

    info!("[service] removed runtime '{}'", id);
    Ok(service.prefix_manager().runtime_manager().clone())
}

/// Set the default runtime and persist.
pub fn set_default_runtime(
    service: &mut AppService,
    id: &str,
) -> std::result::Result<RuntimeManager, String> {
    service.prefix_manager_mut().set_default_runtime(id);
    service.prefix_manager().save_runtime_state();
    Ok(service.prefix_manager().runtime_manager().clone())
}

/// Ensure the system Wine runtime is detected.
pub fn ensure_system_runtime(service: &mut AppService) {
    service
        .prefix_manager_mut()
        .runtime_manager_mut()
        .ensure_system_runtime();
}
