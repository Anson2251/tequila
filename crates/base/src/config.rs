use crate::error::{PrefixError, Result};
use crate::graphics::GraphicsConfig;
use crate::traits::{ConfigOperations, ExecutableManager};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrefixConfig {
    pub version: String,
    pub name: String,
    pub creation_date: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub wine_version: Option<String>,
    pub architecture: String,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphics: Option<GraphicsConfig>,
    pub registered_executables: Vec<RegisteredExecutable>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisteredExecutable {
    pub name: String,
    pub description: Option<String>,
    /// Optional icon location.
    ///
    /// Resolution rules when displayed or persisted:
    /// * `Some(absolute_path)` — used as-is, must exist on disk.
    /// * `Some(relative_path)` — joined with the prefix root.
    /// * `None` — caller should fall back to extracting the icon from the
    ///   executable (see `scan::extract_icon_for_exe`).
    ///
    /// Paths that do not exist on disk are treated as if the field were
    /// `None` so the fallback path can run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_path: Option<PathBuf>,
    pub executable_path: PathBuf,
    pub file_version: Option<String>,
    pub product_version: Option<String>,
    pub company_name: Option<String>,
    pub file_description: Option<String>,
    pub product_name: Option<String>,
    #[serde(default)]
    pub imported_modules: Vec<String>,
    #[serde(default)]
    pub env_vars: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
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
            graphics: None,
            registered_executables: Vec::new(),
        }
    }

    pub fn save_to_file(&self, prefix_path: &Path) -> Result<()> {
        let config_path = prefix_path.join("tequila-config.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, json)?;
        Ok(())
    }

    pub fn load_from_file(prefix_path: &Path) -> Result<Option<Self>> {
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
        self.registered_executables
            .iter()
            .find(|exe| exe.name == name)
    }

    pub fn executables(&self) -> std::slice::Iter<'_, RegisteredExecutable> {
        self.registered_executables.iter()
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(PrefixError::Validation(
                "Prefix name cannot be empty".to_string(),
            ));
        }
        if self.architecture.is_empty() {
            return Err(PrefixError::Validation(
                "Architecture cannot be empty".to_string(),
            ));
        }
        if !["win32", "win64"].contains(&self.architecture.as_str()) {
            return Err(PrefixError::Validation(
                "Architecture must be 'win32' or 'win64'".to_string(),
            ));
        }
        for (i, exe) in self.registered_executables.iter().enumerate() {
            if exe.name.is_empty() {
                return Err(PrefixError::Validation(format!(
                    "Executable {} has empty name",
                    i
                )));
            }
            if !exe.executable_path.exists() {
                return Err(PrefixError::Validation(format!(
                    "Executable {} has non-existent path: {}",
                    i,
                    exe.executable_path.display()
                )));
            }
        }
        Ok(())
    }
}

impl ConfigOperations for PrefixConfig {
    fn save_to_file(&self, prefix_path: &Path) -> Result<()> {
        let config_path = prefix_path.join("tequila-config.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, json)?;
        Ok(())
    }

    fn load_from_file(prefix_path: &Path) -> Result<Option<Self>> {
        let config_path = prefix_path.join("tequila-config.json");
        if !config_path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(config_path)?;
        let config: PrefixConfig = serde_json::from_str(&content)?;
        Ok(Some(config))
    }

    fn validate(&self) -> Result<()> {
        PrefixConfig::validate(self)
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
        self.registered_executables
            .iter()
            .find(|exe| exe.name == name)
    }

    fn executables(&self) -> std::slice::Iter<'_, RegisteredExecutable> {
        self.registered_executables.iter()
    }
}

impl std::fmt::Display for PrefixConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PrefixConfig(name: {}, arch: {}, executables: {})",
            self.name,
            self.architecture,
            self.registered_executables.len()
        )
    }
}

impl std::fmt::Display for RegisteredExecutable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RegisteredExecutable(name: {}, path: {})",
            self.name,
            self.executable_path.display()
        )
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
            env_vars: HashMap::new(),
            cwd: None,
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

    /// Resolve the configured `icon_path` to a usable on-disk path.
    ///
    /// Behaviour:
    /// * `None` → `None` (caller should fall back to extraction).
    /// * Absolute path → returned verbatim if it exists, otherwise `None`.
    /// * Relative path → joined with `prefix_path`; returned if it exists,
    ///   otherwise `None`.
    ///
    /// This does not perform any extraction from the executable; the caller
    /// is responsible for trying `scan::extract_icon_for_exe` when this
    /// returns `None`.
    pub fn resolve_icon_path(&self, prefix_path: &Path) -> Option<PathBuf> {
        let raw = self.icon_path.as_ref()?;
        let candidate = if raw.is_absolute() {
            raw.clone()
        } else {
            prefix_path.join(raw)
        };
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    }
}

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
    env_vars: HashMap<String, String>,
    cwd: Option<PathBuf>,
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
            env_vars: HashMap::new(),
            cwd: None,
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

    pub fn env_vars(mut self, vars: HashMap<String, String>) -> Self {
        self.env_vars = vars;
        self
    }

    pub fn cwd(mut self, cwd: Option<PathBuf>) -> Self {
        self.cwd = cwd;
        self
    }

    pub fn build(self) -> std::result::Result<RegisteredExecutable, PrefixError> {
        Ok(RegisteredExecutable {
            name: self
                .name
                .ok_or_else(|| PrefixError::Validation("Name is required".to_string()))?,
            description: self.description,
            icon_path: self.icon_path,
            executable_path: self.executable_path.ok_or_else(|| {
                PrefixError::Validation("Executable path is required".to_string())
            })?,
            file_version: self.file_version,
            product_version: self.product_version,
            company_name: self.company_name,
            file_description: self.file_description,
            product_name: self.product_name,
            imported_modules: self.imported_modules,
            env_vars: self.env_vars,
            cwd: self.cwd,
        })
    }
}

impl Default for RegisteredExecutableBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn valid_config() -> PrefixConfig {
        PrefixConfig::new("Gaming".to_string(), "win64".to_string())
    }

    fn exe(name: &str, path: &Path) -> RegisteredExecutable {
        RegisteredExecutable::new(name.to_string(), path.to_path_buf())
    }

    fn config_with_exe(dir: &Path) -> PrefixConfig {
        let exe_path = dir.join("game.exe");
        fs::write(&exe_path, b"MZ").unwrap();
        let mut config = valid_config();
        config.add_executable(exe("Game", &exe_path));
        config
    }

    #[test]
    fn new_has_defaults() {
        let config = valid_config();
        assert_eq!(config.version, "1.0.0");
        assert_eq!(config.name, "Gaming");
        assert_eq!(config.architecture, "win64");
        assert!(config.registered_executables.is_empty());
        assert_eq!(config.creation_date, config.last_modified);
    }

    #[test]
    fn validate_accepts_win32_and_win64() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_with_exe(dir.path());
        config.architecture = "win32".to_string();
        config.validate().unwrap();
        config.architecture = "win64".to_string();
        config.validate().unwrap();
    }

    #[test]
    fn validate_rejects_empty_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_with_exe(dir.path());
        config.name = String::new();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn validate_rejects_unknown_architecture() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_with_exe(dir.path());
        config.architecture = "win128".to_string();
        assert!(config.validate().is_err());
        config.architecture = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_executable_with_empty_name() {
        let dir = tempfile::tempdir().unwrap();
        let exe_path = dir.path().join("game.exe");
        fs::write(&exe_path, b"MZ").unwrap();
        let mut config = valid_config();
        config.add_executable(exe("", &exe_path));
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("empty name"));
    }

    #[test]
    fn validate_rejects_missing_executable_path() {
        let mut config = valid_config();
        config.add_executable(exe("Ghost", &dir_join("nope.exe")));
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("non-existent path"));
    }

    fn dir_join(name: &str) -> PathBuf {
        PathBuf::from("/definitely/not/a/real/dir").join(name)
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config_with_exe(dir.path());
        config.description = Some("test prefix".to_string());
        config.wine_version = Some("wine-10.0".to_string());
        let mut exe = config.registered_executables.remove(0);
        exe.env_vars.insert("FOO".to_string(), "bar".to_string());
        exe.imported_modules = vec!["KERNEL32.dll".to_string()];
        config.add_executable(exe);

        config.save_to_file(dir.path()).unwrap();
        let loaded = PrefixConfig::load_from_file(dir.path())
            .unwrap()
            .expect("config should exist");
        assert_eq!(loaded, config);
    }

    #[test]
    fn load_missing_config_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(PrefixConfig::load_from_file(dir.path()).unwrap().is_none());
    }

    #[test]
    fn add_and_count_executables() {
        let dir = tempfile::tempdir().unwrap();
        let exe_path = dir.path().join("a.exe");
        fs::write(&exe_path, b"MZ").unwrap();
        let mut config = valid_config();
        config.add_executable(exe("A", &exe_path));
        config.add_executable(exe("B", &exe_path));
        assert_eq!(config.executable_count(), 2);
        assert_eq!(config.get_executable_count(), 2);
    }

    #[test]
    fn remove_executable_by_index() {
        let dir = tempfile::tempdir().unwrap();
        let exe_path = dir.path().join("a.exe");
        fs::write(&exe_path, b"MZ").unwrap();
        let mut config = valid_config();
        config.add_executable(exe("A", &exe_path));
        config.add_executable(exe("B", &exe_path));
        config.remove_executable(0);
        assert_eq!(config.executable_count(), 1);
        assert_eq!(config.registered_executables[0].name, "B");
    }

    #[test]
    fn remove_out_of_bounds_index_is_noop() {
        let mut config = valid_config();
        config.remove_executable(0);
        config.remove_executable(999);
        assert_eq!(config.executable_count(), 0);
    }

    #[test]
    fn find_executable_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let config = config_with_exe(dir.path());
        assert!(config.get_executable_by_name("Game").is_some());
        assert!(config.get_executable_by_name("missing").is_none());
        let found = config.find_executable_by_name("Game").unwrap();
        assert_eq!(found.executable_path, dir.path().join("game.exe"));
    }

    #[test]
    fn resolve_icon_path_none() {
        let dir = tempfile::tempdir().unwrap();
        let exe = RegisteredExecutable::new("A".to_string(), dir.path().join("a.exe"));
        assert!(exe.resolve_icon_path(dir.path()).is_none());
    }

    #[test]
    fn resolve_icon_path_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let icon = dir.path().join("icon.png");
        fs::write(&icon, b"png").unwrap();
        let missing = dir.path().join("missing.png");
        let exe = RegisteredExecutable::new("A".to_string(), dir.path().join("a.exe"));

        let exe = exe.with_icon_path(icon.clone());
        assert_eq!(exe.resolve_icon_path(dir.path()), Some(icon));

        let exe = exe.with_icon_path(missing);
        assert!(exe.resolve_icon_path(dir.path()).is_none());
    }

    #[test]
    fn resolve_icon_path_relative_joins_prefix() {
        let prefix = tempfile::tempdir().unwrap();
        let icons = prefix.path().join("icons");
        fs::create_dir(&icons).unwrap();
        let icon = icons.join("app.png");
        fs::write(&icon, b"png").unwrap();

        let exe = RegisteredExecutable::new("A".to_string(), prefix.path().join("a.exe"))
            .with_icon_path(PathBuf::from("icons/app.png"));
        assert_eq!(
            exe.resolve_icon_path(prefix.path()),
            Some(prefix.path().join("icons/app.png"))
        );
    }

    #[test]
    fn builder_requires_name_and_path() {
        let err = RegisteredExecutableBuilder::new().build().unwrap_err();
        assert!(err.to_string().contains("Name is required"));

        let err = RegisteredExecutableBuilder::new()
            .name("A")
            .build()
            .unwrap_err();
        assert!(err.to_string().contains("Executable path is required"));
    }

    #[test]
    fn builder_builds_complete_executable() {
        let exe = RegisteredExecutableBuilder::new()
            .name("Steam")
            .description("Valve launcher")
            .executable_path("C:\\Program Files\\Steam\\steam.exe")
            .file_version("1.2.3")
            .product_version("9.9")
            .company_name("Valve")
            .file_description("Steam Client")
            .product_name("Steam")
            .imported_modules(vec!["USER32.dll".to_string()])
            .env_vars(HashMap::from([(
                "WINEDEBUG".to_string(),
                "-all".to_string(),
            )]))
            .cwd(Some(PathBuf::from("C:\\Program Files\\Steam")))
            .build()
            .unwrap();
        assert_eq!(exe.name, "Steam");
        assert_eq!(exe.company_name.as_deref(), Some("Valve"));
        assert_eq!(exe.imported_modules, vec!["USER32.dll"]);
        assert_eq!(
            exe.env_vars.get("WINEDEBUG").map(String::as_str),
            Some("-all")
        );
    }
}
