use base::{PrefixConfig, RegisteredExecutable};
use log::{error, info};
use std::path::PathBuf;

use crate::AppService;

/// Launch a registered executable and register it with the process tracker.
pub fn launch_executable(
    service: &AppService,
    prefix_path: &PathBuf,
    executable: &RegisteredExecutable,
) -> std::result::Result<u32, String> {
    match service
        .prefix_manager()
        .launch_executable(prefix_path, executable)
    {
        Ok(child) => {
            let pid = child.id();
            let mut tracker = service.process_tracker().lock().unwrap();
            tracker.register(&executable.executable_path, child);
            info!("[service] launched '{}' (PID: {})", executable.name, pid);
            Ok(pid)
        }
        Err(e) => {
            error!("[service] failed to launch '{}': {}", executable.name, e);
            Err(e.to_string())
        }
    }
}

/// Launch winecfg for a prefix.
pub fn launch_winecfg(
    service: &AppService,
    prefix_path: &PathBuf,
) -> std::result::Result<(), String> {
    let name = prefix_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    match service.prefix_manager().run_winecfg(prefix_path) {
        Ok(_) => {
            info!("[service] launched winecfg for prefix '{}'", name);
            Ok(())
        }
        Err(e) => {
            error!("[service] failed to launch winecfg for '{}': {}", name, e);
            Err(e.to_string())
        }
    }
}

/// Launch the Wine uninstaller for a prefix.
pub fn launch_uninstaller(
    service: &AppService,
    prefix_path: &PathBuf,
    config: &PrefixConfig,
) -> std::result::Result<PathBuf, String> {
    let track_path = prefix_path.join("__wine_uninstaller__");

    let mut cmd = service.prefix_manager().build_wine_command_with_args(
        &["uninstaller"],
        config,
        prefix_path,
    );
    cmd.current_dir(prefix_path);

    match cmd.spawn() {
        Ok(child) => {
            info!("[service] launched Wine uninstaller");
            let mut tracker = service.process_tracker().lock().unwrap();
            tracker.register(&track_path, child);
            Ok(track_path)
        }
        Err(e) => {
            error!("[service] failed to launch uninstaller: {}", e);
            Err(e.to_string())
        }
    }
}

/// Launch an arbitrary .exe directly (not through the registered apps list).
pub fn launch_direct_exe(
    service: &AppService,
    exe_path: &PathBuf,
    prefix_path: &PathBuf,
    config: &PrefixConfig,
) -> std::result::Result<(), String> {
    let mut cmd = service.prefix_manager().build_wine_command_with_args(
        &[&exe_path.to_string_lossy()],
        config,
        prefix_path,
    );
    cmd.current_dir(exe_path.parent().unwrap_or(prefix_path));

    match cmd.spawn() {
        Ok(child) => {
            info!("[service] launched exe directly: {}", exe_path.display());
            let mut tracker = service.process_tracker().lock().unwrap();
            tracker.register(exe_path, child);
            Ok(())
        }
        Err(e) => {
            error!("[service] failed to launch exe: {}", e);
            Err(e.to_string())
        }
    }
}

/// Reinitialize a prefix with a different runtime (blocking).
pub fn reinitialize_prefix(
    service: &AppService,
    prefix_path: &PathBuf,
    config: &PrefixConfig,
) -> std::result::Result<(), String> {
    service
        .prefix_manager()
        .reinitialize_prefix(prefix_path, config)
        .map_err(|e| e.to_string())
}

/// Poll for dead processes and return the set of currently running paths.
pub fn poll_dead_processes(service: &AppService) -> std::collections::HashSet<PathBuf> {
    let mut tracker = service.process_tracker().lock().unwrap();
    tracker.poll_dead();
    tracker.running_paths().into_iter().collect()
}
