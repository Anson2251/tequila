use base::config::PrefixConfig;
use base::error::{PrefixError, Result};
use base::traits::WinePrefix;
use base::{GraphicsBackend, GraphicsConfig};
use log::{info, warn};
use registry::keys::DllOverrideSetting;
use registry::{RegEditor, RegistryEditor};
use runtime::graphics;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::Manager;
use crate::wine_processes::apply_runtime_env;

/// File extension for Tequila prefix archives (after `.zst`).
/// Full filename: `<prefix_name>.zst.wtea`
pub const TQL_EXTENSION: &str = "wtea";

/// Recursively copy a directory tree, preserving symlinks.
fn copy_dir_recursive(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if ty.is_symlink() {
            let link_target = fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(&link_target, &target)?;
        } else {
            match fs::copy(entry.path(), &target) {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    // Make the source readable and retry
                    let mut perms = entry.path().metadata()?.permissions();
                    perms.set_mode(0o644);
                    fs::set_permissions(entry.path(), perms)?;
                    fs::copy(entry.path(), &target)?;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok(())
}

impl Manager {
    /// Open an existing prefix, loading its configuration.
    ///
    /// Returns a [`Prefix`](crate::Prefix) with shared access to the
    /// manager's scanner, runtime manager, and store.
    pub fn open_prefix(&self, prefix_path: &Path) -> Result<crate::Prefix> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let system_wine_version = self
            .read_runtime()
            .get("wine-system")
            .map(|r| r.wine_version.clone());
        let config = self.load_or_create_config(prefix_path, name, &system_wine_version)?;
        Ok(crate::Prefix {
            path: prefix_path.to_path_buf(),
            config,
            scanner: Arc::clone(&self.scanner),
            runtime_manager: Arc::clone(&self.runtime_manager),
            store: Arc::clone(&self.store),
        })
    }

    pub fn scan_prefixes(&self) -> Result<Vec<WinePrefix>> {
        let mut prefixes: Vec<WinePrefix> = Vec::new();
        let system_wine_version = self
            .read_runtime()
            .get("wine-system")
            .map(|r| r.wine_version.clone());
        for entry in fs::read_dir(&self.wine_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && self.is_valid_wine_prefix(&path) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(config) =
                        self.load_or_create_config(&path, name, &system_wine_version)
                    {
                        prefixes.push(WinePrefix {
                            name: config.name.clone(),
                            path: path.clone(),
                            config,
                        });
                    }
                }
            }
        }
        prefixes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(prefixes)
    }

    fn is_valid_wine_prefix(&self, path: &Path) -> bool {
        path.join("drive_c").exists()
            && path.join("system.reg").exists()
            && path.join("user.reg").exists()
    }

    pub fn load_or_create_config(
        &self,
        prefix_path: &Path,
        name: &str,
        system_wine_version: &Option<String>,
    ) -> Result<PrefixConfig> {
        let mut config = if let Some(config) = PrefixConfig::load_from_file(prefix_path)? {
            config
        } else {
            let mut config = PrefixConfig::new(name.to_string(), "win64".to_string());
            if let Ok(architecture) = self.detect_architecture(prefix_path) {
                config.architecture = architecture;
            }
            config
        };
        if config.wine_version.is_none() {
            if let Some(ver) = system_wine_version {
                config.wine_version = Some(ver.clone());
                config.save_to_file(prefix_path)?;
            }
        }
        Ok(config)
    }

    fn detect_architecture(&self, prefix_path: &Path) -> Result<String> {
        if prefix_path.join("drive_c/Program Files (x86)").exists() {
            Ok("win64".to_string())
        } else if prefix_path.join("drive_c/Program Files").exists() {
            Ok("win32".to_string())
        } else {
            Ok("win64".to_string())
        }
    }

    pub fn create_prefix(&self, name: &str, architecture: &str) -> Result<PathBuf> {
        let runtime_id = self.read_runtime().default_id.clone();
        self.create_prefix_with_runtime(name, architecture, &runtime_id)
    }

    pub fn create_prefix_with_runtime(
        &self,
        name: &str,
        architecture: &str,
        runtime_id: &str,
    ) -> Result<PathBuf> {
        let dir_name = Uuid::new_v4().to_string();
        let prefix_path = self.wine_dir.join(&dir_name);
        if prefix_path.exists() {
            return Err(PrefixError::AlreadyExists(format!(
                "Prefix '{}' already exists",
                dir_name
            )));
        }
        fs::create_dir_all(&prefix_path)?;
        let mut config = PrefixConfig::new(name.to_string(), architecture.to_string());
        config.wine_version = Some(runtime_id.to_string());
        config.save_to_file(&prefix_path).map_err(|e| {
            let _ = fs::remove_dir_all(&prefix_path);
            e
        })?;
        self.reinitialize_prefix(&prefix_path, &config)
            .map_err(|e| {
                let _ = fs::remove_dir_all(&prefix_path);
                e
            })?;
        Ok(prefix_path)
    }

    /// Re-initialize an existing prefix with the Wine version specified in
    /// `config.wine_version`.  This runs `wine cmd /c echo hello, world` to
    /// trigger Wine's prefix creation/update machinery.
    ///
    /// The prefix directory must already exist on disk.
    pub fn reinitialize_prefix(&self, prefix_path: &Path, config: &PrefixConfig) -> Result<()> {
        let wine_arch = if config.architecture == "win32" {
            "win32"
        } else {
            "win64"
        };

        let mut cmd = self.build_wine_command_with_args(
            &["cmd", "/c", "echo hello, world"],
            config,
            prefix_path,
        );
        cmd.env("WINEARCH", wine_arch);
        cmd.env("DISPLAY", "");
        cmd.env("WINEDEBUG", "-all");

        self.check_wine_available("wine", config)?;

        let output = cmd
            .output()
            .map_err(|e| PrefixError::Process(format!("Failed to reinitialize prefix: {}", e)))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("hello, world") {
            return Err(PrefixError::Wine(format!(
                "Prefix reinitialization failed: expected 'hello, world' in output, got: {}",
                stdout.trim()
            )));
        }
        Ok(())
    }

    pub fn delete_prefix(&self, prefix_path: &Path) -> Result<()> {
        if !prefix_path.exists() {
            return Err(PrefixError::NotFound("Prefix does not exist".to_string()));
        }
        if !self.is_valid_wine_prefix(prefix_path) {
            return Err(PrefixError::Validation(
                "Not a valid Wine prefix".to_string(),
            ));
        }
        fs::remove_dir_all(prefix_path)?;
        Ok(())
    }

    /// Activate a graphics backend for a prefix.
    ///
    /// 1. Symlink backend `.dll` files into prefix's `system32/` (and `syswow64/`)
    /// 2. Write DLL override entries to `user.reg`
    /// 3. Save `graphics` field to `tequila-config.json`
    pub async fn activate_graphics_backend(
        &self,
        backend: &GraphicsBackend,
        prefix_path: &PathBuf,
    ) -> Result<GraphicsConfig> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        info!(
            "[prefix] activating {} for prefix '{}' (arch: {})",
            backend.display_name(),
            config.name,
            config.architecture
        );

        if !backend.supports_arch(&config.architecture) {
            warn!(
                "[prefix] {} requires 64-bit prefix, but '{}' is {}",
                backend.display_name(),
                config.name,
                config.architecture
            );
            return Err(PrefixError::Validation(format!(
                "{} requires a 64-bit prefix (current: {})",
                backend.display_name(),
                config.architecture
            )));
        }

        // 1. Symlink backend DLLs into prefix
        let gfx_config = graphics::activate_for_prefix(backend, prefix_path)?;
        info!(
            "[prefix] symlinked DLLs for {} into prefix '{}'\n",
            backend.display_name(),
            config.name
        );

        // 2. Write DLL overrides to registry
        let mut editor = RegistryEditor::with_prefix(prefix_path).await?;
        let entries: Vec<&str> = backend
            .override_entries()
            .iter()
            .map(|(dll, _)| *dll)
            .collect();
        info!(
            "[prefix] writing DLL overrides to registry: {}=native,builtin",
            entries.join(",")
        );
        for (dll, setting_str) in backend.override_entries() {
            let setting = DllOverrideSetting::from_string(setting_str).ok_or_else(|| {
                PrefixError::Validation(format!("Invalid override setting: {}", setting_str))
            })?;
            editor.add_dll_override(dll, setting).await?;
        }
        editor.save_registry(prefix_path).await?;

        // 3. Save to tequila-config.json
        let mut config = self.load_or_create_config(prefix_path, dir_name, &None)?;
        config.graphics = Some(gfx_config.clone());
        config.update_last_modified();
        config.save_to_file(prefix_path)?;

        info!(
            "[prefix] successfully activated {} for prefix '{}'",
            backend.display_name(),
            config.name
        );
        Ok(gfx_config)
    }

    /// Unified DXVK+VKD3D patch: download latest releases + activate for prefix.
    ///
    /// This is the equivalent of `gameportingtoolkit patch` for the DXVK+VKD3D
    /// backend.  It performs the complete patching workflow:
    ///
    /// 1. Fetch the latest DXVK release from GitHub and download it
    /// 2. Fetch the latest VKD3D-Proton release from GitHub and download it
    /// 3. Symlink all DLLs into the prefix (`system32/` and `syswow64/`)
    /// 4. Write `dxvk.conf` and `vkd3d_proton.conf` with sensible defaults
    /// 5. Create the DXVK state cache directory
    /// 6. Write DLL override entries to `user.reg`
    /// 7. Save the `graphics` field to `tequila-config.json`
    pub async fn patch_prefix_with_dxvk_vkd3d(
        &self,
        prefix_path: &PathBuf,
        progress: runtime::download::PhaseProgressFn,
        cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<GraphicsConfig> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        info!(
            "[prefix] patching prefix '{}' with DXVK+VKD3D (arch: {})",
            config.name, config.architecture
        );

        if !(GraphicsBackend::DxvkVkd3d {
            dxvk_version: String::new(),
            vkd3d_version: String::new(),
        })
        .supports_arch(&config.architecture)
        {
            return Err(PrefixError::Validation(format!(
                "DXVK+VKD3D does not support architecture '{}'",
                config.architecture
            )));
        }

        // 1–2. Download latest DXVK + VKD3D
        let (dxvk_dir, vkd3d_dir, dxvk_ver, vkd3d_ver) =
            runtime::graphics::download_dxvk_vkd3d(progress, cancel, &crate::github_client()).await?;

        info!(
            "[prefix] downloaded DXVK {} and VKD3D-Proton {}",
            dxvk_ver, vkd3d_ver
        );

        // 3–5. Apply the full patch (symlinks + configs + state cache)
        runtime::graphics::patch_dxvk_vkd3d_for_prefix(&dxvk_dir, &vkd3d_dir, prefix_path)?;
        info!(
            "[prefix] patched DXVK+VKD3D DLLs and config into prefix '{}'",
            config.name
        );

        // 6. Write DLL overrides to registry
        let backend = GraphicsBackend::DxvkVkd3d {
            dxvk_version: dxvk_ver.clone(),
            vkd3d_version: vkd3d_ver.clone(),
        };
        let mut editor = RegistryEditor::with_prefix(prefix_path).await?;
        for (dll, setting_str) in backend.override_entries() {
            let setting = DllOverrideSetting::from_string(setting_str).ok_or_else(|| {
                PrefixError::Validation(format!("Invalid override setting: {}", setting_str))
            })?;
            editor.add_dll_override(dll, setting).await?;
        }
        editor.save_registry(prefix_path).await?;
        info!(
            "[prefix] wrote DLL overrides to registry for prefix '{}'",
            config.name
        );

        // 7. Save config
        let backend_name: &str = "dxvk-vkd3d";
        let gfx_config = GraphicsConfig {
            backend: backend_name.to_string(),
            version: format!("dxvk-{}+vkd3d-{}", dxvk_ver, vkd3d_ver),
        };
        config.graphics = Some(gfx_config.clone());
        config.update_last_modified();
        config.save_to_file(prefix_path)?;

        info!(
            "[prefix] successfully patched prefix '{}' with DXVK+VKD3D",
            config.name
        );
        Ok(gfx_config)
    }

    /// Deactivate the current graphics backend for a prefix.
    ///
    /// 1. Remove DLL symlinks from prefix
    /// 2. Remove DLL override entries from `user.reg`
    /// 3. Clear `graphics` field from `tequila-config.json`
    ///
    /// `old_graphics` is the backend to deactivate.  When `Some`, it is
    /// used directly without re-reading the config from disk (the disk
    /// config may already have been updated by the time this runs).
    /// When `None`, falls back to reading from the prefix config file.
    pub async fn deactivate_graphics_backend(
        &self,
        prefix_path: &PathBuf,
        old_graphics: Option<GraphicsConfig>,
    ) -> Result<()> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        // Use the caller-provided config, or fall back to loading from disk
        let gfx_config = match old_graphics {
            Some(g) => g,
            None => match config.graphics.take() {
                Some(g) => g,
                None => {
                    info!("[prefix] no active graphics backend to deactivate");
                    return Ok(());
                }
            },
        };
        info!(
            "[prefix] deactivating {} for prefix '{}'",
            gfx_config.display_name(),
            config.name
        );

        // 1. Remove DLL symlinks
        info!("[prefix]   step 1/3: removing DLL symlinks...");
        graphics::deactivate_for_prefix(&gfx_config, prefix_path)?;
        info!("[prefix]   step 1/3: done");

        // 2. Remove registry overrides
        info!("[prefix]   step 2/3: initialising registry editor...");
        let mut editor = RegistryEditor::with_prefix(prefix_path).await?;
        let dlls: Vec<&str> = gfx_config.override_dlls();
        info!(
            "[prefix]   step 2/3: removing overrides: {}",
            dlls.join(",")
        );
        for dll in gfx_config.override_dlls() {
            editor.remove_dll_override(dll).await?;
        }
        info!("[prefix]   step 2/3: saving registry...");
        editor.save_registry(prefix_path).await?;
        info!("[prefix]   step 2/3: done");

        // 3. Clear config
        info!("[prefix]   step 3/3: clearing config...");
        config.graphics = None;
        config.update_last_modified();
        config.save_to_file(prefix_path)?;
        info!("[prefix]   step 3/3: done");

        info!(
            "[prefix] done deactivating {} for prefix '{}'",
            gfx_config.display_name(),
            config.name
        );
        Ok(())
    }

    /// Unpatch DXVK+VKD3D from a prefix — symmetric counterpart of
    /// `patch_prefix_with_dxvk_vkd3d`.
    ///
    /// 1. Remove DXVK+VKD3D DLL symlinks from `system32/` and `syswow64/`
    /// 2. Remove `dxvk.conf` and `vkd3d_proton.conf` (Tequila-generated only)
    /// 3. Delete the DXVK state cache directory
    /// 4. Remove DLL override entries from `user.reg`
    /// 5. Clear the `graphics` field from `tequila-config.json`
    ///
    /// This is safe to call even if the prefix has no DXVK+VKD3D backend
    /// configured — it returns `Ok(())` without doing anything.
    pub async fn unpatch_prefix_with_dxvk_vkd3d(&self, prefix_path: &PathBuf) -> Result<()> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        // Only proceed if the prefix currently has a DXVK+VKD3D backend
        let gfx_config = match config.graphics.take() {
            Some(g) if g.backend == "dxvk-vkd3d" => g,
            Some(g) => {
                // Different backend — put it back and bail
                config.graphics = Some(g);
                return Ok(());
            }
            None => return Ok(()),
        };

        info!(
            "[prefix] unpatching DXVK+VKD3D for prefix '{}'",
            config.name
        );

        // 1. Remove DLL symlinks + config files + state cache
        runtime::graphics::deactivate_for_prefix(&gfx_config, prefix_path)?;
        info!(
            "[prefix] removed DXVK+VKD3D DLL symlinks and config for '{}'",
            config.name
        );

        // 2. Remove DLL overrides from registry
        let mut editor = RegistryEditor::with_prefix(prefix_path).await?;
        for dll in gfx_config.override_dlls() {
            editor.remove_dll_override(dll).await?;
        }
        editor.save_registry(prefix_path).await?;
        info!(
            "[prefix] removed DLL overrides from registry for '{}'",
            config.name
        );

        // 3. Clear graphics field in config (graphics already taken above)
        config.update_last_modified();
        config.save_to_file(prefix_path)?;

        info!(
            "[prefix] successfully unpatched DXVK+VKD3D from '{}'",
            config.name
        );
        Ok(())
    }

    // ── DXMT patch / unpatch ─────────────────────────────────────

    /// Patch a prefix with DXMT: download latest release + activate.
    ///
    /// 1. Fetch the latest DXMT release from GitHub and download it
    /// 2. Symlink DXMT DLLs into the prefix `system32/`
    /// 3. Write DLL override entries to `user.reg`
    /// 4. Save the `graphics` field to `tequila-config.json`
    pub async fn patch_prefix_with_dxmt(
        &self,
        prefix_path: &PathBuf,
        progress: runtime::download::PhaseProgressFn,
        cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<GraphicsConfig> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        info!(
            "[prefix] patching prefix '{}' with DXMT (arch: {})",
            config.name, config.architecture
        );

        if config.architecture != "win64" {
            return Err(PrefixError::Validation(format!(
                "DXMT requires a 64-bit prefix (current: {})",
                config.architecture
            )));
        }

        // 1. Download latest DXMT
        let (version, url) = runtime::graphics::fetch_dxmt_release(&crate::github_client()).await?;

        if let Some(ref cancel) = cancel {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(PrefixError::Process("Cancelled".to_string()));
            }
        }

        // Wrap PhaseProgressFn in Arc<Mutex<>> for use as &ProgressFn
        let progress = std::sync::Arc::new(std::sync::Mutex::new(progress));
        let p = std::sync::Arc::clone(&progress);
        let simple_prog: runtime::download::ProgressFn = Box::new(move |d, t| {
            let cb = p.lock().unwrap();
            cb(d, t, runtime::download::InstallPhase::Download);
        });
        let dxmt_dir = runtime::graphics::download_dxmt(&version, &url, &simple_prog).await?;
        info!("[prefix] downloaded DXMT {}", version);

        // 2. Activate (symlink DLLs)
        runtime::graphics::activate_dxmt_for_prefix(&dxmt_dir, prefix_path)?;
        info!("[prefix] activated DXMT DLLs for prefix '{}'", config.name);

        // 3. Write registry overrides
        let backend = GraphicsBackend::Dxmt {
            version: version.clone(),
        };
        let mut editor = RegistryEditor::with_prefix(prefix_path).await?;
        for (dll, setting_str) in backend.override_entries() {
            let setting = DllOverrideSetting::from_string(setting_str).ok_or_else(|| {
                PrefixError::Validation(format!("Invalid override setting: {}", setting_str))
            })?;
            editor.add_dll_override(dll, setting).await?;
        }
        editor.save_registry(prefix_path).await?;
        info!(
            "[prefix] wrote DLL overrides to registry for prefix '{}'",
            config.name
        );

        // 4. Save config
        let gfx_config = GraphicsConfig {
            backend: "dxmt".to_string(),
            version: version.clone(),
        };
        config.graphics = Some(gfx_config.clone());
        config.update_last_modified();
        config.save_to_file(prefix_path)?;

        info!(
            "[prefix] successfully patched prefix '{}' with DXMT",
            config.name
        );
        Ok(gfx_config)
    }

    /// Unpatch DXMT from a prefix.
    pub async fn unpatch_prefix_with_dxmt(&self, prefix_path: &PathBuf) -> Result<()> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        let gfx_config = match config.graphics.take() {
            Some(g) if g.backend == "dxmt" => g,
            Some(g) => {
                config.graphics = Some(g);
                return Ok(());
            }
            None => return Ok(()),
        };

        info!("[prefix] unpatching DXMT for prefix '{}'", config.name);

        // 1. Remove DLL symlinks
        runtime::graphics::deactivate_for_prefix(&gfx_config, prefix_path)?;
        info!("[prefix] removed DXMT DLL symlinks for '{}'", config.name);

        // 2. Remove registry overrides
        let mut editor = RegistryEditor::with_prefix(prefix_path).await?;
        for dll in gfx_config.override_dlls() {
            editor.remove_dll_override(dll).await?;
        }
        editor.save_registry(prefix_path).await?;

        // 3. Clear config
        config.update_last_modified();
        config.save_to_file(prefix_path)?;

        info!(
            "[prefix] successfully unpatched DXMT from '{}'",
            config.name
        );
        Ok(())
    }

    // ── D3DMetal patch / unpatch ────────────────────────────────

    /// Patch a prefix with D3DMetal (GPTK).
    ///
    /// `source_path` can be:
    /// - A `.dmg` file (Apple Game Porting Toolkit) — will be mounted and imported
    /// - A directory containing a `lib/` tree — copied directly
    pub async fn patch_prefix_with_d3dmetal(
        &self,
        prefix_path: &PathBuf,
        source_path: &std::path::Path,
    ) -> Result<GraphicsConfig> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        info!(
            "[prefix] patching prefix '{}' with D3DMetal (arch: {})",
            config.name, config.architecture
        );

        if config.architecture != "win64" {
            return Err(PrefixError::Validation(format!(
                "D3DMetal requires a 64-bit prefix (current: {})",
                config.architecture
            )));
        }

        // 1. Import D3DMetal from DMG or directory.
        // Both branches involve blocking I/O (hdiutil, cp), so run on a
        // blocking thread pool to avoid stalling the async executor.
        let source_owned = source_path.to_path_buf();
        let d3dmetal_dir = tokio::task::spawn_blocking(move || -> Result<PathBuf> {
            if source_owned
                .extension()
                .map(|e| e == "dmg")
                .unwrap_or(false)
            {
                runtime::graphics::import_d3dmetal_from_dmg(&source_owned)
            } else {
                let gfx_dir = runtime::graphics::graphics_dir();
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let dest = gfx_dir.join(format!("d3dmetal-imported-{}", ts));
                let lib = [
                    source_owned.join("lib"),
                    source_owned.join("redist").join("lib"),
                ]
                .iter()
                .find(|p| p.is_dir())
                .cloned()
                .ok_or_else(|| {
                    PrefixError::NotFound(
                        "Could not find D3DMetal lib directory in the selected path".to_string(),
                    )
                })?;
                std::fs::create_dir_all(&dest)?;
                let status = std::process::Command::new("cp")
                    .arg("-R")
                    .arg(&lib)
                    .arg(&dest)
                    .status()
                    .map_err(|e| PrefixError::Process(format!("cp failed: {}", e)))?;
                if !status.success() {
                    return Err(PrefixError::Process(
                        "Failed to copy D3DMetal files".to_string(),
                    ));
                }
                Ok(dest)
            }
        })
        .await
        .map_err(|e| PrefixError::Process(format!("Blocking task failed: {}", e)))??;
        info!("[prefix] imported D3DMetal for prefix '{}'", config.name);

        // 2. Activate (symlink DLLs + frameworks)
        runtime::graphics::activate_d3dmetal_for_prefix(&d3dmetal_dir, prefix_path)?;
        info!(
            "[prefix] activated D3DMetal DLLs for prefix '{}'",
            config.name
        );

        // 3. Write registry overrides
        let version = d3dmetal_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("imported")
            .trim_start_matches("d3dmetal-")
            .to_string();
        let backend = GraphicsBackend::D3DMetal {
            version: version.clone(),
        };
        let mut editor = RegistryEditor::with_prefix(prefix_path).await?;
        for (dll, setting_str) in backend.override_entries() {
            let setting = DllOverrideSetting::from_string(setting_str).ok_or_else(|| {
                PrefixError::Validation(format!("Invalid override setting: {}", setting_str))
            })?;
            editor.add_dll_override(dll, setting).await?;
        }
        editor.save_registry(prefix_path).await?;
        info!(
            "[prefix] wrote DLL overrides to registry for prefix '{}'",
            config.name
        );

        // 4. Save config
        let gfx_config = GraphicsConfig {
            backend: "d3dmetal".to_string(),
            version,
        };
        config.graphics = Some(gfx_config.clone());
        config.update_last_modified();
        config.save_to_file(prefix_path)?;

        info!(
            "[prefix] successfully patched prefix '{}' with D3DMetal",
            config.name
        );
        Ok(gfx_config)
    }

    /// Unpatch D3DMetal from a prefix.
    pub async fn unpatch_prefix_with_d3dmetal(&self, prefix_path: &PathBuf) -> Result<()> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        let gfx_config = match config.graphics.take() {
            Some(g) if g.backend == "d3dmetal" => g,
            Some(g) => {
                config.graphics = Some(g);
                return Ok(());
            }
            None => return Ok(()),
        };

        info!("[prefix] unpatching D3DMetal for prefix '{}'", config.name);

        // 1. Remove DLL/framework symlinks
        runtime::graphics::deactivate_for_prefix(&gfx_config, prefix_path)?;
        info!("[prefix] removed D3DMetal symlinks for '{}'", config.name);

        // 2. Remove registry overrides
        let mut editor = RegistryEditor::with_prefix(prefix_path).await?;
        for dll in gfx_config.override_dlls() {
            editor.remove_dll_override(dll).await?;
        }
        editor.save_registry(prefix_path).await?;

        // 3. Clear config
        config.update_last_modified();
        config.save_to_file(prefix_path)?;

        info!(
            "[prefix] successfully unpatched D3DMetal from '{}'",
            config.name
        );
        Ok(())
    }

    // ── Zstd export / import ──────────────────────────────────────────

    /// Export a prefix to a compressed `.zst.wtea` archive.
    ///
    /// The archive contains:
    ///   - `drive_c/`  — the Windows drive
    ///   - `*.reg`     — registry files (`system.reg`, `user.reg`, `userdef.reg`)
    ///   - `tequila-config.json` — prefix configuration
    ///
    /// When `include_user_data` is `false`, the current user's profile directory
    /// (`drive_c/users/<current_user>/`) is skipped while `Public/`, `Default/`
    /// and other shared directories are kept.
    ///
    /// `compression_level` is passed directly to zstd (1–22, default 3).
    ///
    /// `progress` is called during compression with `(bytes_completed, bytes_total)`.
    ///
    /// If `dest_path` is a directory the file will be named
    /// `<prefix_name>.zst.wtea` inside it; otherwise it's used as-is.
    pub fn export_prefix<F>(
        &self,
        prefix_path: &PathBuf,
        dest_path: &PathBuf,
        include_user_data: bool,
        compression_level: i32,
        progress: F,
    ) -> Result<PathBuf>
    where
        F: Fn(u64, u64) + Send + 'static,
    {
        let prefix_config = PrefixConfig::load_from_file(prefix_path)?.unwrap_or_else(|| {
            let name = prefix_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("prefix")
                .to_string();
            PrefixConfig::new(name, "win64".to_string())
        });
        let prefix_name = prefix_config.name;

        let output_path = if dest_path.is_dir() {
            dest_path.join(format!("{}.zst.{}", &prefix_name, TQL_EXTENSION))
        } else {
            dest_path.clone()
        };

        // ── 1. Calculate total uncompressed size ─────────────────────
        let total_bytes = Self::calc_export_size(prefix_path, include_user_data);
        progress(0, total_bytes);

        // ── 2. Write tar.zst through a progress-wrapping writer ──────
        let file = fs::File::create(&output_path)?;
        let encoder = zstd::Encoder::new(file, compression_level)
            .map_err(|e| PrefixError::Process(format!("Failed to create zstd encoder: {}", e)))?;

        let progress_writer = ProgressWriter {
            inner: encoder,
            written: 0,
            total: total_bytes,
            callback: Box::new(progress),
        };

        let mut builder = tar::Builder::new(progress_writer);

        // Root prefix directory in the archive
        builder
            .append_dir(&prefix_name, prefix_path)
            .map_err(|e| PrefixError::Process(format!("Failed to add prefix dir: {}", e)))?;

        let drive_c = prefix_path.join("drive_c");
        if drive_c.exists() {
            let drive_arc = format!("{}/drive_c", &prefix_name);
            // Add the drive_c directory itself first
            builder
                .append_dir(&drive_arc, &drive_c)
                .map_err(|e| PrefixError::Process(format!("Failed to add drive_c dir: {}", e)))?;

            let skip_user = if include_user_data {
                None
            } else {
                let user = std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .unwrap_or_default();
                Some(format!("users/{}/", user))
            };
            Self::append_dir_filtered(&mut builder, drive_arc, &drive_c, skip_user.as_deref())?;
        }

        // Pack *.reg (raw) and tequila-config.json (paths cleaned) from root
        if let Ok(dir) = fs::read_dir(prefix_path) {
            for entry in dir.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                let archive_path = format!("{}/{}", prefix_name, name_str);

                if name_str.ends_with(".reg") {
                    let ft = entry.file_type()?;
                    if ft.is_symlink() {
                        let target = fs::read_link(&entry.path())?;
                        let mut hdr = tar::Header::new_gnu();
                        builder.append_link(&mut hdr, &archive_path, &target)?;
                    } else {
                        builder.append_path_with_name(&entry.path(), &archive_path)?;
                    }
                } else if name_str == "tequila-config.json" {
                    Self::append_clean_config(&mut builder, prefix_path, &prefix_name)?;
                }
            }
        }

        let progress_writer = builder
            .into_inner()
            .map_err(|e| PrefixError::Process(format!("Failed to finish tar archive: {}", e)))?;

        // Finish zstd compression (flush + footer)
        progress_writer
            .inner
            .finish()
            .map_err(|e| PrefixError::Process(format!("Failed to finalize zstd: {}", e)))?;

        info!(
            "[prefix] exported '{}' to {}",
            prefix_name,
            output_path.display()
        );
        Ok(output_path)
    }

    /// Walk the source tree (drive_c/ + reg files + config) and sum file sizes.
    /// Includes tar header overhead (512 B per entry + 1024 B footer).
    fn calc_export_size(prefix_path: &PathBuf, include_user_data: bool) -> u64 {
        const TAR_HEADER: u64 = 512;
        let mut total = 0u64;
        let mut entries: u64 = 0;

        // drive_c
        let drive_c = prefix_path.join("drive_c");
        if drive_c.exists() {
            let skip = if include_user_data {
                None
            } else {
                let user = std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .unwrap_or_default();
                Some(format!("users/{}/", user))
            };

            for entry in walkdir::WalkDir::new(&drive_c).into_iter().flatten() {
                if let Some(ref skip) = skip {
                    if let Ok(rel) = entry.path().strip_prefix(&drive_c) {
                        if rel.to_string_lossy().starts_with(skip.as_str()) {
                            continue;
                        }
                    }
                }
                entries += 1;
                if entry.file_type().is_file() {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }

        // reg files + config
        if let Ok(dir) = fs::read_dir(prefix_path) {
            for entry in dir.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.ends_with(".reg") || name_str == "tequila-config.json" {
                    entries += 1;
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }

        // Add tar header overhead and end-of-archive blocks
        total + entries * TAR_HEADER + 1024
    }

    /// Read the prefix's tequila-config.json, make paths relative,
    /// then write the cleaned version into the tar archive.
    fn append_clean_config<W: std::io::Write>(
        builder: &mut tar::Builder<W>,
        prefix_path: &PathBuf,
        prefix_name: &str,
    ) -> Result<()> {
        let config_path = prefix_path.join("tequila-config.json");
        if !config_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&config_path)?;
        let mut config: base::config::PrefixConfig = serde_json::from_str(&content)?;

        for exe in &mut config.registered_executables {
            // Strip the prefix root to get a prefix-relative path
            if let Ok(rel) = exe.executable_path.strip_prefix(prefix_path) {
                exe.executable_path = rel.to_path_buf();
            }
            // Icon paths are local cache references; drop them for portability
            exe.icon_path = None;
        }

        let cleaned = serde_json::to_string_pretty(&config)?;
        let archive_path = format!("{}/tequila-config.json", prefix_name);

        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(cleaned.len() as u64);
        header.set_cksum();
        builder
            .append_data(&mut header, &archive_path, cleaned.as_bytes())
            .map_err(|e| PrefixError::Process(format!("Failed to add cleaned config: {}", e)))?;

        Ok(())
    }

    /// After importing, restore absolute paths in tequila-config.json
    /// so the app manager can find executables.
    fn restore_config_paths(prefix_path: &PathBuf) -> Result<()> {
        let config_path = prefix_path.join("tequila-config.json");
        if !config_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&config_path)?;
        let mut config: base::config::PrefixConfig = serde_json::from_str(&content)?;
        let mut changed = false;
        for exe in &mut config.registered_executables {
            if exe.executable_path.is_relative() {
                exe.executable_path = prefix_path.join(&exe.executable_path);
                changed = true;
            }
        }
        if changed {
            let cleaned = serde_json::to_string_pretty(&config)?;
            fs::write(&config_path, cleaned)?;
        }
        Ok(())
    }

    fn append_dir_filtered<W: std::io::Write>(
        builder: &mut tar::Builder<W>,
        archive_prefix: String,
        src_dir: &PathBuf,
        skip_prefix: Option<&str>,
    ) -> Result<()> {
        let is_root = src_dir.join("drive_c").exists();
        for entry in walkdir::WalkDir::new(src_dir).into_iter().flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(src_dir)
                .map_err(|e| PrefixError::Process(format!("Path strip failed: {}", e)))?;
            let rel_str = rel.to_string_lossy();

            // Skip if it matches the user-data exclusion
            if let Some(skip) = skip_prefix {
                if rel_str.starts_with(skip) {
                    continue;
                }
            }

            // For the root prefix dir, only pack *.reg and tequila-config.json
            if is_root
                && rel_str != ""
                && !rel_str.ends_with(".reg")
                && rel_str != "tequila-config.json"
            {
                continue;
            }

            let archive_path = format!("{}/{}", archive_prefix, rel_str);
            let ft = entry.file_type();

            if ft.is_symlink() {
                let target = std::fs::read_link(path)?;
                let mut hdr = tar::Header::new_gnu();
                builder
                    .append_link(&mut hdr, &archive_path, &target)
                    .map_err(|e| PrefixError::Process(format!("Failed to add symlink: {}", e)))?;
            } else if ft.is_dir() && rel_str != "" && !is_root {
                builder
                    .append_dir(&archive_path, path)
                    .map_err(|e| PrefixError::Process(format!("Failed to add dir: {}", e)))?;
            } else if ft.is_file() {
                builder
                    .append_path_with_name(path, &archive_path)
                    .map_err(|e| PrefixError::Process(format!("Failed to add file: {}", e)))?;
            }
        }
        Ok(())
    }

    /// Peek inside a `.zst.wtea` archive and read the prefix name and
    /// the Wine version from its embedded config, without extracting
    /// into the wine directory.
    pub fn inspect_archive(&self, archive_path: &PathBuf) -> Result<(String, Option<String>)> {
        let data = fs::read(archive_path)?;
        let decompressed = zstd::decode_all(&data[..])
            .map_err(|e| PrefixError::Process(format!("zstd decompression failed: {}", e)))?;

        let tmp = std::env::temp_dir().join("tequila-inspect");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;

        let mut child = std::process::Command::new("tar")
            .args(["-xf", "-", "--no-same-permissions", "-C"])
            .arg(&tmp)
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&decompressed)?;
        }
        let status = child.wait()?;
        if !status.success() {
            let _ = fs::remove_dir_all(&tmp);
            return Err(PrefixError::Process("tar extraction failed".to_string()));
        }

        // Find the prefix directory
        let entries: Vec<_> = fs::read_dir(&tmp)?.flatten().collect();
        let prefix_dir = entries
            .iter()
            .find(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path());

        let (name, wine_version) = match prefix_dir {
            Some(ref dir) => {
                let config_path = dir.join("tequila-config.json");
                let config = fs::read_to_string(&config_path)
                    .ok()
                    .and_then(|c| serde_json::from_str::<base::config::PrefixConfig>(&c).ok());
                let name = config
                    .as_ref()
                    .map(|c| c.name.clone())
                    .or_else(|| {
                        dir.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "prefix".to_string());
                let wine_version = config.and_then(|c| c.wine_version);
                (name, wine_version)
            }
            None => ("prefix".to_string(), None),
        };

        let _ = fs::remove_dir_all(&tmp);
        Ok((name, wine_version))
    }

    /// Import a prefix from a `.zst.wtea` archive.
    ///
    /// The archive is decompressed and extracted into the wine directory.
    /// The prefix name is taken from the archive's top-level entry name, so
    /// renaming the file before import works as expected.
    ///
    /// After import the prefix is re-initialized with the specified
    /// `runtime_id` (pass an empty string to skip reinit).
    pub fn import_prefix(&self, archive_path: &PathBuf, runtime_id: &str) -> Result<PathBuf> {
        if !archive_path.exists() {
            return Err(PrefixError::NotFound(format!(
                "Archive not found: {}",
                archive_path.display()
            )));
        }

        let data = fs::read(archive_path)?;
        let decompressed = zstd::decode_all(&data[..])
            .map_err(|e| PrefixError::Process(format!("zstd decompression failed: {}", e)))?;

        // Extract to a temporary directory first so we can inspect the
        // top-level directory name (the prefix name).
        let tmp = std::env::temp_dir().join("tequila-import");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp)?;

        let mut child = std::process::Command::new("tar")
            .args(["-xf", "-", "--no-same-permissions", "-C"])
            .arg(&tmp)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run tar: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&decompressed)
                .map_err(|e| PrefixError::Process(format!("Failed to pipe data to tar: {}", e)))?;
        }

        let status = child
            .wait()
            .map_err(|e| PrefixError::Process(format!("tar wait failed: {}", e)))?;

        if !status.success() {
            let _ = fs::remove_dir_all(&tmp);
            return Err(PrefixError::Process("tar extraction failed".to_string()));
        }

        // Discover the prefix name from the extracted directory
        let entries: Vec<_> = fs::read_dir(&tmp)
            .map_err(|e| PrefixError::Process(format!("Failed to read temp dir: {}", e)))?
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .collect();

        let prefix_dir = match entries.as_slice() {
            [] => {
                let _ = fs::remove_dir_all(&tmp);
                return Err(PrefixError::Validation(
                    "Archive is empty — no prefix found".to_string(),
                ));
            }
            [single] => single.path(),
            _ => {
                // Multiple directories — use the name of the first valid one
                entries
                    .iter()
                    .find(|e| self.is_valid_wine_prefix(&e.path()))
                    .map(|e| e.path())
                    .unwrap_or_else(|| entries[0].path())
            }
        };

        // Read display name from config; fall back to directory name (legacy)
        let display_name = PrefixConfig::load_from_file(&prefix_dir)?
            .map(|c| c.name.clone())
            .or_else(|| {
                prefix_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "prefix".to_string());

        // Use UUID for the target directory name
        let dir_name = Uuid::new_v4().to_string();
        let target = self.wine_dir.join(&dir_name);

        // Validate the prefix before moving
        if !self.is_valid_wine_prefix(&prefix_dir) {
            let _ = fs::remove_dir_all(&tmp);
            return Err(PrefixError::Validation(format!(
                "'{}' is not a valid Wine prefix (missing drive_c or registry files)",
                display_name
            )));
        }

        // Try rename first (fast path, same filesystem); fall back to copy+remove
        if let Err(e) = fs::rename(&prefix_dir, &target) {
            if e.raw_os_error() == Some(18) {
                // EXDEV — cross-device, copy instead
                copy_dir_recursive(&prefix_dir, &target)?;
            } else {
                return Err(PrefixError::Io(e));
            }
        }
        let _ = fs::remove_dir_all(&tmp);

        // Restore absolute paths in the imported config
        if let Err(e) = Self::restore_config_paths(&target) {
            warn!("[import] failed to restore config paths: {}", e);
        }

        // Reinit the prefix with the specified runtime
        if !runtime_id.is_empty() {
            let mut config = PrefixConfig::load_from_file(&target)?
                .unwrap_or_else(|| PrefixConfig::new(display_name.clone(), "win64".to_string()));
            config.wine_version = Some(runtime_id.to_string());
            config.update_last_modified();

            if let Err(e) = self.reinitialize_prefix(&target, &config) {
                warn!("[import] reinit failed (non-fatal): {}", e);
            } else {
                let _ = config.save_to_file(&target);
            }
        }

        info!(
            "[prefix] imported '{}' from {}",
            display_name,
            archive_path.display()
        );
        Ok(target)
    }
}

// ── Progress writer ─────────────────────────────────────────────────────

/// A `Write` wrapper that calls `callback(written, total)` after every write.
struct ProgressWriter<W> {
    inner: W,
    written: u64,
    total: u64,
    callback: Box<dyn Fn(u64, u64) + Send>,
}

impl<W: std::io::Write> std::io::Write for ProgressWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.written += n as u64;
        (self.callback)(self.written, self.total);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl Manager {
    // ── Terminal helper ─────────────────────────────────────────────

    /// Generate a shell script that sets up all Wine environment variables
    /// for the given prefix (WINEPREFIX, PATH, WINEDLLPATH, WINEDLLOVERRIDES,
    /// GStreamer, etc.) and starts an interactive shell in the prefix directory.
    ///
    /// The auto-delete-on-exit trap and most boilerplate live in
    /// `scripts/tequila-terminal.sh` and are embedded at compile time via
    /// `include_str!`.
    pub fn generate_terminal_script(&self, prefix_path: &PathBuf) -> Result<String> {
        let dir_name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("prefix");
        let config = self.load_or_create_config(prefix_path, dir_name, &None)?;

        // Build a dummy Command and apply the full runtime env setup to it.
        // We then iterate over the explicitly-set env vars to generate
        // `export` statements — this reuses ALL existing env logic
        // (WINEPREFIX, PATH, WINEDLLPATH, WINEDLLOVERRIDES, GStreamer, …)
        // without any duplication.
        let mut cmd = std::process::Command::new("true");
        if let Some(runtime) = self.runtime_for_prefix(&config) {
            apply_runtime_env(&mut cmd, &runtime, prefix_path);
        } else {
            cmd.env("WINEPREFIX", prefix_path);
        }

        let mut exports = String::new();
        let mut banner = String::new();
        for (key, val) in cmd.get_envs() {
            if let Some(val) = val {
                let k = key.to_string_lossy();
                let v = val.to_string_lossy();
                let safe_val = v.replace('\'', "'\\''");
                exports.push_str(&format!("export {}='{}'\n", k, safe_val));

                if k == "PATH" {
                    banner.push_str("echo \"  PATH = ${PATH%%:*}: ...\"\n");
                } else {
                    let safe_k = k.replace('\'', "'\\''");
                    banner.push_str(&format!("echo \"  {} = ${}\"\n", safe_k, k));
                }
            }
        }

        let safe_name = config.name.replace('\'', "'\\''");
        let safe_path = prefix_path.to_string_lossy().replace('\'', "'\\''");
        let ps1 = format!("(tequila: {})", safe_name);

        let script = include_str!("../../../scripts/tequila-terminal.sh")
            .replace("__TEQUILA_EXPORTS__", &exports)
            .replace("__TEQUILA_PS1__", &ps1)
            .replace("__TEQUILA_PREFIX_PATH__", &safe_path)
            .replace("__TEQUILA_PREFIX_NAME__", &safe_name)
            .replace("__TEQUILA_BANNER__", &banner);

        Ok(script)
    }
}
