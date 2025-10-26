use crate::prefix::config::{PrefixConfig, RegisteredExecutable};
use crate::prefix::scanner::ApplicationScanner;
use std::path::PathBuf;
use std::fs;
use std::process::Command;

pub struct PrefixManager {
    pub wine_dir: PathBuf,
    scanner: ApplicationScanner,
}

impl PrefixManager {
    pub fn new(wine_dir: PathBuf) -> Self {
        Self {
            wine_dir,
            scanner: ApplicationScanner::new(),
        }
    }

    pub fn scan_prefixes(&self) -> Result<Vec<WinePrefix>, Box<dyn std::error::Error>> {
        let mut prefixes = Vec::new();
        
        if let Ok(entries) = fs::read_dir(&self.wine_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // Check if this directory looks like a Wine prefix
                    if self.is_valid_wine_prefix(&path) {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let config = self.load_or_create_config(&path, name)?;
                            prefixes.push(WinePrefix {
                                name: name.to_string(),
                                path: path.clone(),
                                config,
                            });
                        }
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

    fn load_or_create_config(&self, prefix_path: &PathBuf, name: &str) -> Result<PrefixConfig, Box<dyn std::error::Error>> {
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

    fn detect_wine_version(&self, prefix_path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
        // Try to detect wine version from prefix
        let version_file = prefix_path.join(".update-timestamp");
        if version_file.exists() {
            if let Ok(content) = fs::read_to_string(version_file) {
                if let Ok(timestamp) = content.trim().parse::<u64>() {
                    // Convert timestamp to readable format (this is a simplified approach)
                    return Ok(format!("Wine (timestamp: {})", timestamp));
                }
            }
        }

        // Try to get wine version from system
        if let Ok(output) = Command::new("wine").arg("--version").output() {
            if let Ok(version_str) = String::from_utf8(output.stdout) {
                return Ok(version_str.trim().to_string());
            }
        }

        Ok("unknown".to_string())
    }

    fn detect_architecture(&self, prefix_path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
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

    pub fn create_prefix(&self, name: &str, architecture: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let prefix_path = self.wine_dir.join(name);
        
        // Check if prefix already exists
        if prefix_path.exists() {
            return Err(format!("Prefix '{}' already exists", name).into());
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
                    return Err(format!("Failed to create Wine prefix: {}", error).into());
                }
            }
            Err(e) => {
                return Err(format!("Failed to run winecfg: {}", e).into());
            }
        }
        
        Ok(prefix_path)
    }

    pub fn delete_prefix(&self, prefix_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        if !prefix_path.exists() {
            return Err("Prefix does not exist".into());
        }

        // Additional safety check
        if !self.is_valid_wine_prefix(prefix_path) {
            return Err("Not a valid Wine prefix".into());
        }

        fs::remove_dir_all(prefix_path)?;
        Ok(())
    }

    pub fn scan_for_applications(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>, Box<dyn std::error::Error>> {
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

    pub fn update_config(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<(), Box<dyn std::error::Error>> {
        // Validate config before saving
        if let Err(e) = config.validate() {
            return Err(format!("Invalid config: {}", e).into());
        }

        let mut updated_config = config.clone();
        updated_config.update_last_modified();
        updated_config.save_to_file(prefix_path)?;
        Ok(())
    }

    pub fn add_executable_to_prefix(&self, prefix_path: &PathBuf, executable: RegisteredExecutable) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.load_or_create_config(prefix_path, &prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"))?;
        
        config.add_executable(executable);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn remove_executable_from_prefix(&self, prefix_path: &PathBuf, index: usize) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = self.load_or_create_config(prefix_path, &prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"))?;
        
        config.remove_executable(index);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn launch_executable(&self, prefix_path: &PathBuf, executable: &RegisteredExecutable) -> Result<(), Box<dyn std::error::Error>> {
        if !executable.executable_path.exists() {
            return Err("Executable file does not exist".into());
        }

        // Set WINEPREFIX environment variable
        let mut command = Command::new("wine");
        command.current_dir(&prefix_path);
        command.env("WINEPREFIX", prefix_path.to_string_lossy().as_ref());
        command.arg(&executable.executable_path);

        let output = command.output();
        
        match output {
            Ok(result) => {
                if !result.status.success() {
                    let error = String::from_utf8_lossy(&result.stderr);
                    return Err(format!("Failed to launch executable: {}", error).into());
                }
            }
            Err(e) => {
                return Err(format!("Failed to run wine: {}", e).into());
            }
        }
        
        Ok(())
    }

    pub fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let output = Command::new("winecfg")
            .current_dir(&prefix_path)
            .env("WINEPREFIX", prefix_path.to_string_lossy().as_ref())
            .output();
            
        match output {
            Ok(result) => {
                if !result.status.success() {
                    let error = String::from_utf8_lossy(&result.stderr);
                    return Err(format!("Failed to run winecfg: {}", error).into());
                }
            }
            Err(e) => {
                return Err(format!("Failed to run winecfg: {}", e).into());
            }
        }
        
        Ok(())
    }

    pub fn get_prefix_info(&self, prefix_path: &PathBuf) -> Result<PrefixInfo, Box<dyn std::error::Error>> {
        let config = self.load_or_create_config(prefix_path, &prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"))?;
        
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

    fn calculate_prefix_size(&self, prefix_path: &PathBuf) -> Result<u64, Box<dyn std::error::Error>> {
        let mut total_size = 0u64;
        
        for entry in walkdir::WalkDir::new(prefix_path).into_iter().flatten() {
            if entry.file_type().is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                }
            }
        }
        
        Ok(total_size)
    }
}

#[derive(Debug, Clone)]
pub struct WinePrefix {
    pub name: String,
    pub path: PathBuf,
    pub config: PrefixConfig,
}

#[derive(Debug)]
pub struct PrefixInfo {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub executable_count: usize,
    pub wine_version: Option<String>,
    pub architecture: String,
    pub creation_date: chrono::DateTime<chrono::Utc>,
    pub last_modified: chrono::DateTime<chrono::Utc>,
}