use crate::config::{PrefixConfig, RegisteredExecutable};
use crate::error::Result;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

pub trait ConfigOperations {
    fn save_to_file(&self, prefix_path: &PathBuf) -> Result<()>;
    fn load_from_file(prefix_path: &PathBuf) -> Result<Option<Self>>
    where
        Self: Sized;
    fn validate(&self) -> Result<()>;
    fn update_last_modified(&mut self);
}

pub trait Scanner {
    fn scan_prefix(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>>;
    fn scan_for_desktop_files(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>>;
}

pub trait PrefixManager {
    fn scan_prefixes(&self) -> Result<Vec<WinePrefix>>;
    fn create_prefix(&self, name: &str, architecture: &str) -> Result<PathBuf>;
    fn delete_prefix(&self, prefix_path: &PathBuf) -> Result<()>;
    fn scan_for_applications(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>>;
    fn update_config(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<()>;
    fn add_executable_to_prefix(
        &self,
        prefix_path: &PathBuf,
        executable: RegisteredExecutable,
    ) -> Result<()>;
    fn remove_executable_from_prefix(&self, prefix_path: &PathBuf, index: usize) -> Result<()>;
    fn launch_executable(
        &self,
        prefix_path: &PathBuf,
        executable: &RegisteredExecutable,
    ) -> Result<()>;
    fn run_winecfg(&self, prefix_path: &PathBuf) -> Result<()>;
    fn get_prefix_info(&self, prefix_path: &PathBuf) -> Result<PrefixInfo>;
}

pub trait ExecutableManager {
    fn add_executable(&mut self, executable: RegisteredExecutable);
    fn remove_executable(&mut self, index: usize);
    fn executable_count(&self) -> usize;
    fn find_executable_by_name(&self, name: &str) -> Option<&RegisteredExecutable>;
    fn executables(&self) -> std::slice::Iter<'_, RegisteredExecutable>;
}

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
