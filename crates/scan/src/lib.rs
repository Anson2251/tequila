pub mod icon_cache;
pub mod icon_extract;

pub use icon_cache::IconCache;

use base::config::RegisteredExecutable;
use base::error::{PrefixError, Result};
use base::traits::Scanner;
use exe::VecPE;
use exe::types::{CCharString, ImportDirectory, VSVersionInfo};
use sha2::{Digest, Sha256};
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use walkdir::WalkDir;

pub struct ApplicationScanner {
    app_dirs: Vec<&'static str>,
    executable_extensions: Vec<&'static str>,
    icon_extensions: Vec<&'static str>,
    icon_cache: Arc<IconCache>,
}

#[derive(Debug, Default)]
pub struct ExecutableMetadata {
    pub file_version: Option<String>,
    pub product_version: Option<String>,
    pub company_name: Option<String>,
    pub file_description: Option<String>,
    pub product_name: Option<String>,
    pub imported_modules: Vec<String>,
}

impl ApplicationScanner {
    pub fn new(icon_cache: Arc<IconCache>) -> Self {
        Self {
            app_dirs: vec![
                "drive_c/Program Files",
                "drive_c/Program Files (x86)",
                "drive_c/users",
                "drive_c/ProgramData/Microsoft/Windows/Start Menu/Programs",
                "drive_c/ProgramData/Desktop",
            ],
            executable_extensions: vec!["exe"],
            icon_extensions: vec!["ico", "icns", "png", "jpg", "jpeg"],
            icon_cache,
        }
    }

    pub fn icon_cache(&self) -> &Arc<IconCache> {
        &self.icon_cache
    }

    pub fn scan_prefix(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        for app_dir in &self.app_dirs {
            let full_path = prefix_path.join(app_dir);
            if full_path.exists() && full_path.is_dir() {
                if let Ok(mut dir_executables) = self.scan_directory(&full_path) {
                    executables.append(&mut dir_executables);
                }
            }
        }
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        Ok(executables)
    }

    fn scan_directory(&self, dir_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let executables: Vec<RegisteredExecutable> = WalkDir::new(dir_path)
            .max_depth(10)
            .into_iter()
            .flatten()
            .filter(|entry| {
                let path = entry.path();
                path.is_file()
                    && path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|ext| {
                            self.executable_extensions
                                .contains(&ext.to_lowercase().as_str())
                        })
                        .unwrap_or(false)
            })
            .filter_map(|entry| {
                let path = entry.path().to_path_buf();
                match self.create_executable_from_path(&path) {
                    Ok(Some(executable)) => Some(executable),
                    Ok(None) => None,
                    Err(_) => None,
                }
            })
            .collect();
        Ok(executables)
    }

    fn create_executable_from_path(&self, path: &PathBuf) -> Result<Option<RegisteredExecutable>> {
        let path_str = path.to_string_lossy();
        if self.should_skip_executable(&path_str) {
            return Ok(None);
        }

        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let mut icon_path = self.find_icon_for_executable(path)?;
        if icon_path.is_none() {
            icon_path = self.extract_icon_with_cache(path);
        }

        let description = self.extract_description_from_path(path);

        let exe_path_for_meta = path.clone();
        let metadata = catch_unwind(AssertUnwindSafe(|| {
            let image = VecPE::from_disk_file(&exe_path_for_meta).ok()?;
            self.extract_executable_metadata(&image)
        }))
        .unwrap_or_else(|_| {
            eprintln!("PE parsing panicked for: {}", exe_path_for_meta.display());
            None
        });

        let mut executable = RegisteredExecutable::new(name, path.to_path_buf())
            .with_description(description.unwrap_or_default());

        if let Some(icon) = icon_path {
            executable = executable.with_icon_path(icon);
        }
        if let Some(meta) = metadata {
            if let Some(file_version) = meta.file_version {
                executable = executable.with_file_version(file_version);
            }
            if let Some(product_version) = meta.product_version {
                executable = executable.with_product_version(product_version);
            }
            if let Some(company_name) = meta.company_name {
                executable = executable.with_company_name(company_name);
            }
            if let Some(file_description) = meta.file_description {
                executable = executable.with_file_description(file_description);
            }
            if let Some(product_name) = meta.product_name {
                executable = executable.with_product_name(product_name);
            }
            if !meta.imported_modules.is_empty() {
                executable = executable.with_imported_modules(meta.imported_modules);
            }
        }

        Ok(Some(executable))
    }

    fn should_skip_executable(&self, path: &str) -> bool {
        let skip_patterns = vec![
            "windows/system32",
            "windows/syswow64",
            "windows/servicing",
            "windows/inf",
            "windows/driverstore",
            "windows/winSxS",
            "windows/microsoft.net",
            "windows/assembly",
            "program files/common files",
            "program files (x86)/common files",
            "programdata/microsoft",
            "users/default",
            "users/public",
            "$recycle.bin",
            "system volume information",
        ];
        let path_lower = path.to_lowercase();
        for pattern in &skip_patterns {
            if path_lower.contains(pattern) {
                return true;
            }
        }

        let skip_executables = vec![
            "unins000",
            "unins001",
            "uninstall",
            "setup",
            "install",
            "update",
            "patch",
            "dllhost",
            "rundll32",
            "regsvr32",
            "msiexec",
            "wuauclt",
            "svchost",
        ];
        if let Some(filename) = std::path::Path::new(path).file_name() {
            if let Some(filename_str) = filename.to_str() {
                let filename_lower = filename_str.to_lowercase();
                for skip_exe in &skip_executables {
                    if filename_lower.starts_with(skip_exe) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn find_icon_for_executable(&self, exe_path: &PathBuf) -> Result<Option<PathBuf>> {
        let parent = exe_path
            .parent()
            .ok_or_else(|| PrefixError::InvalidPath("No parent directory".to_string()))?;
        let stem = exe_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PrefixError::InvalidPath("Invalid file stem".to_string()))?;

        let same_name_icon = self.icon_extensions.iter().find_map(|ext| {
            let icon_path = parent.join(format!("{}.{}", stem, ext));
            icon_path.exists().then(|| icon_path)
        });
        if same_name_icon.is_some() {
            return Ok(same_name_icon);
        }

        let common_icon_names = ["icon", "app", "main", "logo"];
        let common_icon = common_icon_names.iter().find_map(|icon_name| {
            self.icon_extensions.iter().find_map(|ext| {
                let icon_path = parent.join(format!("{}.{}", icon_name, ext));
                icon_path.exists().then(|| icon_path)
            })
        });
        Ok(common_icon)
    }

    fn extract_icon_with_cache(&self, exe_path: &Path) -> Option<PathBuf> {
        extract_icon_for_exe(exe_path, &self.icon_cache)
    }

    fn extract_description_from_path(&self, path: &PathBuf) -> Option<String> {
        let path_components: Vec<&str> = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();
        for (i, component) in path_components.iter().enumerate() {
            if component.to_lowercase() == "program files"
                || component.to_lowercase() == "program files (x86)"
            {
                if i + 1 < path_components.len() {
                    let app_name = path_components[i + 1];
                    if !app_name.to_lowercase().contains("common") {
                        return Some(format!("Application: {}", app_name));
                    }
                }
            }
        }
        if let Some(parent) = path.parent() {
            if let Some(parent_name) = parent.file_name().and_then(|n| n.to_str()) {
                if !parent_name.to_lowercase().contains("system")
                    && !parent_name.to_lowercase().contains("windows")
                {
                    return Some(format!("Located in: {}", parent_name));
                }
            }
        }
        None
    }

    fn extract_executable_metadata(&self, image: &VecPE) -> Option<ExecutableMetadata> {
        let mut metadata = ExecutableMetadata::default();
        self.extract_version_info(image, &mut metadata);
        self.extract_imported_modules(image, &mut metadata);
        Some(metadata)
    }

    fn extract_version_info(&self, image: &VecPE, metadata: &mut ExecutableMetadata) {
        match VSVersionInfo::parse(image) {
            Ok(version_info) => {
                if let Some(fixed) = version_info.value {
                    metadata.file_version = Some(format!(
                        "{}.{}.{}.{}",
                        fixed.file_version_ms >> 16,
                        fixed.file_version_ms & 0xFFFF,
                        fixed.file_version_ls >> 16,
                        fixed.file_version_ls & 0xFFFF
                    ));
                    metadata.product_version = Some(format!(
                        "{}.{}.{}.{}",
                        fixed.product_version_ms >> 16,
                        fixed.product_version_ms & 0xFFFF,
                        fixed.product_version_ls >> 16,
                        fixed.product_version_ls & 0xFFFF
                    ));
                }
                if let Some(string_info) = version_info.string_file_info {
                    for table in &string_info.children {
                        if let Ok(map) = table.string_map() {
                            for (key, value) in &map {
                                match key.as_str() {
                                    "CompanyName" => {
                                        metadata.company_name = Some(value.clone());
                                    }
                                    "FileDescription" => {
                                        metadata.file_description = Some(value.clone());
                                    }
                                    "ProductName" => {
                                        metadata.product_name = Some(value.clone());
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                metadata.file_description = Some("Windows Application".to_string());
            }
        }
    }

    fn extract_imported_modules(&self, image: &VecPE, metadata: &mut ExecutableMetadata) {
        match ImportDirectory::parse(image) {
            Ok(import_directory) => {
                for descriptor in import_directory.descriptors {
                    if let Ok(name) = descriptor.get_name(image) {
                        if let Ok(name_str) = name.as_str() {
                            metadata.imported_modules.push(name_str.to_string());
                        }
                    }
                }
            }
            Err(_) => {}
        }
    }

    pub fn scan_for_desktop_files(
        &self,
        prefix_path: &PathBuf,
    ) -> Result<Vec<RegisteredExecutable>> {
        let desktop_dirs = vec![
            "drive_c/users/Public/Desktop",
            "drive_c/ProgramData/Microsoft/Windows/Start Menu/Programs",
            "drive_c/users/default/Desktop",
        ];
        let executables: Vec<RegisteredExecutable> = desktop_dirs
            .iter()
            .filter_map(|desktop_dir| {
                let full_path = prefix_path.join(desktop_dir);
                if full_path.exists() {
                    self.scan_desktop_directory(&full_path).ok()
                } else {
                    Some(Vec::new())
                }
            })
            .flatten()
            .collect();
        Ok(executables)
    }

    fn scan_desktop_directory(&self, dir_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let executables: Vec<RegisteredExecutable> = fs::read_dir(dir_path)?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| {
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| match ext.to_lowercase().as_str() {
                        "lnk" | "desktop" => false,
                        _ => self
                            .executable_extensions
                            .contains(&ext.to_lowercase().as_str()),
                    })
                    .unwrap_or(false)
            })
            .filter_map(|path| self.create_executable_from_path(&path).ok().flatten())
            .collect();
        Ok(executables)
    }
}

impl Scanner for ApplicationScanner {
    fn scan_prefix(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        self.scan_prefix(prefix_path)
    }

    fn scan_for_desktop_files(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        self.scan_for_desktop_files(prefix_path)
    }
}

impl Clone for ApplicationScanner {
    fn clone(&self) -> Self {
        Self {
            app_dirs: self.app_dirs.clone(),
            executable_extensions: self.executable_extensions.clone(),
            icon_extensions: self.icon_extensions.clone(),
            icon_cache: Arc::clone(&self.icon_cache),
        }
    }
}

impl ApplicationScanner {
    pub async fn scan_prefix_async(
        &self,
        prefix_path: &PathBuf,
    ) -> Result<Vec<RegisteredExecutable>> {
        let prefix_path = prefix_path.clone();
        let scanner = self.clone();
        tokio::task::spawn_blocking(move || scanner.scan_prefix(&prefix_path))
            .await
            .map_err(|e| {
                PrefixError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to spawn scanning task: {}", e),
                ))
            })?
    }

    pub async fn scan_for_desktop_files_async(
        &self,
        prefix_path: &PathBuf,
    ) -> Result<Vec<RegisteredExecutable>> {
        let prefix_path = prefix_path.clone();
        let scanner = self.clone();
        tokio::task::spawn_blocking(move || scanner.scan_for_desktop_files(&prefix_path))
            .await
            .map_err(|e| {
                PrefixError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to spawn scanning task: {}", e),
                ))
            })?
    }
}

pub fn extract_icon_for_exe(exe_path: &Path, icon_cache: &IconCache) -> Option<PathBuf> {
    let file_bytes = std::fs::read(exe_path).ok()?;
    let sha256 = hex::encode(Sha256::digest(&file_bytes));
    match icon_cache.has_icon(&sha256) {
        Some(true) => return icon_cache.icon_path(&sha256),
        Some(false) => return None,
        None => {}
    }
    let image = VecPE::from_disk_file(exe_path).ok()?;
    match icon_extract::extract_icon(&image) {
        Some(icon_data) => {
            let _ = icon_cache.put(&sha256, &icon_data);
            icon_cache.icon_path(&sha256)
        }
        None => {
            let _ = icon_cache.put(&sha256, &[]);
            None
        }
    }
}

pub fn extract_metadata_for_exe(exe_path: &Path) -> ExecutableMetadata {
    catch_unwind(AssertUnwindSafe(|| {
        let image = VecPE::from_disk_file(exe_path).ok()?;
        let mut meta = ExecutableMetadata::default();
        if let Ok(version_info) = VSVersionInfo::parse(&image) {
            if let Some(fixed) = version_info.value {
                meta.file_version = Some(format!(
                    "{}.{}.{}.{}",
                    fixed.file_version_ms >> 16,
                    fixed.file_version_ms & 0xFFFF,
                    fixed.file_version_ls >> 16,
                    fixed.file_version_ls & 0xFFFF
                ));
                meta.product_version = Some(format!(
                    "{}.{}.{}.{}",
                    fixed.product_version_ms >> 16,
                    fixed.product_version_ms & 0xFFFF,
                    fixed.product_version_ls >> 16,
                    fixed.product_version_ls & 0xFFFF
                ));
            }
            if let Some(string_info) = version_info.string_file_info {
                for table in &string_info.children {
                    if let Ok(map) = table.string_map() {
                        for (key, value) in &map {
                            match key.as_str() {
                                "CompanyName" => {
                                    meta.company_name = Some(value.clone());
                                }
                                "FileDescription" => {
                                    meta.file_description = Some(value.clone());
                                }
                                "ProductName" => {
                                    meta.product_name = Some(value.clone());
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if let Ok(import_dir) = ImportDirectory::parse(&image) {
            for descriptor in import_dir.descriptors {
                if let Ok(name) = descriptor.get_name(&image) {
                    if let Ok(name_str) = name.as_str() {
                        meta.imported_modules.push(name_str.to_string());
                    }
                }
            }
        }
        Some(meta)
    }))
    .unwrap_or_else(|_| None)
    .unwrap_or_default()
}
