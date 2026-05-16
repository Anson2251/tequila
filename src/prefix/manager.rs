use crate::prefix::config::{PrefixConfig, RegisteredExecutable};
use crate::prefix::scanner::ApplicationScanner;
use crate::prefix::IconCache;
use crate::prefix::error::{Result, PrefixError};
use crate::prefix::traits::{PrefixManager as PrefixManagerTrait, WinePrefix, PrefixInfo};
use std::path::PathBuf;
use std::sync::Arc;
use std::fs;
use std::process::Command;

#[derive(Clone, PartialEq)]
pub struct Manager {
    wine_dir: PathBuf,
    scanner: ApplicationScanner,
}

impl Manager {
    /// Create a new PrefixManager with the specified wine directory and icon cache.
    pub fn new(wine_dir: PathBuf, icon_cache: Arc<IconCache>) -> Self {
        Self {
            wine_dir,
            scanner: ApplicationScanner::new(icon_cache),
        }
    }

    /// Get the wine directory
    pub fn wine_dir(&self) -> &PathBuf {
        &self.wine_dir
    }

    /// Get a reference to the scanner
    pub fn scanner(&self) -> &ApplicationScanner {
        &self.scanner
    }

    pub fn scan_prefixes(&self) -> Result<Vec<WinePrefix>> {
        let mut prefixes: Vec<WinePrefix> = Vec::new();

        println!("Scanning Wine prefixes in directory: {:?}", self.wine_dir);

        // Detect wine version once via `wine --version`; shared across all prefixes
        let wine_version = detect_system_wine_version().ok();

        for entry in fs::read_dir(&self.wine_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                println!("Found directory: {:?}", path);
                if self.is_valid_wine_prefix(&path) {
                    println!("✅ Valid Wine prefix: {:?}", path);
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        println!("🔧 Loading config for prefix: {}", name);
                        match self.load_or_create_config(&path, name, &wine_version) {
                            Ok(mut config) => {
                                // Always refresh wine version from system on scan
                                if let Some(ref ver) = wine_version {
                                    if config.wine_version.as_ref() != Some(ver) {
                                        config.wine_version = Some(ver.clone());
                                        let _ = config.save_to_file(&path);
                                    }
                                }
                                println!("✅ Config loaded successfully for: {}", name);
                                prefixes.push(WinePrefix {
                                    name: name.to_string(),
                                    path: path.clone(),
                                    config,
                                });
                            }
                            Err(e) => {
                                println!("❌ Failed to load config for {}: {:?}", name, e);
                            }
                        }
                    }
                } else {
                    println!("❌ Not a valid Wine prefix (missing required files): {:?}", path);
                }
            }
        }

        prefixes.sort_by(|a, b| a.name.cmp(&b.name));
        println!("Found {} Wine prefixes: {:?}", prefixes.len(), prefixes.iter().map(|p| &p.name).collect::<Vec<_>>());
        Ok(prefixes)
    }

    fn is_valid_wine_prefix(&self, path: &PathBuf) -> bool {
        let drive_c = path.join("drive_c");
        let system_reg = path.join("system.reg");
        let user_reg = path.join("user.reg");
        
        // Check for basic Wine prefix structure
        drive_c.exists() && system_reg.exists() && user_reg.exists()
    }

    fn load_or_create_config(&self, prefix_path: &PathBuf, name: &str,
                             system_wine_version: &Option<String>) -> Result<PrefixConfig> {
        let mut config = if let Some(config) = PrefixConfig::load_from_file(prefix_path)? {
            config
        } else {
            let mut config = PrefixConfig::new(name.to_string(), "win64".to_string());
            if let Ok(architecture) = self.detect_architecture(prefix_path) {
                config.architecture = architecture;
            }
            config
        };

        // Fill in wine version from cached system result if the prefix lacks one
        if config.wine_version.is_none() {
            if let Some(ver) = system_wine_version {
                config.wine_version = Some(ver.clone());
                config.save_to_file(prefix_path)?;
            }
        }

        Ok(config)
    }

    fn detect_architecture(&self, prefix_path: &PathBuf) -> Result<String> {
        // Check for 64-bit indicators
        let program_files_x64 = prefix_path.join("drive_c/Program Files");
        let program_files_x86 = prefix_path.join("drive_c/Program Files (x86)");
        
        if program_files_x86.exists() {
            Ok("win64".to_string())
        } else if program_files_x64.exists() {
            Ok("win32".to_string())
        } else {
            // Default to win64 for modern systems
            Ok("win64".to_string())
        }
    }

    pub fn create_prefix(&self, name: &str, architecture: &str) -> Result<PathBuf> {
        let prefix_path = self.wine_dir.join(name);

        // Check if prefix already exists
        if prefix_path.exists() {
            return Err(PrefixError::AlreadyExists(format!("Prefix '{}' already exists", name)));
        }

        // Create prefix directory
        fs::create_dir_all(&prefix_path)?;

        // Create initial config
        let config = PrefixConfig::new(name.to_string(), architecture.to_string());
        config.save_to_file(&prefix_path)?;

        // Initialize Wine prefix using winecfg (non-blocking)
        let wine_arch = if architecture == "win32" { "win32" } else { "win64" };
        Command::new("winecfg")
            .env("WINEPREFIX", prefix_path.to_string_lossy().as_ref())
            .env("WINEARCH", wine_arch)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run winecfg: {}", e)))?;

        Ok(prefix_path)
    }

    pub fn delete_prefix(&self, prefix_path: &PathBuf) -> Result<()> {
        if !prefix_path.exists() {
            return Err(PrefixError::NotFound("Prefix does not exist".to_string()));
        }

        // Additional safety check
        if !self.is_valid_wine_prefix(prefix_path) {
            return Err(PrefixError::Validation("Not a valid Wine prefix".to_string()));
        }

        fs::remove_dir_all(prefix_path)?;
        Ok(())
    }

    pub fn scan_for_applications(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        
        // Scan regular directories
        executables.extend(self.scanner.scan_prefix(prefix_path)?);
        
        // Scan desktop files and shortcuts
        executables.extend(self.scanner.scan_for_desktop_files(prefix_path)?);
        
        // Remove duplicates and sort
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        
        Ok(executables)
    }

    /// Async version of scan_for_applications
    pub async fn scan_for_applications_async(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        
        // Scan regular directories asynchronously
        executables.extend(self.scanner.scan_prefix_async(prefix_path).await?);
        
        // Scan desktop files and shortcuts asynchronously
        executables.extend(self.scanner.scan_for_desktop_files_async(prefix_path).await?);
        
        // Remove duplicates and sort
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        
        Ok(executables)
    }

    pub fn update_config(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<()> {
        // Validate config before saving
        config.validate()?;

        let mut updated_config = config.clone();
        updated_config.update_last_modified();
        updated_config.save_to_file(prefix_path)?;
        Ok(())
    }

    pub fn add_executable_to_prefix(&self, prefix_path: &PathBuf, executable: RegisteredExecutable) -> Result<()> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, name, &None)?;
        
        config.add_executable(executable);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn remove_executable_from_prefix(&self, prefix_path: &PathBuf, index: usize) -> Result<()> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, name, &None)?;
        
        config.remove_executable(index);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn launch_executable(&self, prefix_path: &PathBuf, executable: &RegisteredExecutable) -> Result<()> {
        if !executable.executable_path.exists() {
            return Err(PrefixError::NotFound("Executable file does not exist".to_string()));
        }

        Command::new("wine")
            .current_dir(&prefix_path)
            .env("WINEPREFIX", prefix_path.to_string_lossy().as_ref())
            .arg(&executable.executable_path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to launch executable: {}", e)))?;

        Ok(())
    }

    pub fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<()> {
        Command::new("winecfg")
            .current_dir(&prefix_path)
            .env("WINEPREFIX", prefix_path.to_string_lossy().as_ref())
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run winecfg: {}", e)))?;

        Ok(())
    }

    pub fn get_prefix_info(&self, prefix_path: &PathBuf) -> Result<PrefixInfo> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        
        let size = self.calculate_prefix_size(prefix_path)?;
        let executable_count = config.get_executable_count();
        
        Ok(PrefixInfo {
            name: config.name.clone(),
            path: prefix_path.clone(),
            size,
            executable_count,
            wine_version: config.wine_version.clone(),
            architecture: config.architecture.clone(),
            creation_date: config.creation_date,
            last_modified: config.last_modified,
        })
    }

    /// Enrich registered executables with PE icons and metadata extracted from the filesystem.
    /// Returns true if any executable was modified.
    pub fn enrich_executables(&self, config: &mut PrefixConfig) -> bool {
        let ic = self.scanner.icon_cache();
        let mut changed = false;
        for exe in &mut config.registered_executables {
            if let Some(icon_path) = crate::prefix::scanner::extract_icon_for_exe(&exe.executable_path, ic) {
                if exe.icon_path.as_ref() != Some(&icon_path) {
                    exe.icon_path = Some(icon_path);
                    changed = true;
                }
            }
            if exe.file_description.is_none() {
                let meta = crate::prefix::scanner::extract_metadata_for_exe(&exe.executable_path);
                if meta.file_version.is_some() || meta.file_description.is_some() {
                    exe.file_version = meta.file_version;
                    exe.product_version = meta.product_version;
                    exe.company_name = meta.company_name;
                    exe.file_description = meta.file_description;
                    exe.product_name = meta.product_name;
                    exe.imported_modules = meta.imported_modules;
                    changed = true;
                }
            }
        }
        changed
    }

    fn calculate_prefix_size(&self, prefix_path: &PathBuf) -> Result<u64> {
        let total_size = walkdir::WalkDir::new(prefix_path)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| entry.metadata().ok())
            .map(|metadata| metadata.len())
            .sum();

        Ok(total_size)
    }
}

impl PrefixManagerTrait for Manager {
    fn scan_prefixes(&self) -> Result<Vec<WinePrefix>> {
        self.scan_prefixes()
    }

    fn create_prefix(&self, name: &str, architecture: &str) -> Result<PathBuf> {
        self.create_prefix(name, architecture)
    }

    fn delete_prefix(&self, prefix_path: &PathBuf) -> Result<()> {
        self.delete_prefix(prefix_path)
    }

    fn scan_for_applications(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        self.scan_for_applications(prefix_path)
    }

    fn update_config(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<()> {
        self.update_config(prefix_path, config)
    }

    fn add_executable_to_prefix(&self, prefix_path: &PathBuf, executable: RegisteredExecutable) -> Result<()> {
        self.add_executable_to_prefix(prefix_path, executable)
    }

    fn remove_executable_from_prefix(&self, prefix_path: &PathBuf, index: usize) -> Result<()> {
        self.remove_executable_from_prefix(prefix_path, index)
    }

    fn launch_executable(&self, prefix_path: &PathBuf, executable: &RegisteredExecutable) -> Result<()> {
        self.launch_executable(prefix_path, executable)
    }

    fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<()> {
        self.run_winecfg(prefix_path)
    }

    fn get_prefix_info(&self, prefix_path: &PathBuf) -> Result<PrefixInfo> {
        self.get_prefix_info(prefix_path)
    }
}

impl std::fmt::Display for Manager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrefixManager(wine_dir: {})", self.wine_dir.display())
    }
}

/// Run `wine --version` once and return the formatted result (e.g. `"WINE 11.0"`).
/// Cache this value and pass it to all prefixes to avoid redundant subprocess calls.
fn detect_system_wine_version() -> Result<String> {
    let output = Command::new("wine").arg("--version").output()
        .map_err(|e| PrefixError::Process(format!("Failed to run wine --version: {}", e)))?;
    if output.status.success() {
        let raw = String::from_utf8(output.stdout)
            .map_err(|e| PrefixError::Wine(format!("Failed to parse wine version: {}", e)))?;
        // "wine-11.0" -> "WINE 11.0"
        let formatted = raw.trim().split('-').collect::<Vec<_>>().join(" ").to_uppercase();
        Ok(formatted)
    } else {
        Err(PrefixError::Wine("wine --version exited with non-zero status".to_string()))
    }
}