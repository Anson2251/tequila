use base::error::{Result, PrefixError};
use regashii::{Format, Key, Registry, Value, ValueName};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct WineRegistry {
    registry: Arc<RwLock<Registry>>,
    path: Option<PathBuf>,
}

impl WineRegistry {
    pub fn new() -> Self {
        let registry = Registry::new(Format::Regedit5);
        Self {
            registry: Arc::new(RwLock::new(registry)),
            path: None,
        }
    }

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

    pub async fn load_from_prefix(prefix_path: &PathBuf) -> Result<Self> {
        let system_reg_path = prefix_path.join("system.reg");
        let user_reg_path = prefix_path.join("user.reg");
        let userdef_reg_path = prefix_path.join("userdef.reg");

        let (system_result, user_result, userdef_result) = tokio::join!(
            tokio::task::spawn_blocking({
                let path = system_reg_path.clone();
                move || {
                    if path.exists() {
                        Registry::deserialize_file(&path).map(Some)
                    } else {
                        Ok(None)
                    }
                }
            }),
            tokio::task::spawn_blocking({
                let path = user_reg_path.clone();
                move || {
                    if path.exists() {
                        Registry::deserialize_file(&path).map(Some)
                    } else {
                        Ok(None)
                    }
                }
            }),
            tokio::task::spawn_blocking({
                let path = userdef_reg_path.clone();
                move || {
                    if path.exists() {
                        Registry::deserialize_file(&path).map(Some)
                    } else {
                        Ok(None)
                    }
                }
            })
        );

        let mut merged_registry = Registry::new(Format::Regedit5);

        match system_result.map_err(|e| PrefixError::RegistryError(format!("System registry task error: {}", e)))? {
            Ok(Some(system_registry)) => {
                merged_registry = system_registry;
            }
            Ok(None) => {}
            Err(e) => eprintln!("Warning: Failed to load system.reg: {}", e),
        }

        match userdef_result.map_err(|e| PrefixError::RegistryError(format!("Userdef registry task error: {}", e)))? {
            Ok(Some(userdef_registry)) => {
                if merged_registry.keys().is_empty() {
                    merged_registry = userdef_registry;
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("Warning: Failed to load userdef.reg: {}", e),
        }

        match user_result.map_err(|e| PrefixError::RegistryError(format!("User registry task error: {}", e)))? {
            Ok(Some(user_registry)) => {
                merged_registry = user_registry;
            }
            Ok(None) => {}
            Err(e) => eprintln!("Warning: Failed to load user.reg: {}", e),
        }

        Ok(WineRegistry {
            registry: Arc::new(RwLock::new(merged_registry)),
            path: Some(prefix_path.clone()),
        })
    }

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

    pub async fn get_value(&self, key_path: &str, value_name: &str) -> Result<Option<Value>> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        let value_name = value_name.to_string();
        tokio::task::spawn_blocking(move || {
            let reg = registry.blocking_read();
            for (name, key) in reg.keys() {
                if name.raw() == key_path {
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

    pub async fn set_value(&self, key_path: &str, value_name: &str, value: Value) -> Result<()> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        let value_name = value_name.to_string();
        tokio::task::spawn_blocking(move || {
            let mut reg = registry.blocking_write();
            let val_name = if value_name == "(default)" {
                ValueName::Default
            } else {
                ValueName::Named(value_name)
            };
            let key = Key::new().with(val_name, value);
            let mut registry = (*reg).clone();
            registry = registry.with(key_path.clone(), key);
            *reg = registry;
            Ok::<(), PrefixError>(())
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    pub async fn delete_value(&self, key_path: &str, value_name: &str) -> Result<()> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        let value_name = value_name.to_string();
        tokio::task::spawn_blocking(move || {
            let mut reg = registry.blocking_write();
            let val_name = if value_name == "(default)" {
                ValueName::Default
            } else {
                ValueName::Named(value_name)
            };
            let key = Key::new().with(val_name, Value::Delete);
            let mut registry = (*reg).clone();
            registry = registry.with(key_path.clone(), key);
            *reg = registry;
            Ok::<(), PrefixError>(())
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

    pub async fn delete_key(&self, key_path: &str) -> Result<()> {
        let registry = self.registry.clone();
        let key_path = key_path.to_string();
        tokio::task::spawn_blocking(move || {
            let mut reg = registry.blocking_write();
            let key = Key::deleted();
            let mut registry = (*reg).clone();
            registry = registry.with(key_path.clone(), key);
            *reg = registry;
            Ok::<(), PrefixError>(())
        })
        .await
        .map_err(|e| PrefixError::RegistryError(format!("Task join error: {}", e)))?
    }

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

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }
}

impl Default for WineRegistry {
    fn default() -> Self {
        Self::new()
    }
}
