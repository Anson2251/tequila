use crate::prefix::config::{PrefixConfig, RegisteredExecutable};
use crate::prefix::scanner::ApplicationScanner;
use crate::prefix::error::{Result, PrefixError};
use crate::prefix::traits::{PrefixManager as PrefixManagerTrait, WinePrefix, PrefixInfo};
use std::path::PathBuf;
use std::fs;
use std::process::Command;

#[derive(PartialEq)]
pub struct Manager {
    wine_dir: PathBuf,
    scanner: ApplicationScanner,
}

impl Manager {
    /// Create a new PrefixManager with the specified wine directory
    ///
    /// # Arguments
    ///
    /// * `wine_dir` - Directory containing Wine prefixes
    ///
    /// # Returns
    ///
    /// Returns a new PrefixManager instance
    pub fn new(wine_dir: PathBuf) -> Self {
        Self {
            wine_dir,
            scanner: ApplicationScanner::new(),
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
        
        for entry in fs::read_dir(&self.wine_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() && self.is_valid_wine_prefix(&path) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(config) = self.load_or_create_config(&path, name) {
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
        let drive_c = path.join("drive_c");
        let system_reg = path.join("system.reg");
        let user_reg = path.join("user.reg");
        
        // Check for basic Wine prefix structure
        drive_c.exists() && system_reg.exists() && user_reg.exists()
    }

    fn load_or_create_config(&self, prefix_path: &PathBuf, name: &str) -> Result<PrefixConfig> {
        // Try to load existing config
        if let Some(config) = PrefixConfig::load_from_file(prefix_path)? {
            return Ok(config);
        }

        // Create a new config for existing prefix
        let mut config = PrefixConfig::new(name.to_string(), "win64".to_string());
        
        // Try to detect wine version and architecture
        if let Ok(wine_version) = self.detect_wine_version(prefix_path) {
            config.wine_version = Some(wine_version);
        }

        if let Ok(architecture) = self.detect_architecture(prefix_path) {
            config.architecture = architecture;
        }

        // Save new config
        config.save_to_file(prefix_path)?;
        Ok(config)
    }

    fn detect_wine_version(&self, prefix_path: &PathBuf) -> Result<String> {
        // Try to detect wine version from prefix
        let version_file = prefix_path.join(".update-timestamp");
        if version_file.exists() {
            let content = fs::read_to_string(version_file)?;
            if let Ok(timestamp) = content.trim().parse::<u64>() {
                // Convert timestamp to readable format (this is a simplified approach)
                return Ok(format!("Wine (timestamp: {})", timestamp));
            }
        }

        // Try to get wine version from system
        let output = Command::new("wine").arg("--version").output()?;
        if output.status.success() {
            let version_str = String::from_utf8(output.stdout)
                .map_err(|e| PrefixError::Wine(format!("Failed to parse wine version: {}", e)))?;
            return Ok(version_str.trim().to_string());
        }

        Err(PrefixError::Wine("Failed to get wine version".to_string()))
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
        
        // Initialize Wine prefix using winecfg
        let wine_arch = if architecture == "win32" { "win32" } else { "win64" };
        let output = Command::new("winecfg")
            .args(&["--arch", wine_arch, "--prefix", &prefix_path.to_string_lossy()])
            .output();
            
        match output {
            Ok(result) => {
                if !result.status.success() {
                    let error = String::from_utf8_lossy(&result.stderr);
                    return Err(PrefixError::Process(format!("Failed to create Wine prefix: {}", error)));
                }
            }
            Err(e) => {
                return Err(PrefixError::Process(format!("Failed to run winecfg: {}", e)));
            }
        }
        
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
        let mut config = self.load_or_create_config(prefix_path, name)?;
        
        config.add_executable(executable);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn remove_executable_from_prefix(&self, prefix_path: &PathBuf, index: usize) -> Result<()> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, name)?;
        
        config.remove_executable(index);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn launch_executable(&self, prefix_path: &PathBuf, executable: &RegisteredExecutable) -> Result<()> {
        if !executable.executable_path.exists() {
            return Err(PrefixError::NotFound("Executable file does not exist".to_string()));
        }

        // Set WINEPREFIX environment variable
        let output = Command::new("wine")
            .current_dir(&prefix_path)
            .env("WINEPREFIX", prefix_path.to_string_lossy().as_ref())
            .arg(&executable.executable_path)
            .output();
        
        match output {
            Ok(result) => {
                if !result.status.success() {
                    let error = String::from_utf8_lossy(&result.stderr);
                    return Err(PrefixError::Process(format!("Failed to launch executable: {}", error)));
                }
            }
            Err(e) => {
                return Err(PrefixError::Process(format!("Failed to run wine: {}", e)));
            }
        }
        
        Ok(())
    }

    pub fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<()> {
        let output = Command::new("winecfg")
            .current_dir(&prefix_path)
            .env("WINEPREFIX", prefix_path.to_string_lossy().as_ref())
            .output();
            
        match output {
            Ok(result) => {
                if !result.status.success() {
                    let error = String::from_utf8_lossy(&result.stderr);
                    return Err(PrefixError::Process(format!("Failed to run winecfg: {}", error)));
                }
            }
            Err(e) => {
                return Err(PrefixError::Process(format!("Failed to run winecfg: {}", e)));
            }
        }
        
        Ok(())
    }

    pub fn get_prefix_info(&self, prefix_path: &PathBuf) -> Result<PrefixInfo> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name)?;
        
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