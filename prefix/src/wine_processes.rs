use base::config::PrefixConfig;
use base::error::{PrefixError, Result};
use base::GraphicsBackend;
use log::info;
use runtime::Runtime;
use runtime::graphics;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn apply_runtime_env(cmd: &mut Command, runtime: &Runtime, prefix_path: &Path) {
    cmd.env("WINEPREFIX", prefix_path);
    info!("[spawn] WINEPREFIX={}", prefix_path.to_string_lossy());
    let system_path = std::env::var("PATH").unwrap_or_default();
    let path = if runtime.bundle_dir.as_os_str().is_empty() {
        system_path.clone()
    } else {
        format!(
            "{}:{}",
            runtime.bundle_dir.join("bin").display(),
            system_path
        )
    };
    let mut path = path;

    if let Some(gst_dir) = find_gstreamer_dir() {
        if let Ok(content) = std::fs::read_to_string(gst_dir.join("env")) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    if k == "PATH_PREPEND" {
                        path = format!("{}:{}", v, path);
                    } else {
                        cmd.env(k, v);
                        info!("[spawn] {}={}", k, v);
                    }
                }
            }
        }
    }

    cmd.env("PATH", &path);
    // info!("[spawn] PATH=...{}", &path[..path.len().saturating_sub(80)]);

    // Graphics backend: inject .so search path and DLL overrides
    apply_graphics_env(cmd, prefix_path);
}

/// Inject WINEDLLPATH and WINEDLLOVERRIDES for the prefix's graphics backend.
///
/// Different backends (dxmt, d3dmetal, dxvk+vkd3d) have different directory
/// layouts for their .so files. We reconstruct the `GraphicsBackend` from the
/// stored config to derive correct paths per-backend. For WINEDLLOVERRIDES,
/// we merge with any existing environment value so user customizations are
/// preserved.
fn apply_graphics_env(cmd: &mut Command, prefix_path: &Path) {
    let pb = prefix_path.to_path_buf();
    let config = match PrefixConfig::load_from_file(&pb) {
        Ok(Some(c)) => c,
        _ => return,
    };
    let gfx = match config.graphics {
        Some(ref g) => g,
        None => return,
    };

    // Reconstruct the backend enum for backend-specific path logic
    let backend = match gfx.to_backend() {
        Some(b) => b,
        None => return,
    };

    // ── WINEDLLPATH ──────────────────────────────────────────────
    // DXMT stores .so files under  dxmt-{version}/lib/wine/x86_64-unix/
    // D3DMetal (GPTK) stores .so files under  d3dmetal-{version}/lib/wine/x86_64-unix/
    // DXVK+VKD3D uses two separate directories and each may have its own .so files.
    let gfx_dir = graphics::graphics_dir();
    let so_dirs: Vec<PathBuf> = match &backend {
        GraphicsBackend::Dxmt { version } => {
            // WINEDLLPATH only for .so (unixlib) files — DXMT provides winemetal.so.
            // All .dll files are symlinked into prefix system32 as native overrides.
            vec![gfx_dir
                .join(format!("dxmt-{}", version))
                .join("lib")
                .join("wine")
                .join("x86_64-unix")]
        }
        GraphicsBackend::D3DMetal { version } => {
            vec![gfx_dir
                .join(format!("d3dmetal-{}", version))
                .join("lib")
                .join("wine")
                .join("x86_64-unix")]
        }
        GraphicsBackend::DxvkVkd3d {
            dxvk_version,
            vkd3d_version,
        } => {
            let mut paths = Vec::new();
            let dxvk_path = gfx_dir
                .join(format!("dxvk-{}", dxvk_version))
                .join("lib")
                .join("wine")
                .join("x86_64-unix");
            if dxvk_path.exists() {
                paths.push(dxvk_path);
            }
            let vkd3d_path = gfx_dir
                .join(format!("vkd3d-{}", vkd3d_version))
                .join("lib")
                .join("wine")
                .join("x86_64-unix");
            if vkd3d_path.exists() {
                paths.push(vkd3d_path);
            }
            paths
        }
    };

    // Only set WINEDLLPATH for directories that actually exist
    let existing: Vec<String> = so_dirs
        .into_iter()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if !existing.is_empty() {
        let winedllpath = existing.join(":");
        cmd.env("WINEDLLPATH", &winedllpath);
        info!("[spawn] WINEDLLPATH={}", winedllpath);
    }

    // ── WINEDLLOVERRIDES ──────────────────────────────────────────
    // Ensure native DLLs (symlinked during activation) are loaded before
    // Wine's builtin DLLs. Merge with any existing environment variable
    // so user-set overrides (e.g. winemenubuilder.exe=d) are preserved.
    let override_str = backend.override_env_string();
    match std::env::var("WINEDLLOVERRIDES") {
        Ok(existing) if !existing.is_empty() => {
            // Prepend backend defaults — user's overrides (appended)
            // take precedence in Wine's left-to-right last-match-wins.
            let merged = format!("{};{}", override_str, existing);
            cmd.env("WINEDLLOVERRIDES", &merged);
            info!("[spawn] WINEDLLOVERRIDES={} (merged)", merged);
        }
        _ => {
            cmd.env("WINEDLLOVERRIDES", &override_str);
            info!("[spawn] WINEDLLOVERRIDES={}", override_str);
        }
    }
}

fn find_gstreamer_dir() -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    let gst_dir = data_dir.join("tequila").join("runtimes").join("gstreamer");
    if gst_dir.is_dir() {
        Some(gst_dir)
    } else {
        None
    }
}

pub trait WineProcesses {
    fn get_wine_version(&self) -> Result<String>;
    fn start_winecfg(&self) -> Result<()>;
    fn start_regedit(&self) -> Result<()>;
    fn start_control_panel(&self) -> Result<()>;
    fn run_executable(&self, executable_path: &PathBuf) -> Result<()>;
    fn run_windows_command(&self, command: &str) -> Result<()>;
}

impl WineProcesses for base::traits::WinePrefix {
    fn get_wine_version(&self) -> Result<String> {
        let wine_prefix = self.path.to_string_lossy().to_string();
        let output = Command::new("wine")
            .env("WINEPREFIX", &wine_prefix)
            .arg("--version")
            .output()
            .map_err(|e| PrefixError::Process(format!("Failed to get wine version: {}", e)))?;
        if output.status.success() {
            let version = String::from_utf8(output.stdout).map_err(|e| {
                PrefixError::Process(format!("Failed to parse wine version: {}", e))
            })?;
            Ok(version.trim().to_string())
        } else {
            Err(PrefixError::Process(format!(
                "Failed to get wine version: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
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
            return Err(PrefixError::NotFound(format!(
                "Executable not found: {}",
                executable_path.display()
            )));
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

impl WineProcesses for crate::Manager {
    fn get_wine_version(&self) -> Result<String> {
        let output = Command::new("wine")
            .arg("--version")
            .output()
            .map_err(|e| PrefixError::Process(format!("Failed to get wine version: {}", e)))?;
        if output.status.success() {
            let version = String::from_utf8(output.stdout).map_err(|e| {
                PrefixError::Process(format!("Failed to parse wine version: {}", e))
            })?;
            Ok(version.trim().to_string())
        } else {
            Err(PrefixError::Process(format!(
                "Failed to get wine version: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
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
            return Err(PrefixError::NotFound(format!(
                "Executable not found: {}",
                executable_path.display()
            )));
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
