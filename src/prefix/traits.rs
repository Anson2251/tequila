use crate::prefix::config::{PrefixConfig, RegisteredExecutable};
use crate::prefix::error::{Result, PrefixError};
use std::path::PathBuf;

/// Trait for configuration operations
pub trait ConfigOperations {
    /// Save configuration to file
    fn save_to_file(&self, prefix_path: &PathBuf) -> Result<()>;
    
    /// Load configuration from file
    fn load_from_file(prefix_path: &PathBuf) -> Result<Option<Self>>
    where
        Self: Sized;
    
    /// Validate the configuration
    fn validate(&self) -> Result<()>;
    
    /// Update the last modified timestamp
    fn update_last_modified(&mut self);
}

/// Trait for scanning operations
pub trait Scanner {
    /// Scan a prefix for applications
    fn scan_prefix(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>>;
    
    /// Scan for desktop files and shortcuts
    fn scan_for_desktop_files(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>>;
}

/// Trait for prefix management operations
pub trait PrefixManager {
    /// Scan for all valid prefixes
    fn scan_prefixes(&self) -> Result<Vec<WinePrefix>>;
    
    /// Create a new prefix
    fn create_prefix(&self, name: &str, architecture: &str) -> Result<PathBuf>;
    
    /// Delete an existing prefix
    fn delete_prefix(&self, prefix_path: &PathBuf) -> Result<()>;
    
    /// Scan a prefix for applications
    fn scan_for_applications(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>>;
    
    /// Update prefix configuration
    fn update_config(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<()>;
    
    /// Add an executable to the prefix configuration
    fn add_executable_to_prefix(&self, prefix_path: &PathBuf, executable: RegisteredExecutable) -> Result<()>;
    
    /// Remove an executable from the prefix configuration
    fn remove_executable_from_prefix(&self, prefix_path: &PathBuf, index: usize) -> Result<()>;
    
    /// Launch an executable within the prefix
    fn launch_executable(&self, prefix_path: &PathBuf, executable: &RegisteredExecutable) -> Result<()>;
    
    /// Run winecfg for the prefix
    fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<()>;
    
    /// Get detailed information about a prefix
    fn get_prefix_info(&self, prefix_path: &PathBuf) -> Result<PrefixInfo>;
}

/// Trait for executable management
pub trait ExecutableManager {
    /// Add an executable to the configuration
    fn add_executable(&mut self, executable: RegisteredExecutable);
    
    /// Remove an executable by index
    fn remove_executable(&mut self, index: usize);
    
    /// Get the number of registered executables
    fn executable_count(&self) -> usize;
    
    /// Find an executable by name
    fn find_executable_by_name(&self, name: &str) -> Option<&RegisteredExecutable>;
    
    /// Get all executables as an iterator
    fn executables(&self) -> std::slice::Iter<RegisteredExecutable>;
}

// Forward declaration of types used in traits
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq)]
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
    pub creation_date: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
}