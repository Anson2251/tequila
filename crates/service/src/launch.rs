use base::config::PrefixConfig;
use base::RegisteredExecutable;
use log::{error, info};
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;

use crate::AppService;

/// Launch a registered executable and register it with the process tracker.
pub fn launch_executable(
    service: &AppService,
    prefix_path: &Path,
    executable: &RegisteredExecutable,
) -> std::result::Result<u32, String> {
    let prefix = match service.prefix_manager().open_prefix(prefix_path) {
        Ok(p) => p,
        Err(e) => return Err(e.to_string()),
    };

    match prefix.launch_executable(executable) {
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
    prefix_path: &Path,
) -> std::result::Result<(), String> {
    let prefix = match service.prefix_manager().open_prefix(prefix_path) {
        Ok(p) => p,
        Err(e) => return Err(e.to_string()),
    };

    let name = prefix.name().to_string();
    match prefix.run_winecfg() {
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
    prefix_path: &Path,
    _config: &PrefixConfig,
) -> std::result::Result<PathBuf, String> {
    let track_path = prefix_path.join("__wine_uninstaller__");

    let prefix = match service.prefix_manager().open_prefix(prefix_path) {
        Ok(p) => p,
        Err(e) => return Err(e.to_string()),
    };

    let mut cmd = prefix.build_wine_command_with_args(&["uninstaller"]);
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
    prefix_path: &Path,
    _config: &PrefixConfig,
) -> std::result::Result<(), String> {
    let prefix = match service.prefix_manager().open_prefix(prefix_path) {
        Ok(p) => p,
        Err(e) => return Err(e.to_string()),
    };

    let mut cmd = prefix.build_wine_command_with_args(&[&exe_path.to_string_lossy()]);
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

/// Launch a registered executable with piped stdout/stderr/stdin for debugging.
/// The returned `Child` has its stdout, stderr, and stdin pipes available.
pub fn launch_executable_debug(
    service: &AppService,
    prefix_path: &Path,
    executable: &RegisteredExecutable,
) -> std::result::Result<std::process::Child, String> {
    let prefix = match service.prefix_manager().open_prefix(prefix_path) {
        Ok(p) => p,
        Err(e) => return Err(e.to_string()),
    };

    if !executable.executable_path.exists() {
        return Err("Executable file does not exist".to_string());
    }

    let mut cmd = prefix.build_wine_command_with_args(
        &[&executable.executable_path.to_string_lossy()],
    );

    info!(
        "[service] launching '{}' in debug mode for prefix '{}'",
        executable.name, prefix_path.display()
    );

    // Set up pipes for debug capture
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::piped());

    // Apply per-executable environment variables
    for (key, value) in &executable.env_vars {
        cmd.env(key, value);
    }

    // Apply per-executable working directory (fall back to prefix path)
    if let Some(cwd) = &executable.cwd {
        cmd.current_dir(cwd);
    } else {
        cmd.current_dir(prefix_path);
    }

    match cmd.spawn() {
        Ok(child) => {
            info!("[service] debug process started (PID: {})", child.id());
            Ok(child)
        }
        Err(e) => {
            error!("[service] failed to launch debug '{}': {}", executable.name, e);
            Err(format!("Failed to launch executable: {}", e))
        }
    }
}

/// Register a debug-mode PID with the process tracker so it gets killed
/// on Ctrl+C / shutdown even though the debug window owns the Child handle.
pub fn track_debug_process(
    service: &AppService,
    exe_path: &Path,
    pid: u32,
) {
    let mut tracker = service.process_tracker().lock().unwrap();
    tracker.track_pid(exe_path, pid);
    info!("[service] tracking debug PID {} for {}", pid, exe_path.display());
}

/// Reinitialize a prefix with a different runtime (blocking).
pub fn reinitialize_prefix(
    service: &AppService,
    prefix_path: &Path,
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

/// Check whether a process with the given executable path is still running.
pub fn is_process_running(service: &AppService, path: &PathBuf) -> bool {
    service.process_tracker().lock().unwrap().is_running(path)
}

/// Kill a running process by executable path. Returns true if the process was found and killed.
pub fn kill_process(service: &AppService, path: &PathBuf) -> bool {
    service.process_tracker().lock().unwrap().kill(path)
}
