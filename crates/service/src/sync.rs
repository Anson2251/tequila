use base::WinePrefix;
use log::{error, info};
use std::path::PathBuf;

use crate::AppService;

/// Result of a full prefix sync operation.
pub struct SyncResult {
    pub prefixes: Vec<WinePrefix>,
}

/// Scan all prefixes on disk, then for each one:
/// 1. Scan for applications (exe detection)
/// 2. Save scan results to the persistent store
/// 3. Enrich executables with icon/metadata
/// 4. Persist any config changes
///
/// This is a blocking operation — run it on a background thread.
pub fn sync_all_prefixes(service: &AppService) -> SyncResult {
    let mut fresh = service.scan_prefixes();
    let total = fresh.len();
    info!("[sync] starting full sync of {} prefixes", total);

    for (i, p) in fresh.iter_mut().enumerate() {
        // Scan for applications
        if let Ok(exes) = service.prefix_manager().scan_for_applications(&p.path) {
            let _ = service
                .prefix_store()
                .save_scanned_executables(&p.path.to_string_lossy(), &exes);
        }
        // Enrich executables with icon/metadata
        let changed = service
            .prefix_manager()
            .enrich_executables(&p.path, &mut p.config);
        if changed {
            let _ = service.update_config(&p.path, &p.config);
        }

        info!("[sync] {}/{} scanned: {}", i + 1, total, p.path.display());
    }

    info!("[sync] full sync complete");
    SyncResult { prefixes: fresh }
}

/// Scan a single prefix for applications and update its config.
///
/// Returns the new executables found and the updated config.
pub fn scan_prefix_apps(
    service: &AppService,
    prefix_path: &PathBuf,
    config: base::PrefixConfig,
) -> ScanAppsResult {
    match service.prefix_manager().scan_for_applications(prefix_path) {
        Ok(executables) => {
            let initial_count = config.registered_executables.len();
            let mut new_config = config;
            for exe in &executables {
                new_config.add_executable(exe.clone());
            }
            let added = new_config.registered_executables.len() - initial_count;

            // Persist to config file
            if let Err(e) = service.update_config(prefix_path, &new_config) {
                error!(
                    "[service] failed to save config after scan for '{}': {}",
                    prefix_path.display(),
                    e
                );
            } else {
                info!(
                    "[service] scanned {} executables, {} new in prefix '{}'",
                    executables.len(),
                    added,
                    prefix_path.display()
                );
            }

            // Also save scanned executables to the store
            let _ = service
                .prefix_store()
                .save_scanned_executables(&prefix_path.to_string_lossy(), &executables);

            ScanAppsResult {
                executables,
                config: new_config,
                error: None,
            }
        }
        Err(e) => {
            error!(
                "[service] failed to scan applications in '{}': {}",
                prefix_path.display(),
                e
            );
            ScanAppsResult {
                executables: Vec::new(),
                config,
                error: Some(e.to_string()),
            }
        }
    }
}

pub struct ScanAppsResult {
    pub executables: Vec<base::RegisteredExecutable>,
    pub config: base::PrefixConfig,
    pub error: Option<String>,
}

/// Activate a new graphics backend with rollback on failure.
///
/// 1. Deactivate the old backend (if any)
/// 2. Activate the new backend (if any)
/// 3. On failure, restore the old config on disk
///
/// Returns `Ok(())` on success, or `Err(error_message)` on failure.
pub async fn switch_graphics_backend(
    service: &AppService,
    prefix_path: &PathBuf,
    old_graphics: &Option<base::GraphicsConfig>,
    new_graphics: &Option<base::GraphicsConfig>,
    rollback_config: &base::PrefixConfig,
) -> std::result::Result<(), String> {
    let mut last_error: Option<String> = None;

    // Deactivate old
    if let Some(old_gfx) = old_graphics {
        info!("[service] deactivating old graphics backend");
        if let Err(e) = service
            .prefix_manager()
            .deactivate_graphics_backend(prefix_path, Some(old_gfx.clone()))
            .await
        {
            let msg = format!("failed to deactivate old graphics: {}", e);
            error!("[service] {}", msg);
            last_error = Some(msg);
        }
    }

    // Activate new
    if last_error.is_none() {
        if let Some(gfx) = new_graphics {
            if let Some(backend) = gfx.to_backend() {
                info!(
                    "[service] activating {} graphics backend",
                    backend.display_name()
                );
                if let Err(e) = service
                    .prefix_manager()
                    .activate_graphics_backend(&backend, prefix_path)
                    .await
                {
                    let msg = format!(
                        "failed to activate {} graphics: {}",
                        backend.display_name(),
                        e
                    );
                    error!("[service] {}", msg);
                    last_error = Some(msg);
                }
            }
        }
    }

    // Rollback on failure
    if let Some(err) = last_error {
        let _ = service.update_config(prefix_path, rollback_config);
        return Err(format!(
            "Graphics backend switch failed: {} — config rolled back",
            err
        ));
    }

    Ok(())
}
