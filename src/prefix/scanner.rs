use crate::prefix::config::RegisteredExecutable;
use std::path::PathBuf;
use walkdir::WalkDir;
use std::fs;

pub struct ApplicationScanner {
    app_dirs: Vec<&'static str>,
    executable_extensions: Vec<&'static str>,
    icon_extensions: Vec<&'static str>,
}

impl ApplicationScanner {
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

    pub fn scan_prefix(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>, Box<dyn std::error::Error>> {
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
        
        // Remove duplicates and sort
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        
        Ok(executables)
    }

    fn scan_directory(&self, dir_path: &PathBuf) -> Result<Vec<RegisteredExecutable>, Box<dyn std::error::Error>> {
        let mut executables = Vec::new();
        
        for entry in WalkDir::new(dir_path)
            .max_depth(3)
            .into_iter()
            .flatten() 
        {
            let path = entry.path();
            
            if path.is_file() {
                if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                    if self.executable_extensions.contains(&extension.to_lowercase().as_str()) {
                        if let Some(executable) = self.create_executable_from_path(&path.to_path_buf())? {
                            executables.push(executable);
                        }
                    }
                }
            }
        }
        
        Ok(executables)
    }

    fn create_executable_from_path(&self, path: &PathBuf) -> Result<Option<RegisteredExecutable>, Box<dyn std::error::Error>> {
        // Skip system directories and common non-application executables
        let path_str = path.to_string_lossy();
        if self.should_skip_executable(&path_str) {
            return Ok(None);
        }

        // Extract name from file path
        let name = path.file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();
        
        // Try to find icon file
        let icon_path = self.find_icon_for_executable(path)?;
        
        // Try to extract description from path or file metadata
        let description = self.extract_description_from_path(path);
        
        Ok(Some(RegisteredExecutable {
            name,
            description,
            icon_path,
            executable_path: path.to_path_buf(),
        }))
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

    fn find_icon_for_executable(&self, exe_path: &PathBuf) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
        if let Some(parent) = exe_path.parent() {
            if let Some(stem) = exe_path.file_stem() {
                // Look for icon files with same name as executable
                for ext in &self.icon_extensions {
                    let icon_path = parent.join(format!("{}.{}", stem.to_str().unwrap_or(""), ext));
                    if icon_path.exists() {
                        return Ok(Some(icon_path));
                    }
                }

                // Look for common icon names in the same directory
                let common_icon_names = vec!["icon", "app", "main", "logo"];
                for icon_name in &common_icon_names {
                    for ext in &self.icon_extensions {
                        let icon_path = parent.join(format!("{}.{}", icon_name, ext));
                        if icon_path.exists() {
                            return Ok(Some(icon_path));
                        }
                    }
                }
            }
        }
        
        Ok(None)
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

    pub fn scan_for_desktop_files(&self, prefix_path: &PathBuf) -> Result<Vec<RegisteredExecutable>, Box<dyn std::error::Error>> {
        let mut executables = Vec::new();
        
        // Look for Windows desktop files and shortcuts
        let desktop_dirs = vec![
            "drive_c/users/Public/Desktop",
            "drive_c/ProgramData/Microsoft/Windows/Start Menu/Programs",
            "drive_c/users/default/Desktop",
        ];

        for desktop_dir in &desktop_dirs {
            let full_path = prefix_path.join(desktop_dir);
            if full_path.exists() {
                executables.extend(self.scan_desktop_directory(&full_path)?);
            }
        }

        Ok(executables)
    }

    fn scan_desktop_directory(&self, dir_path: &PathBuf) -> Result<Vec<RegisteredExecutable>, Box<dyn std::error::Error>> {
        let mut executables = Vec::new();
        
        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                
                if path.is_file() {
                    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
                        match extension.to_lowercase().as_str() {
                            "lnk" => {
                                // TODO: Parse Windows shortcut files
                                // For now, skip as it requires special parsing
                            }
                            "desktop" => {
                                // TODO: Parse .desktop files
                                // For now, skip as it's primarily for Linux
                            }
                            _ => {
                                // Check if it's an executable
                                if self.executable_extensions.contains(&extension.to_lowercase().as_str()) {
                                    if let Some(executable) = self.create_executable_from_path(&path.to_path_buf())? {
                                        executables.push(executable);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(executables)
    }
}

impl Default for ApplicationScanner {
    fn default() -> Self {
        Self::new()
    }
}