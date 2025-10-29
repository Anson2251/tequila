use crate::prefix::config::RegisteredExecutable;
use crate::prefix::error::{Result, PrefixError};
use crate::prefix::traits::Scanner;
use std::path::PathBuf;
use walkdir::WalkDir;
use std::fs;
use exe::pe::PE;
use exe::VecPE;
use exe::types::ImportDirectory;
use exe::types::ImportData;
use exe::types::CCharString;

/// Scanner for discovering Windows applications in Wine prefixes
///
/// This struct scans Wine prefix directories to find Windows executables
/// and their associated metadata like icons and descriptions.
#[derive(PartialEq)]
pub struct ApplicationScanner {
    /// Directories to scan for applications
    app_dirs: Vec<&'static str>,
    /// File extensions that indicate Windows executables
    executable_extensions: Vec<&'static str>,
    /// File extensions that indicate icon files
    icon_extensions: Vec<&'static str>,
}

/// Metadata extracted from Windows executables using the exe crate
#[derive(Debug, Default)]
struct ExecutableMetadata {
    file_version: Option<String>,
    product_version: Option<String>,
    company_name: Option<String>,
    file_description: Option<String>,
    product_name: Option<String>,
    imported_modules: Vec<String>,
}

impl ApplicationScanner {
    /// Create a new ApplicationScanner with default configuration
    pub fn new() -> Self {
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
        }
    }

    pub fn scan_prefix(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        
        for app_dir in &self.app_dirs {
            let full_path = prefix_path.join(app_dir);
            println!("Scanning directory: {}, {}", &full_path.display(), &prefix_path.display());
            if full_path.exists() && full_path.is_dir() {
                match self.scan_directory(&full_path) {
                    Ok(mut dir_executables) => {
                        executables.append(&mut dir_executables);
                    }
                    Err(e) => {
                        eprintln!("Error scanning directory {}: {}", full_path.display(), e);
                    }
                }
            }
        }
        
        // Remove duplicates and sort using functional programming patterns
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
                path.is_file() && path.extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| self.executable_extensions.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false)
            })
            .filter_map(|entry| {
                let path = entry.path().to_path_buf();
                match self.create_executable_from_path(&path) {
                    Ok(Some(executable)) => {
                        println!("Found executable: {} at {}", executable.name, path.display());
                        Some(executable)
                    }
                    Ok(None) => {
                        println!("Skipped executable at: {}", path.display());
                        None
                    }
                    Err(e) => {
                        eprintln!("Error processing executable at {}: {}", path.display(), e);
                        None
                    }
                }
            })
            .collect();
            
        println!("Scanned directory {}: found {} executables", dir_path.display(), executables.len());
        Ok(executables)
    }

    fn create_executable_from_path(&self, path: &PathBuf) -> Result<Option<RegisteredExecutable>> {
        // Skip system directories and common non-application executables
        let path_str = path.to_string_lossy();
        if self.should_skip_executable(&path_str) {
            return Ok(None);
        }

        // Extract name from file path using functional chaining
        let name = path.file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();
        
        // Try to find icon file
        let icon_path = self.find_icon_for_executable(path)?;
        
        // Try to extract description from path or file metadata
        let description = self.extract_description_from_path(path);
        
        // Extract rich metadata using exe crate
        let metadata = self.extract_executable_metadata(path);
        
        let mut executable = RegisteredExecutable::new(name, path.to_path_buf())
            .with_description(description.unwrap_or_default());
        
        if let Some(icon) = icon_path {
            executable = executable.with_icon_path(icon);
        }
        
        // Add extracted metadata if available
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

        // Skip common system executables
        let skip_executables = vec![
            "unins000", "unins001", "uninstall",
            "setup", "install", "update", "patch",
            "dllhost", "rundll32", "regsvr32",
            "msiexec", "wuauclt", "svchost",
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
        let parent = exe_path.parent().ok_or_else(|| PrefixError::InvalidPath("No parent directory".to_string()))?;
        let stem = exe_path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PrefixError::InvalidPath("Invalid file stem".to_string()))?;

        // Look for icon files with same name as executable
        let same_name_icon = self.icon_extensions.iter()
            .find_map(|ext| {
                let icon_path = parent.join(format!("{}.{}", stem, ext));
                icon_path.exists().then(|| icon_path)
            });

        if same_name_icon.is_some() {
            return Ok(same_name_icon);
        }

        // Look for common icon names in the same directory
        let common_icon_names = ["icon", "app", "main", "logo"];
        let common_icon = common_icon_names.iter()
            .find_map(|icon_name| {
                self.icon_extensions.iter().find_map(|ext| {
                    let icon_path = parent.join(format!("{}.{}", icon_name, ext));
                    icon_path.exists().then(|| icon_path)
                })
            });

        Ok(common_icon)
    }

    fn extract_description_from_path(&self, path: &PathBuf) -> Option<String> {
        // Try to extract description from directory structure
        let path_components: Vec<&str> = path.components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect();

        // Look for common application directories
        for (i, component) in path_components.iter().enumerate() {
            if component.to_lowercase() == "program files" || component.to_lowercase() == "program files (x86)" {
                if i + 1 < path_components.len() {
                    let app_name = path_components[i + 1];
                    if !app_name.to_lowercase().contains("common") {
                        return Some(format!("Application: {}", app_name));
                    }
                }
            }
        }

        // Try to get description from parent directory name
        if let Some(parent) = path.parent() {
            if let Some(parent_name) = parent.file_name().and_then(|n| n.to_str()) {
                if !parent_name.to_lowercase().contains("system") &&
                   !parent_name.to_lowercase().contains("windows") {
                    return Some(format!("Located in: {}", parent_name));
                }
            }
        }

        None
    }

    /// Extract rich metadata from Windows executables using the exe crate
    fn extract_executable_metadata(&self, path: &PathBuf) -> Option<ExecutableMetadata> {
        let path_str = path.to_string_lossy();
        
        // Try to load the PE file
        let image = match VecPE::from_disk_file(path_str.as_ref()) {
            Ok(image) => image,
            Err(e) => {
                println!("Failed to parse PE file {}: {}", path.display(), e);
                return None;
            }
        };

        let mut metadata = ExecutableMetadata::default();

        // Extract version information if available
        self.extract_version_info(&image, &mut metadata);
        
        // Extract imported modules
        self.extract_imported_modules(&image, &mut metadata);
        
        Some(metadata)
    }

    /// Extract version information from PE file
    fn extract_version_info(&self, image: &VecPE, metadata: &mut ExecutableMetadata) {
        // Note: Version info extraction would require additional parsing
        // For now, we'll focus on import directory which is more reliable
        // with the current exe crate version
        println!("Version info extraction not implemented yet - requires additional parsing");
    }

    /// Extract imported modules (DLLs) from PE file
    fn extract_imported_modules(&self, image: &VecPE, metadata: &mut ExecutableMetadata) {
        match ImportDirectory::parse(image) {
            Ok(import_directory) => {
                for descriptor in import_directory.descriptors {
                    if let Ok(name) = descriptor.get_name(image) {
                        if let Ok(name_str) = name.as_str() {
                            metadata.imported_modules.push(name_str.to_string());
                            println!("Found imported module: {}", name_str);
                        }
                    }
                }
            }
            Err(e) => {
                println!("Failed to parse import directory: {}", e);
            }
        }
    }

    pub fn scan_for_desktop_files(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        // Look for Windows desktop files and shortcuts
        let desktop_dirs = vec![
            "drive_c/users/Public/Desktop",
            "drive_c/ProgramData/Microsoft/Windows/Start Menu/Programs",
            "drive_c/users/default/Desktop",
        ];

        let executables: Vec<RegisteredExecutable> = desktop_dirs.iter()
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
                    .map(|ext| {
                        match ext.to_lowercase().as_str() {
                            "lnk" | "desktop" => false, // Skip for now
                            _ => self.executable_extensions.contains(&ext.to_lowercase().as_str()),
                        }
                    })
                    .unwrap_or(false)
            })
            .filter_map(|path| {
                self.create_executable_from_path(&path)
                    .ok()
                    .flatten()
            })
            .collect();

        Ok(executables)
    }
}

impl Scanner for ApplicationScanner {
    fn scan_prefix(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        
        for app_dir in &self.app_dirs {
            let full_path = prefix_path.join(app_dir);
            if full_path.exists() && full_path.is_dir() {
                match self.scan_directory(&full_path) {
                    Ok(mut dir_executables) => {
                        executables.append(&mut dir_executables);
                    }
                    Err(e) => {
                        eprintln!("Error scanning directory {}: {}", full_path.display(), e);
                    }
                }
            }
        }
        
        // Remove duplicates and sort using functional programming patterns
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        
        Ok(executables)
    }

    fn scan_for_desktop_files(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>> {
        // Look for Windows desktop files and shortcuts
        let desktop_dirs = vec![
            "drive_c/users/Public/Desktop",
            "drive_c/ProgramData/Microsoft/Windows/Start Menu/Programs",
            "drive_c/users/default/Desktop",
        ];

        let executables: Vec<RegisteredExecutable> = desktop_dirs.iter()
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
}

impl Default for ApplicationScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ApplicationScanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ApplicationScanner(app_dirs: {}, extensions: {})",
               self.app_dirs.len(),
               self.executable_extensions.len())
    }
}