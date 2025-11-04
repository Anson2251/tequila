use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use crate::prefix::error::{Result, PrefixError};
use crate::prefix::traits::{ConfigOperations, ExecutableManager};

/// Configuration for a Wine prefix
///
/// This struct contains all the metadata and configuration for a Wine prefix,
/// including version information, creation dates, and registered executables.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrefixConfig {
    /// Configuration version (for migration purposes)
    pub version: String,
    /// Human-readable name for the prefix
    pub name: String,
    /// When this prefix was created
    pub creation_date: DateTime<Utc>,
    /// When this prefix was last modified
    pub last_modified: DateTime<Utc>,
    /// Wine version used with this prefix
    pub wine_version: Option<String>,
    /// Architecture (win32 or win64)
    pub architecture: String,
    /// Optional description of the prefix
    pub description: Option<String>,
    /// List of registered executables for this prefix
    pub registered_executables: Vec<RegisteredExecutable>,
}

/// Represents a registered executable within a Wine prefix
///
/// This struct contains information about an executable that has been
/// registered for management within the application.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisteredExecutable {
    /// Human-readable name for the executable
    pub name: String,
    /// Optional description of what this executable does
    pub description: Option<String>,
    /// Optional path to an icon file
    pub icon_path: Option<PathBuf>,
    /// Path to the executable file
    pub executable_path: PathBuf,
    /// Optional file version from executable metadata
    pub file_version: Option<String>,
    /// Optional product version from executable metadata
    pub product_version: Option<String>,
    /// Optional company name from executable metadata
    pub company_name: Option<String>,
    /// Optional file description from executable metadata
    pub file_description: Option<String>,
    /// Optional product name from executable metadata
    pub product_name: Option<String>,
    /// Optional list of imported modules (DLLs)
    #[serde(default)]
    pub imported_modules: Vec<String>,
}

impl PrefixConfig {
    /// Create a new PrefixConfig with default values
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the prefix
    /// * `architecture` - Either "win32" or "win64"
    ///
    /// # Returns
    ///
    /// Returns a new PrefixConfig with current timestamp and empty executable list
    pub fn new(name: String, architecture: String) -> Self {
        let now = Utc::now();
        Self {
            version: "1.0.0".to_string(),
            name,
            creation_date: now,
            last_modified: now,
            wine_version: None,
            architecture,
            description: None,
            registered_executables: Vec::new(),
        }
    }

    /// Save this configuration to a JSON file in the prefix directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - File cannot be created
    /// - Configuration cannot be serialized to JSON
    /// - File cannot be written
    pub fn save_to_file(&self, prefix_path: &PathBuf) -> Result<()> {
        let config_path = prefix_path.join("tequila-config.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, json)?;
        Ok(())
    }

    /// Load configuration from a JSON file in the prefix directory
    ///
    /// # Arguments
    ///
    /// * `prefix_path` - Path to the prefix directory
    ///
    /// # Returns
    ///
    /// * `Ok(Some(config))` - If configuration file exists and is valid
    /// * `Ok(None)` - If no configuration file exists
    /// * `Err(e)` - If file cannot be read or parsed
    pub fn load_from_file(prefix_path: &PathBuf) -> Result<Option<Self>> {
        let config_path = prefix_path.join("tequila-config.json");
        
        if !config_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(config_path)?;
        let config: PrefixConfig = serde_json::from_str(&content)?;
        Ok(Some(config))
    }

    pub fn update_last_modified(&mut self) {
        self.last_modified = Utc::now();
    }

    pub fn add_executable(&mut self, executable: RegisteredExecutable) {
        self.registered_executables.push(executable);
        self.update_last_modified();
    }

    pub fn remove_executable(&mut self, index: usize) {
        if index < self.registered_executables.len() {
            self.registered_executables.remove(index);
            self.update_last_modified();
        }
    }

    pub fn get_executable_count(&self) -> usize {
        self.registered_executables.len()
    }

    pub fn get_executable_by_name(&self, name: &str) -> Option<&RegisteredExecutable> {
        self.registered_executables.iter().find(|exe| exe.name == name)
    }

    pub fn executables(&self) -> std::slice::Iter<RegisteredExecutable> {
        self.registered_executables.iter()
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(PrefixError::Validation("Prefix name cannot be empty".to_string()));
        }

        if self.architecture.is_empty() {
            return Err(PrefixError::Validation("Architecture cannot be empty".to_string()));
        }

        if !["win32", "win64"].contains(&self.architecture.as_str()) {
            return Err(PrefixError::Validation("Architecture must be 'win32' or 'win64'".to_string()));
        }

        for (i, exe) in self.registered_executables.iter().enumerate() {
            if exe.name.is_empty() {
                return Err(PrefixError::Validation(format!("Executable {} has empty name", i)));
            }
            
            if !exe.executable_path.exists() {
                return Err(PrefixError::Validation(format!("Executable {} has non-existent path: {}",
                                 i, exe.executable_path.display())));
            }
        }

        Ok(())
    }
}

impl ConfigOperations for PrefixConfig {
    fn save_to_file(&self, prefix_path: &PathBuf) -> Result<()> {
        let config_path = prefix_path.join("tequila-config.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, json)?;
        Ok(())
    }

    fn load_from_file(prefix_path: &PathBuf) -> Result<Option<Self>> {
        let config_path = prefix_path.join("tequila-config.json");
        
        if !config_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(config_path)?;
        let config: PrefixConfig = serde_json::from_str(&content)?;
        Ok(Some(config))
    }

    fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(PrefixError::Validation("Prefix name cannot be empty".to_string()));
        }

        if self.architecture.is_empty() {
            return Err(PrefixError::Validation("Architecture cannot be empty".to_string()));
        }

        if !["win32", "win64"].contains(&self.architecture.as_str()) {
            return Err(PrefixError::Validation("Architecture must be 'win32' or 'win64'".to_string()));
        }

        for (i, exe) in self.registered_executables.iter().enumerate() {
            if exe.name.is_empty() {
                return Err(PrefixError::Validation(format!("Executable {} has empty name", i)));
            }
            
            if !exe.executable_path.exists() {
                return Err(PrefixError::Validation(format!("Executable {} has non-existent path: {}",
                                 i, exe.executable_path.display())));
            }
        }

        Ok(())
    }

    fn update_last_modified(&mut self) {
        self.last_modified = Utc::now();
    }
}

impl ExecutableManager for PrefixConfig {
    fn add_executable(&mut self, executable: RegisteredExecutable) {
        self.registered_executables.push(executable);
        self.update_last_modified();
    }

    fn remove_executable(&mut self, index: usize) {
        if index < self.registered_executables.len() {
            self.registered_executables.remove(index);
            self.update_last_modified();
        }
    }

    fn executable_count(&self) -> usize {
        self.registered_executables.len()
    }

    fn find_executable_by_name(&self, name: &str) -> Option<&RegisteredExecutable> {
        self.registered_executables.iter().find(|exe| exe.name == name)
    }

    fn executables(&self) -> std::slice::Iter<RegisteredExecutable> {
        self.registered_executables.iter()
    }
}

impl std::fmt::Display for PrefixConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrefixConfig(name: {}, arch: {}, executables: {})",
               self.name, self.architecture, self.registered_executables.len())
    }
}

impl std::fmt::Display for RegisteredExecutable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RegisteredExecutable(name: {}, path: {})",
               self.name, self.executable_path.display())
    }
}

impl RegisteredExecutable {
    pub fn new(name: String, executable_path: PathBuf) -> Self {
        Self {
            name,
            description: None,
            icon_path: None,
            executable_path,
            file_version: None,
            product_version: None,
            company_name: None,
            file_description: None,
            product_name: None,
            imported_modules: Vec::new(),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_icon_path(mut self, icon_path: PathBuf) -> Self {
        self.icon_path = Some(icon_path);
        self
    }

    pub fn with_file_version<S: Into<String>>(mut self, version: S) -> Self {
        self.file_version = Some(version.into());
        self
    }

    pub fn with_product_version<S: Into<String>>(mut self, version: S) -> Self {
        self.product_version = Some(version.into());
        self
    }

    pub fn with_company_name<S: Into<String>>(mut self, company: S) -> Self {
        self.company_name = Some(company.into());
        self
    }

    pub fn with_file_description<S: Into<String>>(mut self, description: S) -> Self {
        self.file_description = Some(description.into());
        self
    }

    pub fn with_product_name<S: Into<String>>(mut self, product: S) -> Self {
        self.product_name = Some(product.into());
        self
    }

    pub fn with_imported_modules(mut self, modules: Vec<String>) -> Self {
        self.imported_modules = modules;
        self
    }
}

/// Builder for RegisteredExecutable
pub struct RegisteredExecutableBuilder {
    name: Option<String>,
    description: Option<String>,
    icon_path: Option<PathBuf>,
    executable_path: Option<PathBuf>,
    file_version: Option<String>,
    product_version: Option<String>,
    company_name: Option<String>,
    file_description: Option<String>,
    product_name: Option<String>,
    imported_modules: Vec<String>,
}

impl RegisteredExecutableBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            description: None,
            icon_path: None,
            executable_path: None,
            file_version: None,
            product_version: None,
            company_name: None,
            file_description: None,
            product_name: None,
            imported_modules: Vec::new(),
        }
    }

    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description<S: Into<String>>(mut self, description: S) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn icon_path<P: Into<PathBuf>>(mut self, icon_path: P) -> Self {
        self.icon_path = Some(icon_path.into());
        self
    }

    pub fn executable_path<P: Into<PathBuf>>(mut self, executable_path: P) -> Self {
        self.executable_path = Some(executable_path.into());
        self
    }

    pub fn file_version<S: Into<String>>(mut self, version: S) -> Self {
        self.file_version = Some(version.into());
        self
    }

    pub fn product_version<S: Into<String>>(mut self, version: S) -> Self {
        self.product_version = Some(version.into());
        self
    }

    pub fn company_name<S: Into<String>>(mut self, company: S) -> Self {
        self.company_name = Some(company.into());
        self
    }

    pub fn file_description<S: Into<String>>(mut self, description: S) -> Self {
        self.file_description = Some(description.into());
        self
    }

    pub fn product_name<S: Into<String>>(mut self, product: S) -> Self {
        self.product_name = Some(product.into());
        self
    }

    pub fn imported_modules(mut self, modules: Vec<String>) -> Self {
        self.imported_modules = modules;
        self
    }

    pub fn build(self) -> std::result::Result<RegisteredExecutable, PrefixError> {
        Ok(RegisteredExecutable {
            name: self.name.ok_or_else(|| PrefixError::Validation("Name is required".to_string()))?,
            description: self.description,
            icon_path: self.icon_path,
            executable_path: self.executable_path.ok_or_else(|| PrefixError::Validation("Executable path is required".to_string()))?,
            file_version: self.file_version,
            product_version: self.product_version,
            company_name: self.company_name,
            file_description: self.file_description,
            product_name: self.product_name,
            imported_modules: self.imported_modules,
        })
    }
}

impl Default for RegisteredExecutableBuilder {
    fn default() -> Self {
        Self::new()
    }
}