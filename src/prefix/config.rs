use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixConfig {
    pub version: String,
    pub name: String,
    pub creation_date: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub wine_version: Option<String>,
    pub architecture: String,
    pub description: Option<String>,
    pub registered_executables: Vec<RegisteredExecutable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredExecutable {
    pub name: String,
    pub description: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub executable_path: PathBuf,
}

impl PrefixConfig {
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

    pub fn save_to_file(&self, prefix_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = prefix_path.join("tequila-config.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, json)?;
        Ok(())
    }

    pub fn load_from_file(prefix_path: &PathBuf) -> Result<Option<Self>, Box<dyn std::error::Error>> {
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

    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Prefix name cannot be empty".to_string());
        }

        if self.architecture.is_empty() {
            return Err("Architecture cannot be empty".to_string());
        }

        if !["win32", "win64"].contains(&self.architecture.as_str()) {
            return Err("Architecture must be 'win32' or 'win64'".to_string());
        }

        for (i, exe) in self.registered_executables.iter().enumerate() {
            if exe.name.is_empty() {
                return Err(format!("Executable {} has empty name", i));
            }
            
            if !exe.executable_path.exists() {
                return Err(format!("Executable {} has non-existent path: {}", 
                                 i, exe.executable_path.display()));
            }
        }

        Ok(())
    }
}

impl RegisteredExecutable {
    pub fn new(name: String, executable_path: PathBuf) -> Self {
        Self {
            name,
            description: None,
            icon_path: None,
            executable_path,
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
}