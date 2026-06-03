use base::{PrefixConfig, RegisteredExecutable};
use log::{error, info};
use std::path::PathBuf;

use crate::AppService;

/// Add a registered executable to a prefix's config and persist it.
pub fn add_executable(
    service: &AppService,
    prefix_path: &PathBuf,
    config: &mut PrefixConfig,
    executable: RegisteredExecutable,
) -> bool {
    config.add_executable(executable);
    match service.update_config(prefix_path, config) {
        Ok(()) => {
            info!(
                "[service] added executable to prefix '{}'",
                prefix_path.display()
            );
            true
        }
        Err(e) => {
            error!("[service] failed to save config: {}", e);
            false
        }
    }
}

/// Add multiple registered executables to a prefix's config and persist it.
pub fn add_executables(
    service: &AppService,
    prefix_path: &PathBuf,
    config: &mut PrefixConfig,
    executables: &[RegisteredExecutable],
) -> bool {
    for exe in executables {
        config.add_executable(exe.clone());
    }
    match service.update_config(prefix_path, config) {
        Ok(()) => {
            info!(
                "[service] added {} executables to prefix '{}'",
                executables.len(),
                prefix_path.display()
            );
            true
        }
        Err(e) => {
            error!("[service] failed to save config: {}", e);
            false
        }
    }
}

/// Remove a registered executable from a prefix's config and persist it.
pub fn remove_executable(
    service: &AppService,
    prefix_path: &PathBuf,
    config: &mut PrefixConfig,
    index: usize,
) -> bool {
    if index >= config.registered_executables.len() {
        return false;
    }
    config.remove_executable(index);
    match service.update_config(prefix_path, config) {
        Ok(()) => {
            info!(
                "[service] removed executable at index {} from '{}'",
                index,
                prefix_path.display()
            );
            true
        }
        Err(e) => {
            error!("[service] failed to save config: {}", e);
            false
        }
    }
}

/// Update a single executable's settings and persist the config.
pub fn update_executable(
    service: &AppService,
    prefix_path: &PathBuf,
    config: &mut PrefixConfig,
    updated_exec: RegisteredExecutable,
) -> bool {
    if let Some(pos) = config
        .registered_executables
        .iter()
        .position(|e| e.executable_path == updated_exec.executable_path)
    {
        config.registered_executables[pos] = updated_exec;
        match service.update_config(prefix_path, config) {
            Ok(()) => {
                info!(
                    "[service] updated executable settings in '{}'",
                    prefix_path.display()
                );
                true
            }
            Err(e) => {
                error!("[service] failed to save config: {}", e);
                false
            }
        }
    } else {
        error!("[service] executable not found in config");
        false
    }
}
