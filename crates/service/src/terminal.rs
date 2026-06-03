use log::error;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::AppService;

/// Generate a terminal script for a prefix and open a terminal emulator to run it.
///
/// 1. Generate the script via `Manager::generate_terminal_script`
/// 2. Write it to a temp file
/// 3. chmod +x
/// 4. Spawn a terminal emulator
pub fn open_terminal_for_prefix(
    service: &AppService,
    prefix_path: &Path,
) -> std::result::Result<(), String> {
    let script = service
        .prefix_manager()
        .generate_terminal_script(&prefix_path.to_path_buf())
        .map_err(|e| e.to_string())?;

    let tmp = std::env::temp_dir().join("tequila-terminal.sh");
    fs::write(&tmp, &script).map_err(|e| {
        let msg = format!("Failed to write script: {}", e);
        error!("[terminal] {}", msg);
        msg
    })?;

    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755)).map_err(|e| {
        let msg = format!("Failed to chmod script: {}", e);
        error!("[terminal] {}", msg);
        msg
    })?;

    open_terminal_with_script(&tmp);
    Ok(())
}

/// Spawn a terminal emulator to run the given shell script.
///
/// On macOS uses Terminal.app via AppleScript; on Linux tries common terminals.
pub fn open_terminal_with_script(script_path: &Path) {
    let path_str = script_path.to_string_lossy();

    #[cfg(target_os = "macos")]
    {
        let template = include_str!("../../../scripts/tequila-terminal.applescript");
        let src = template.replace("__TEQUILA_SCRIPT_PATH__", &path_str.replace('"', "\\\""));
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&src)
            .status();
        let _ = std::process::Command::new("open")
            .args(["-a", "Terminal"])
            .status();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let cmds: &[&[&str]] = &[
            &["x-terminal-emulator", "-e", "bash", &path_str],
            &["gnome-terminal", "--", "bash", &path_str],
            &["xfce4-terminal", "-e", "bash", &path_str],
            &["konsole", "-e", "bash", &path_str],
            &["lxterminal", "-e", "bash", &path_str],
            &["xterm", "-e", "bash", &path_str],
            &["kgx", "-e", "bash", &path_str],
        ];
        for args in cmds {
            let cmd = args[0];
            let rest = &args[1..];
            if std::process::Command::new(cmd).args(rest).spawn().is_ok() {
                return;
            }
        }
    }
}

/// Open the system file manager at the given path.
pub fn open_in_file_manager(path: &Path) {
    let path_str = path.to_string_lossy();
    let path_ref: &str = &path_str;
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path_ref).status();
    #[cfg(not(target_os = "macos"))]
    let _ = std::process::Command::new("xdg-open")
        .arg(path_ref)
        .status();
}
