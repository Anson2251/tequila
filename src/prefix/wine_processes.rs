use crate::prefix::error::{Result, PrefixError};
use crate::prefix::runtime::Runtime;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// Apply runtime environment to a Command before spawning.
///
/// Prepends `runtime.bundle_dir/bin` to PATH for managed/imported runtimes.
/// For system Wine, the PATH is left as-is (wine is already on PATH).
/// Also injects GStreamer environment variables (macOS).
pub fn apply_runtime_env(cmd: &mut Command, runtime: &Runtime, prefix_path: &Path) {
    cmd.env("WINEPREFIX", prefix_path);

    let system_path = std::env::var("PATH").unwrap_or_default();

    let path = if runtime.bundle_dir.as_os_str().is_empty() {
        // System runtime — wine is already on PATH
        system_path.clone()
    } else {
        format!(
            "{}:{}",
            runtime.bundle_dir.join("bin").display(),
            system_path
        )
    };

    let mut path = path;

    // GStreamer (macOS) — read env vars from a key=val file generated at install time
    if let Some(gst_dir) = find_gstreamer_dir() {
        if let Ok(content) = std::fs::read_to_string(gst_dir.join("env")) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    if k == "PATH_PREPEND" {
                        path = format!("{}:{}", v, path);
                    } else {
                        cmd.env(k, v);
                    }
                }
            }
        }
    }

    cmd.env("PATH", &path);
}

/// Locate the shared GStreamer runtime installation.
fn find_gstreamer_dir() -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    let gst_dir = data_dir.join("tequila").join("runtimes").join("gstreamer");
    if gst_dir.is_dir() {
        Some(gst_dir)
    } else {
        None
    }
}

/// Trait for Wine process operations
pub trait WineProcesses {
    /// Get the Wine version for this prefix
    fn get_wine_version(&self) -> Result<String>;

    /// Start winecfg for this prefix
    fn start_winecfg(&self) -> Result<()>;

    /// Start regedit for this prefix
    fn start_regedit(&self) -> Result<()>;

    /// Start control panel for this prefix
    fn start_control_panel(&self) -> Result<()>;

    /// Run an executable within this prefix
    fn run_executable(&self, executable_path: &PathBuf) -> Result<()>;

    /// Run a Windows command within this prefix
    fn run_windows_command(&self, command: &str) -> Result<()>;
}

/// Implementation of WineProcesses for WinePrefix
impl WineProcesses for super::traits::WinePrefix {
    fn get_wine_version(&self) -> Result<String> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        let output = Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("--version")
            .output()
            .map_err(|e| PrefixError::Process(format!("Failed to get wine version: {}", e)))?;

        if output.status.success() {
            let version = String::from_utf8(output.stdout)
                .map_err(|e| PrefixError::Process(format!("Failed to parse wine version: {}", e)))?;
            Ok(version.trim().to_string())
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(PrefixError::Process(format!("Failed to get wine version: {}", error)))
        }
    }

    fn start_winecfg(&self) -> Result<()> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("winecfg")
            .env("WINEPREFIX", &wine_prefix)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start winecfg: {}", e)))?;
        Ok(())
    }

    fn start_regedit(&self) -> Result<()> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("regedit")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start regedit: {}", e)))?;
        Ok(())
    }

    fn start_control_panel(&self) -> Result<()> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("control")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start control panel: {}", e)))?;
        Ok(())
    }

    fn run_executable(&self, executable_path: &PathBuf) -> Result<()> {
        if !executable_path.exists() {
            return Err(PrefixError::NotFound(format!("Executable not found: {}", executable_path.display())));
        }

        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg(executable_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run executable: {}", e)))?;
        Ok(())
    }

    fn run_windows_command(&self, command: &str) -> Result<()> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("cmd")
            .arg("/c")
            .arg(command)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run Windows command: {}", e)))?;
        Ok(())
    }
}

/// Implementation of WineProcesses for Manager
impl WineProcesses for super::manager::Manager {
    fn get_wine_version(&self) -> Result<String> {
        let output = Command::new("wine")
            .arg("--version")
            .output()
            .map_err(|e| PrefixError::Process(format!("Failed to get wine version: {}", e)))?;

        if output.status.success() {
            let version = String::from_utf8(output.stdout)
                .map_err(|e| PrefixError::Process(format!("Failed to parse wine version: {}", e)))?;
            Ok(version.trim().to_string())
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(PrefixError::Process(format!("Failed to get wine version: {}", error)))
        }
    }

    fn start_winecfg(&self) -> Result<()> {
        Command::new("winecfg")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start winecfg: {}", e)))?;
        Ok(())
    }

    fn start_regedit(&self) -> Result<()> {
        Command::new("wine")
            .arg("regedit")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start regedit: {}", e)))?;
        Ok(())
    }

    fn start_control_panel(&self) -> Result<()> {
        Command::new("wine")
            .arg("control")
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to start control panel: {}", e)))?;
        Ok(())
    }

    fn run_executable(&self, executable_path: &PathBuf) -> Result<()> {
        if !executable_path.exists() {
            return Err(PrefixError::NotFound(format!("Executable not found: {}", executable_path.display())));
        }

        Command::new("wine")
            .arg(executable_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run executable: {}", e)))?;
        Ok(())
    }

    fn run_windows_command(&self, command: &str) -> Result<()> {
        Command::new("wine")
            .arg("cmd")
            .arg("/c")
            .arg(command)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run Windows command: {}", e)))?;
        Ok(())
    }
}
