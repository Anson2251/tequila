use base::config::PrefixConfig;
use base::error::{Result, PrefixError};
use base::traits::WinePrefix;
use std::path::PathBuf;
use std::fs;

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
                    if let Ok(mut config) = self.load_or_create_config(&path, name, &system_wine_version) {
                        if let Some(ref ver) = system_wine_version {
                            if config.wine_version.as_ref() != Some(ver) {
                                config.wine_version = Some(ver.clone());
                                let _ = config.save_to_file(&path);
                            }
                        }
                        prefixes.push(WinePrefix { name: name.to_string(), path: path.clone(), config });
                    }
                }
            }
        }
        prefixes.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(prefixes)
    }

    fn is_valid_wine_prefix(&self, path: &PathBuf) -> bool {
        path.join("drive_c").exists() && path.join("system.reg").exists() && path.join("user.reg").exists()
    }

    pub(crate) fn load_or_create_config(&self, prefix_path: &PathBuf, name: &str, system_wine_version: &Option<String>) -> Result<PrefixConfig> {
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

    pub fn create_prefix_with_runtime(&self, name: &str, architecture: &str, runtime_id: &str) -> Result<PathBuf> {
        let prefix_path = self.wine_dir.join(name);
        if prefix_path.exists() {
            return Err(PrefixError::AlreadyExists(format!("Prefix '{}' already exists", name)));
        }
        fs::create_dir_all(&prefix_path)?;
        let mut config = PrefixConfig::new(name.to_string(), architecture.to_string());
        config.wine_version = Some(runtime_id.to_string());
        config.save_to_file(&prefix_path)?;
        let wine_arch = if architecture == "win32" { "win32" } else { "win64" };
        let mut cmd = self.build_wine_command_with_args(&["cmd", "/c", "echo hello, world"], &config, &prefix_path);
        cmd.env("WINEARCH", wine_arch);
        cmd.env("DISPLAY", "");
        cmd.env("WINEDEBUG", "-all");
        let output = cmd.output()
            .map_err(|e| PrefixError::Process(format!("Failed to run wine: {}", e)))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("hello, world") {
            let _ = fs::remove_dir_all(&prefix_path);
            return Err(PrefixError::Wine(format!(
                "Prefix bootstrap failed: expected 'hello, world' in output, got: {}",
                stdout.trim()
            )));
        }
        Ok(prefix_path)
    }

    pub fn delete_prefix(&self, prefix_path: &PathBuf) -> Result<()> {
        if !prefix_path.exists() {
            return Err(PrefixError::NotFound("Prefix does not exist".to_string()));
        }
        if !self.is_valid_wine_prefix(prefix_path) {
            return Err(PrefixError::Validation("Not a valid Wine prefix".to_string()));
        }
        fs::remove_dir_all(prefix_path)?;
        Ok(())
    }
}
