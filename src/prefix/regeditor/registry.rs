//! Wine registry wrapper using Regashii
//! 
//! This module provides a wrapper around Regashii's Registry functionality
//! to handle Wine registry files with async support.

use crate::prefix::error::{Result, PrefixError};
use regashii::{Format, Key, Registry, Value, ValueName};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Wine registry wrapper
#[derive(Debug, Clone)]
pub struct WineRegistry {
    /// The underlying Regashii registry
    registry: Arc<RwLock<Registry>>,
    /// Path to the registry file
    path: Option<PathBuf>,
}

impl WineRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        let registry = Registry::new(Format::Regedit5);
        Self {
            registry: Arc::new(RwLock::new(registry)),
            path: None,
        }
    }

    /// Load registry from a file
    /// 
    /// # Arguments
    /// * `path` - Path to the .reg file
    /// 
    /// # Returns
    /// `Result<Self>` - The loaded registry or error
    pub async fn load_from_file(path: &PathBuf) -> Result<Self> {
        let path_clone = path.clone();
        
        tokio::task::spawn_blocking(move || {
            let registry = Registry::deserialize_file(&path_clone)
                .map_err(|e| PrefixError::RegistryError(format!("Failed to load registry: {}", e)))?;
            
            Ok::<Self, PrefixError>(WineRegistry {
                registry: Arc::new(RwLock::new(registry)),
                path: Some(path_clone),
            })
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Save registry to a file
    /// 
    /// # Arguments
    /// * `path` - Path to save the .reg file
    /// 
    /// # Returns
    /// `Result<()>` - Success or error
    pub async fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let registry = self.registry.clone();
        let path_clone = path.clone();
        
        tokio::task::spawn_blocking(move || {
            let reg = registry.blocking_read();
            reg.serialize_file(&path_clone)
                .map_err(|e| PrefixError::RegistryError(format!("Failed to save registry: {}", e)))?;
            
            Ok::<(), PrefixError>(())
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Get a value from the registry
    /// 
    /// # Arguments
    /// * `key_path` - Full path to the key (e.g., "HKEY_CURRENT_USER\\Software\\Wine")
    /// * `value_name` - Name of the value (use ValueName::Default for the default value)
    /// 
    /// # Returns
    /// `Result<Option<Value>>` - The value if it exists
    pub async fn get_value(&self, key_path: &str, value_name: &str) -> Result<Option<Value>> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        let value_name = value_name.to_string();
        
        tokio::task::spawn_blocking(move || {
            let reg = registry.blocking_read();
            
            // Find the key by path
            for (name, key) in reg.keys() {
                if name.raw() == key_path {
                    // Find the value by name
                    for (val_name, value) in key.values() {
                        let val_name_str = match val_name {
                            ValueName::Default => "(default)".to_string(),
                            ValueName::Named(name) => name.clone(),
                        };
                        
                        if val_name_str == value_name {
                            return Ok(Some(value.clone()));
                        }
                    }
                    return Ok(None);
                }
            }
            
            Ok(None)
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Set a value in the registry
    /// 
    /// # Arguments
    /// * `key_path` - Full path to the key
    /// * `value_name` - Name of the value
    /// * `value` - The value to set
    /// 
    /// # Returns
    /// `Result<()>` - Success or error
    pub async fn set_value(&self, key_path: &str, value_name: &str, value: Value) -> Result<()> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        let value_name = value_name.to_string();
        
        tokio::task::spawn_blocking(move || {
            let mut reg = registry.blocking_write();
            
            // Convert value name string to ValueName
            let val_name = if value_name == "(default)" {
                ValueName::Default
            } else {
                ValueName::Named(value_name)
            };
            
            // Create or update the key
            let key = Key::new().with(val_name, value);
            
            // Add or replace the key
            let mut registry = (*reg).clone();
            registry = registry.with(key_path.clone(), key);
            *reg = registry;
            
            Ok::<(), PrefixError>(())
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Delete a value from the registry
    /// 
    /// # Arguments
    /// * `key_path` - Full path to the key
    /// * `value_name` - Name of the value to delete
    /// 
    /// # Returns
    /// `Result<()>` - Success or error
    pub async fn delete_value(&self, key_path: &str, value_name: &str) -> Result<()> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        let value_name = value_name.to_string();
        
        tokio::task::spawn_blocking(move || {
            let mut reg = registry.blocking_write();
            
            // Convert value name string to ValueName
            let val_name = if value_name == "(default)" {
                ValueName::Default
            } else {
                ValueName::Named(value_name)
            };
            
            // Create a delete value
            let key = Key::new().with(val_name, Value::Delete);
            
            // Add the delete key
            let mut registry = (*reg).clone();
            registry = registry.with(key_path.clone(), key);
            *reg = registry;
            
            Ok::<(), PrefixError>(())
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Delete an entire key from the registry
    /// 
    /// # Arguments
    /// * `key_path` - Full path to the key to delete
    /// 
    /// # Returns
    /// `Result<()>` - Success or error
    pub async fn delete_key(&self, key_path: &str) -> Result<()> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        
        tokio::task::spawn_blocking(move || {
            let mut reg = registry.blocking_write();
            
            // Create a delete key
            let key = Key::deleted();
            
            // Add the delete key
            let mut registry = (*reg).clone();
            registry = registry.with(key_path.clone(), key);
            *reg = registry;
            
            Ok::<(), PrefixError>(())
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Get all keys matching a pattern
    /// 
    /// # Arguments
    /// * `pattern` - Pattern to match against key paths
    /// 
    /// # Returns
    /// `Result<Vec<String>>` - List of matching key paths
    pub async fn find_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let registry = self.registry.clone();
        let pattern = pattern.to_string();
        
        tokio::task::spawn_blocking(move || {
            let reg = registry.blocking_read();
            let mut matching_keys = Vec::new();
            
            for (name, _) in reg.keys() {
                let key_path_str = name.raw();
                if key_path_str.contains(&pattern) {
                    matching_keys.push(key_path_str.to_string());
                }
            }
            
            Ok::<Vec<String>, PrefixError>(matching_keys)
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Get all values in a key
    /// 
    /// # Arguments
    /// * `key_path` - Full path to the key
    /// 
    /// # Returns
    /// `Result<HashMap<String, Value>>` - Map of value names to values
    pub async fn get_key_values(&self, key_path: &str) -> Result<HashMap<String, Value>> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        
        tokio::task::spawn_blocking(move || {
            let reg = registry.blocking_read();
            let mut values = HashMap::new();
            
            for (name, key) in reg.keys() {
                if name.raw() == key_path {
                    for (val_name, value) in key.values() {
                        let val_name_str = match val_name {
                            ValueName::Default => "(default)".to_string(),
                            ValueName::Named(name) => name.clone(),
                        };
                        values.insert(val_name_str, value.clone());
                    }
                    break;
                }
            }
            
            Ok::<HashMap<String, Value>, PrefixError>(values)
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Check if a key exists
    /// 
    /// # Arguments
    /// * `key_path` - Full path to the key
    /// 
    /// # Returns
    /// `Result<bool>` - True if the key exists
    pub async fn key_exists(&self, key_path: &str) -> Result<bool> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        
        tokio::task::spawn_blocking(move || {
            let reg = registry.blocking_read();
            
            for (name, _) in reg.keys() {
                if name.raw() == key_path {
                    return Ok(true);
                }
            }
            
            Ok(false)
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    /// Get the path of the loaded registry file
    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }
}

impl Default for WineRegistry {
    fn default() -> Self {
        Self::new()
    }
}