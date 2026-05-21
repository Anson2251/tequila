use base::config::{PrefixConfig, RegisteredExecutable};
use base::error::{Result, PrefixError};
use base::traits::{WinePrefix, PrefixInfo};
use runtime::{RuntimeManager, Runtime, Channel};
use crate::wine_processes::apply_runtime_env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fs;
use std::process::Command;

#[derive(Clone)]
pub struct Manager {
    #[allow(dead_code)]
    wine_dir: PathBuf,
    scanner: scan::ApplicationScanner,
    runtime_manager: RuntimeManager,
}

// Manual PartialEq that skips scan::ApplicationScanner (no PartialEq)
impl PartialEq for Manager {
    fn eq(&self, other: &Self) -> bool {
        self.wine_dir == other.wine_dir && self.runtime_manager == other.runtime_manager
    }
}

impl Manager {
    pub fn new(wine_dir: PathBuf, icon_cache: Arc<scan::IconCache>) -> Self {
        let mut runtime_manager = RuntimeManager::new();
        if let Some(settings) = store::Settings::load() {
            let mut rm: RuntimeManager = settings.into();
            rm.ensure_system_runtime();
            runtime_manager = rm;
        } else { runtime_manager.ensure_system_runtime(); }
        Self { wine_dir, scanner: scan::ApplicationScanner::new(icon_cache), runtime_manager }
    }

    pub fn wine_dir(&self) -> &PathBuf { &self.wine_dir }
    pub fn scanner(&self) -> &scan::ApplicationScanner { &self.scanner }
    pub fn runtime_manager(&self) -> &RuntimeManager { &self.runtime_manager }
    pub fn runtime_manager_mut(&mut self) -> &mut RuntimeManager { &mut self.runtime_manager }

    pub fn save_runtime_state(&self) {
        let settings: store::Settings = self.runtime_manager.clone().into();
        if let Err(e) = settings.save() { eprintln!("Failed to save runtime settings: {}", e); }
    }

    pub async fn download_channel_runtime(&mut self, channel: Channel, progress: runtime::download::ProgressFn) -> Result<Runtime> {
        let runtimes = runtime::download::runtimes_dir();
        runtime::download::cleanup_temp_runtimes(&runtimes);
        let bundle_dir = runtime::download::download_channel_runtime(&channel, &progress).await?;
        let cask = runtime::homebrew::fetch_cask(channel.cask_name())
            .await.map_err(|e| PrefixError::Process(e))?;
        let runtime = self.runtime_manager.register_channel(channel, cask.version, bundle_dir).clone();
        self.save_runtime_state();
        Ok(runtime)
    }

    pub fn import_runtime(&mut self, source_path: &PathBuf, label: &str) -> std::result::Result<Runtime, String> {
        let runtimes = runtime::download::runtimes_dir();
        let runtime = self.runtime_manager.import_runtime(source_path, label, &runtimes)?;
        self.save_runtime_state();
        Ok(runtime)
    }

    pub fn remove_runtime(&mut self, id: &str) { self.runtime_manager.remove(id); self.save_runtime_state(); }
    pub fn set_default_runtime(&mut self, id: &str) { self.runtime_manager.set_default(id); self.save_runtime_state(); }

    fn runtime_for_prefix(&self, config: &PrefixConfig) -> Option<&Runtime> {
        self.runtime_manager.resolve(config.wine_version.as_deref())
    }

    fn build_wine_command(&self, config: &PrefixConfig, prefix_path: &Path) -> Command {
        let mut cmd = Command::new("wine");
        if let Some(runtime) = self.runtime_for_prefix(config) { apply_runtime_env(&mut cmd, runtime, prefix_path); }
        else { cmd.env("WINEPREFIX", prefix_path); }
        cmd
    }

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

    fn load_or_create_config(&self, prefix_path: &PathBuf, name: &str, system_wine_version: &Option<String>) -> Result<PrefixConfig> {
        let mut config = if let Some(config) = PrefixConfig::load_from_file(prefix_path)? { config }
        else {
            let mut config = PrefixConfig::new(name.to_string(), "win64".to_string());
            if let Ok(architecture) = self.detect_architecture(prefix_path) { config.architecture = architecture; }
            config
        };
        if config.wine_version.is_none() {
            if let Some(ver) = system_wine_version { config.wine_version = Some(ver.clone()); config.save_to_file(prefix_path)?; }
        }
        Ok(config)
    }

    fn detect_architecture(&self, prefix_path: &PathBuf) -> Result<String> {
        if prefix_path.join("drive_c/Program Files (x86)").exists() { Ok("win64".to_string()) }
        else if prefix_path.join("drive_c/Program Files").exists() { Ok("win32".to_string()) }
        else { Ok("win64".to_string()) }
    }

    pub fn create_prefix(&self, name: &str, architecture: &str) -> Result<PathBuf> {
        let runtime_id = self.runtime_manager.default_id.clone();
        self.create_prefix_with_runtime(name, architecture, &runtime_id)
    }

    pub fn create_prefix_with_runtime(&self, name: &str, architecture: &str, runtime_id: &str) -> Result<PathBuf> {
        let prefix_path = self.wine_dir.join(name);
        if prefix_path.exists() { return Err(PrefixError::AlreadyExists(format!("Prefix '{}' already exists", name))); }
        fs::create_dir_all(&prefix_path)?;
        let mut config = PrefixConfig::new(name.to_string(), architecture.to_string());
        config.wine_version = Some(runtime_id.to_string());
        config.save_to_file(&prefix_path)?;
        let wine_arch = if architecture == "win32" { "win32" } else { "win64" };
        let mut cmd = self.build_wine_command_with_args(&["cmd", "/c", "echo hello, world"], &config, &prefix_path);
        cmd.env("WINEARCH", wine_arch);
        cmd.env("DISPLAY", "");
        cmd.env("WINEDEBUG", "-all");
        let output = cmd.output().map_err(|e| PrefixError::Process(format!("Failed to run wine: {}", e)))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("hello, world") {
            let _ = fs::remove_dir_all(&prefix_path);
            return Err(PrefixError::Wine(format!("Prefix bootstrap failed: expected 'hello, world' in output, got: {}", stdout.trim())));
        }
        Ok(prefix_path)
    }

    fn build_wine_command_for_exe(&self, exe: &str, config: &PrefixConfig, prefix_path: &Path) -> Command {
        if let Some(runtime) = self.runtime_for_prefix(config) {
            let mut cmd = Command::new(exe);
            apply_runtime_env(&mut cmd, runtime, prefix_path);
            cmd
        } else { let mut cmd = Command::new(exe); cmd.env("WINEPREFIX", prefix_path); cmd }
    }

    fn build_wine_command_with_args(&self, args: &[&str], config: &PrefixConfig, prefix_path: &Path) -> Command {
        let mut cmd = if let Some(runtime) = self.runtime_for_prefix(config) {
            let mut cmd = Command::new("wine");
            apply_runtime_env(&mut cmd, runtime, prefix_path);
            cmd
        } else { let mut cmd = Command::new("wine"); cmd.env("WINEPREFIX", prefix_path); cmd };
        for arg in args { cmd.arg(arg); }
        cmd
    }

    pub fn delete_prefix(&self, prefix_path: &PathBuf) -> Result<()> {
        if !prefix_path.exists() { return Err(PrefixError::NotFound("Prefix does not exist".to_string())); }
        if !self.is_valid_wine_prefix(prefix_path) { return Err(PrefixError::Validation("Not a valid Wine prefix".to_string())); }
        fs::remove_dir_all(prefix_path)?;
        Ok(())
    }

    pub fn scan_for_applications(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        executables.extend(self.scanner.scan_prefix(prefix_path)?);
        executables.extend(self.scanner.scan_for_desktop_files(prefix_path)?);
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        Ok(executables)
    }

    pub async fn scan_for_applications_async(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        executables.extend(self.scanner.scan_prefix_async(prefix_path).await?);
        executables.extend(self.scanner.scan_for_desktop_files_async(prefix_path).await?);
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        Ok(executables)
    }

    pub fn update_config(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<()> {
        config.validate()?;
        let mut updated_config = config.clone();
        updated_config.update_last_modified();
        updated_config.save_to_file(prefix_path)?;
        Ok(())
    }

    pub fn add_executable_to_prefix(&self, prefix_path: &PathBuf, executable: RegisteredExecutable) -> Result<()> {
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, name, &None)?;
        config.add_executable(executable);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn remove_executable_from_prefix(&self, prefix_path: &PathBuf, index: usize) -> Result<()> {
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, name, &None)?;
        config.remove_executable(index);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn launch_executable(&self, prefix_path: &PathBuf, executable: &RegisteredExecutable) -> Result<()> {
        if !executable.executable_path.exists() { return Err(PrefixError::NotFound("Executable file does not exist".to_string())); }
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        self.build_wine_command_with_args(&[&executable.executable_path.to_string_lossy()], &config, prefix_path)
            .current_dir(prefix_path).spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to launch executable: {}", e)))?;
        Ok(())
    }

    pub fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<()> {
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        self.build_wine_command_for_exe("winecfg", &config, prefix_path)
            .current_dir(prefix_path).spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run winecfg: {}", e)))?;
        Ok(())
    }

    pub fn run_regedit(&self, prefix_path: &PathBuf) -> Result<()> {
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        self.build_wine_command_with_args(&["regedit"], &config, prefix_path)
            .current_dir(prefix_path).spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run regedit: {}", e)))?;
        Ok(())
    }

    pub fn get_prefix_info(&self, prefix_path: &PathBuf) -> Result<PrefixInfo> {
        let name = prefix_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        let size = self.calculate_prefix_size(prefix_path)?;
        Ok(PrefixInfo {
            name: config.name.clone(), path: prefix_path.clone(), size,
            executable_count: config.get_executable_count(),
            wine_version: config.wine_version.clone(), architecture: config.architecture.clone(),
            creation_date: config.creation_date, last_modified: config.last_modified,
        })
    }

    pub fn enrich_executables(&self, config: &mut PrefixConfig) -> bool {
        let ic = self.scanner.icon_cache();
        let mut changed = false;
        for exe in &mut config.registered_executables {
            if let Some(icon_path) = scan::extract_icon_for_exe(&exe.executable_path, ic) {
                if exe.icon_path.as_ref() != Some(&icon_path) { exe.icon_path = Some(icon_path); changed = true; }
            }
            if exe.file_description.is_none() {
                let meta = scan::extract_metadata_for_exe(&exe.executable_path);
                if meta.file_version.is_some() || meta.file_description.is_some() {
                    exe.file_version = meta.file_version; exe.product_version = meta.product_version;
                    exe.company_name = meta.company_name; exe.file_description = meta.file_description;
                    exe.product_name = meta.product_name; exe.imported_modules = meta.imported_modules;
                    changed = true;
                }
            }
        }
        changed
    }

    fn calculate_prefix_size(&self, prefix_path: &PathBuf) -> Result<u64> {
        let total_size = walkdir::WalkDir::new(prefix_path)
            .into_iter().flatten()
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| entry.metadata().ok())
            .map(|metadata| metadata.len())
            .sum();
        Ok(total_size)
    }
}
