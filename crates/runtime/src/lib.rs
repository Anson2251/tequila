pub mod anson2251;
pub mod download;
pub mod github;
pub mod graphics;
pub mod kron4ek;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Runtime {
    pub id: String,
    pub name: String,
    pub wine_version: String,
    pub bundle_dir: PathBuf,
    pub source: RuntimeSource,
    pub graphics: Vec<base::GraphicsBackend>,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuntimeSource {
    System,
    ManagedVersion {
        source_url: String,
        /// Version identifier for update checking, e.g. `"26.2.0"` or `"11.9"`.
        /// Uses `#[serde(default)]` for backwards compat with older configs.
        #[serde(default)]
        version: String,
    },
    Imported {
        label: String,
        original_path: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeManager {
    pub runtimes: Vec<Runtime>,
    pub default_id: String,
}

impl RuntimeManager {
    pub fn new() -> Self {
        Self {
            runtimes: Vec::new(),
            default_id: String::new(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&Runtime> {
        self.runtimes.iter().find(|r| r.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Runtime> {
        self.runtimes.iter_mut().find(|r| r.id == id)
    }

    pub fn get_default(&self) -> Option<&Runtime> {
        if self.default_id.is_empty() {
            self.runtimes.iter().find(|r| r.id == "wine-system")
        } else {
            self.runtimes.iter().find(|r| r.id == self.default_id)
        }
    }

    pub fn resolve(&self, runtime_id: Option<&str>) -> Option<&Runtime> {
        match runtime_id {
            Some(id) => self.get(id).or_else(|| self.get_default()),
            None => self.get_default(),
        }
    }

    pub fn set_default(&mut self, id: &str) {
        if self.runtimes.iter().any(|r| r.id == id) {
            self.default_id = id.to_string();
        }
    }

    /// Register a managed build from an external source (e.g. Anson2251, Kron4ek).
    ///
    /// `source_id` becomes the ID prefix (e.g. `"anson2251"` → id `anson2251-26.2.0`).
    /// `display_name` is used for the runtime name (e.g. `"CrossOver"` → `"CrossOver 26.2.0"`).
    pub fn register_managed_build(
        &mut self,
        source_id: &str,
        display_name: &str,
        version: &str,
        source_url: String,
        bundle_dir: PathBuf,
    ) -> &Runtime {
        let id = format!("{}-{}", source_id, version);
        let wine_bin = discover_wine_binary(&bundle_dir);
        let wine_version = wine_bin
            .as_ref()
            .and_then(|b| run_wine_version(b))
            .unwrap_or_else(|| version.to_string());
        self.runtimes.retain(|r| r.id != id);
        self.runtimes.push(Runtime {
            id: id.clone(),
            name: format!("{} {}", display_name, version),
            wine_version,
            bundle_dir,
            source: RuntimeSource::ManagedVersion {
                source_url,
                version: version.to_string(),
            },
            graphics: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        });
        if self.default_id.is_empty() {
            self.default_id = id;
        }
        self.runtimes.last().unwrap()
    }

    pub fn register_version(
        &mut self,
        version: &str,
        source_url: String,
        bundle_dir: PathBuf,
    ) -> &Runtime {
        let id = format!("wine-{}", version);
        let wine_bin = discover_wine_binary(&bundle_dir);
        let wine_version = wine_bin
            .as_ref()
            .and_then(|b| run_wine_version(b))
            .unwrap_or_else(|| version.to_string());
        self.runtimes.retain(|r| r.id != id);
        self.runtimes.push(Runtime {
            id: id.clone(),
            name: format!("Wine {}", version),
            wine_version,
            bundle_dir,
            source: RuntimeSource::ManagedVersion {
                source_url,
                version: version.to_string(),
            },
            graphics: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        });
        if self.default_id.is_empty() {
            self.default_id = id;
        }
        self.runtimes.last().unwrap()
    }

    pub fn import_runtime(
        &mut self,
        source_path: &PathBuf,
        label: &str,
        runtimes_dir: &PathBuf,
    ) -> Result<Runtime, String> {
        let wine_bin = discover_wine_binary(source_path)
            .ok_or_else(|| "Could not find bin/wine in the selected path".to_string())?;
        let version = run_wine_version(&wine_bin)
            .ok_or_else(|| "Failed to run wine --version".to_string())?;
        let bundle_dir = wine_bin
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| {
                wine_bin
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| source_path.clone())
            });
        let sanitized = sanitize_label(label);
        let id = format!("wine-imported-{}", sanitized);
        let target_dir = runtimes_dir.join(&id);
        if target_dir.exists() {
            let _ = std::fs::remove_dir_all(&target_dir);
        }
        symlink_or_copy(&bundle_dir, &target_dir)?;
        self.runtimes.retain(|r| r.id != id);
        let runtime = Runtime {
            id: id.clone(),
            name: format!("Imported: {}", label),
            wine_version: version,
            bundle_dir: target_dir,
            source: RuntimeSource::Imported {
                label: label.to_string(),
                original_path: source_path.clone(),
            },
            graphics: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        if self.default_id.is_empty() {
            self.default_id = id;
        }
        self.runtimes.push(runtime);
        Ok(self.runtimes.last().unwrap().clone())
    }

    pub fn remove(&mut self, id: &str) {
        self.runtimes.retain(|r| r.id != id);
        if self.default_id == id {
            self.default_id = self
                .runtimes
                .first()
                .map(|r| r.id.clone())
                .unwrap_or_default();
        }
    }

    pub fn detect_system() -> Option<Runtime> {
        let output = std::process::Command::new("wine")
            .arg("--version")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let version = String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        Some(Runtime {
            id: "wine-system".to_string(),
            name: "System Wine".to_string(),
            wine_version: version,
            bundle_dir: PathBuf::new(),
            source: RuntimeSource::System,
            graphics: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Re-detect system Wine (`wine --version`) and update the runtime list.
    ///
    /// - If system Wine is found: add or update the entry with the current version.
    /// - If system Wine is gone: remove the entry from the list.
    pub fn ensure_system_runtime(&mut self) {
        let sys = Self::detect_system();

        // Remove any previous system runtime entry
        let had_system = self.runtimes.iter().any(|r| r.id == "wine-system");
        self.runtimes.retain(|r| r.id != "wine-system");

        if let Some(sys) = sys {
            if self.default_id.is_empty() || (had_system && self.default_id == "wine-system") {
                self.default_id = sys.id.clone();
            }
            self.runtimes.push(sys);
        } else if had_system && self.default_id == "wine-system" {
            // System Wine was uninstalled — pick another default
            self.default_id = self
                .runtimes
                .first()
                .map(|r| r.id.clone())
                .unwrap_or_default();
        }
    }
}

impl Default for RuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns a human-readable source label for a managed version URL.
///
/// Used by UI code to display where a runtime was downloaded from.
pub fn managed_source_label(source_url: &str) -> &str {
    if source_url.contains("Kron4ek") {
        "Kron4ek"
    } else if source_url.contains("crossover-foss-build") {
        "Anson2251"
    } else {
        "Managed"
    }
}

fn run_wine_version(wine_bin: &Path) -> Option<String> {
    let output = std::process::Command::new(wine_bin)
        .arg("--version")
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
    } else {
        None
    }
}

pub fn discover_wine_binary(path: &Path) -> Option<PathBuf> {
    let candidate = path.join("bin").join("wine");
    if candidate.is_file() {
        return Some(candidate);
    }
    if path.to_string_lossy().ends_with("app") {
        let wine = path
            .join("Contents")
            .join("Resources")
            .join("wine")
            .join("bin")
            .join("wine");
        if wine.is_file() {
            return Some(wine);
        }
    }
    for entry in walkdir::WalkDir::new(path)
        .max_depth(10)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() && entry.file_name() == "wine" {
            let parent = entry.path().parent()?;
            if parent.file_name().map(|n| n == "bin").unwrap_or(false) {
                return Some(entry.path().to_path_buf());
            }
        }
    }
    None
}

fn sanitize_label(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn symlink_or_copy(src: &Path, dest: &Path) -> Result<(), String> {
    if std::os::unix::fs::symlink(src, dest).is_ok() {
        return Ok(());
    }
    copy_dir_recursive(src, dest).map_err(|e| format!("Failed to copy runtime: {}", e))
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if ty.is_symlink() {
            let link_target = fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(&link_target, &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
