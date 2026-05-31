use base::config::PrefixConfig;
use base::error::{PrefixError, Result};
use base::traits::WinePrefix;
use base::{GraphicsBackend, GraphicsConfig};
use log::{error, info, warn};
use registry::keys::DllOverrideSetting;
use registry::{InMemoryRegistryCache, RegEditor, RegistryEditor};
use runtime::graphics;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::Manager;

impl Manager {
    pub fn scan_prefixes(&self) -> Result<Vec<WinePrefix>> {
        let mut prefixes: Vec<WinePrefix> = Vec::new();
        let system_runtime = self.runtime_manager.get("wine-system");
        let system_wine_version = system_runtime.map(|r| r.wine_version.clone());
        for entry in fs::read_dir(&self.wine_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && self.is_valid_wine_prefix(&path) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(mut config) =
                        self.load_or_create_config(&path, name, &system_wine_version)
                    {
                        if let Some(ref ver) = system_wine_version {
                            if config.wine_version.as_ref() != Some(ver) {
                                config.wine_version = Some(ver.clone());
                                let _ = config.save_to_file(&path);
                            }
                        }
                        prefixes.push(WinePrefix {
                            name: name.to_string(),
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

    fn is_valid_wine_prefix(&self, path: &PathBuf) -> bool {
        path.join("drive_c").exists()
            && path.join("system.reg").exists()
            && path.join("user.reg").exists()
    }

    pub fn load_or_create_config(
        &self,
        prefix_path: &PathBuf,
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

    fn detect_architecture(&self, prefix_path: &PathBuf) -> Result<String> {
        if prefix_path.join("drive_c/Program Files (x86)").exists() {
            Ok("win64".to_string())
        } else if prefix_path.join("drive_c/Program Files").exists() {
            Ok("win32".to_string())
        } else {
            Ok("win64".to_string())
        }
    }

    pub fn create_prefix(&self, name: &str, architecture: &str) -> Result<PathBuf> {
        let runtime_id = self.runtime_manager.default_id.clone();
        self.create_prefix_with_runtime(name, architecture, &runtime_id)
    }

    pub fn create_prefix_with_runtime(
        &self,
        name: &str,
        architecture: &str,
        runtime_id: &str,
    ) -> Result<PathBuf> {
        let prefix_path = self.wine_dir.join(name);
        if prefix_path.exists() {
            return Err(PrefixError::AlreadyExists(format!(
                "Prefix '{}' already exists",
                name
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
    pub fn reinitialize_prefix(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<()> {
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

    pub fn delete_prefix(&self, prefix_path: &PathBuf) -> Result<()> {
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
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;

        info!(
            "[prefix] Activating {} for prefix '{}' (arch: {})",
            backend.display_name(),
            name,
            config.architecture
        );

        if !backend.supports_arch(&config.architecture) {
            warn!(
                "[prefix] {} requires 64-bit prefix, but '{}' is {}",
                backend.display_name(),
                name,
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
            "[prefix] Symlinked DLLs for {} into prefix '{}'\n",
            backend.display_name(),
            name
        );

        // 2. Write DLL overrides to registry
        let cache = Arc::new(InMemoryRegistryCache::new(Duration::from_secs(30)));
        let mut editor = RegistryEditor::with_prefix(cache, prefix_path).await?;
        let entries: Vec<&str> = backend
            .override_entries()
            .iter()
            .map(|(dll, _)| *dll)
            .collect();
        info!(
            "[prefix] Writing DLL overrides to registry: {}=native,builtin",
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
        let mut config = self.load_or_create_config(prefix_path, name, &None)?;
        config.graphics = Some(gfx_config.clone());
        config.update_last_modified();
        config.save_to_file(prefix_path)?;

        info!(
            "[prefix] Successfully activated {} for prefix '{}'",
            backend.display_name(),
            name
        );
        Ok(gfx_config)
    }

    /// Deactivate the current graphics backend for a prefix.
    ///
    /// 1. Remove DLL symlinks from prefix
    /// 2. Remove DLL override entries from `user.reg`
    /// 3. Clear `graphics` field from `tequila-config.json`
    pub async fn deactivate_graphics_backend(&self, prefix_path: &PathBuf) -> Result<()> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, name, &None)?;

        if let Some(gfx_config) = config.graphics.take() {
            info!(
                "[prefix] Deactivating {} for prefix '{}'",
                gfx_config.display_name(),
                name
            );

            // 1. Remove DLL symlinks
            graphics::deactivate_for_prefix(&gfx_config, prefix_path)?;
            info!("[prefix] Removed DLL symlinks for prefix '{}'", name);

            // 2. Remove registry overrides
            let cache = Arc::new(InMemoryRegistryCache::new(Duration::from_secs(30)));
            let mut editor = RegistryEditor::with_prefix(cache, prefix_path).await?;
            let dlls: Vec<&str> = gfx_config.override_dlls();
            info!(
                "[prefix] Removing DLL overrides from registry: {}",
                dlls.join(",")
            );
            for dll in gfx_config.override_dlls() {
                editor.remove_dll_override(dll).await?;
            }
            editor.save_registry(prefix_path).await?;

            // 3. Clear config
            config.graphics = None;
            config.update_last_modified();
            config.save_to_file(prefix_path)?;
        }

        Ok(())
    }
}
