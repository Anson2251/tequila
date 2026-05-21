use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::fs;

/// A Wine runtime installation managed by Tequila.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Runtime {
    pub id: String,
    pub name: String,
    pub wine_version: String,
    pub bundle_dir: PathBuf,
    pub source: RuntimeSource,
    pub graphics: Vec<GraphicsBackend>,
    pub installed_at: String, // ISO 8601 date
}

/// Where a runtime came from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuntimeSource {
    System,
    ManagedChannel {
        channel: Channel,
        installed_cask_version: String,
    },
    ManagedVersion {
        source_url: String,
    },
    Imported {
        label: String,
        original_path: PathBuf,
    },
}

/// macOS Homebrew cask channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Channel {
    Stable,
    Devel,
    Staging,
}

impl Channel {
    pub fn cask_name(&self) -> &'static str {
        match self {
            Channel::Stable => "wine-stable",
            Channel::Devel => "wine@devel",
            Channel::Staging => "wine@staging",
        }
    }

    pub fn runtime_id(&self) -> &'static str {
        match self {
            Channel::Stable => "wine-stable",
            Channel::Devel => "wine-devel",
            Channel::Staging => "wine-staging",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Channel::Stable => "Stable",
            Channel::Devel => "Devel",
            Channel::Staging => "Staging",
        }
    }
}

/// Graphics translation backends installed for a runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GraphicsBackend {
    Dxmt { version: String },
    D3DMetal { version: String },
    DxvkVkd3d { dxvk_version: String, vkd3d_version: String },
}

/// Per-prefix graphics configuration, stored in tequila-config.json.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphicsConfig {
    pub backend: String, // "dxmt" | "d3dmetal" | "dxvk-vkd3d"
    pub version: String, // upstream version string
}

/// Manages all Wine runtimes and the global default.
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

    /// Resolve the runtime for a given prefix's stored runtime id.
    /// Falls back to the global default if the runtime no longer exists.
    pub fn resolve(&self, runtime_id: Option<&str>) -> Option<&Runtime> {
        match runtime_id {
            Some(id) => self.get(id).or_else(|| self.get_default()),
            None => self.get_default(),
        }
    }

    /// Set the global default runtime.
    pub fn set_default(&mut self, id: &str) {
        if self.runtimes.iter().any(|r| r.id == id) {
            self.default_id = id.to_string();
        }
    }

    /// Register a managed channel runtime (macOS) after successful download.
    pub fn register_channel(
        &mut self,
        channel: Channel,
        installed_cask_version: String,
        bundle_dir: PathBuf,
    ) -> &Runtime {
        let id = channel.runtime_id().to_string();
        let wine_bin = discover_wine_binary(&bundle_dir);
        let version = wine_bin
            .as_ref()
            .and_then(|b| run_wine_version(b))
            .unwrap_or_else(|| "unknown".to_string());

        // Remove existing runtime with the same id (update in place)
        self.runtimes.retain(|r| r.id != id);

        self.runtimes.push(Runtime {
            id: id.clone(),
            name: format!("Wine ({})", channel.display_name()),
            wine_version: version,
            bundle_dir,
            source: RuntimeSource::ManagedChannel {
                channel,
                installed_cask_version,
            },
            graphics: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        });

        if self.default_id.is_empty() {
            self.default_id = id;
        }
        self.runtimes.last().unwrap()
    }

    /// Register a managed version runtime (Linux) after successful download.
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
            source: RuntimeSource::ManagedVersion { source_url },
            graphics: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        });

        if self.default_id.is_empty() {
            self.default_id = id;
        }
        self.runtimes.last().unwrap()
    }

    /// Import a Wine installation from a user-provided path.
    /// Discovers bin/wine (handles .app bundles on macOS), runs --version,
    /// symlinks into runtimes/ dir, and registers as an imported runtime.
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

        // Symlink or copy into runtimes dir
        let target_dir = runtimes_dir.join(&id);
        if target_dir.exists() {
            let _ = std::fs::remove_dir_all(&target_dir);
        }

        symlink_or_copy(&bundle_dir, &target_dir)?;

        // Remove existing runtime with the same id
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

    /// Remove a runtime by id. Does not remove from disk (caller decides).
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

    /// Detect system Wine from PATH and build a System runtime entry.
    /// Returns None if wine is not found on PATH.
    pub fn detect_system() -> Option<Runtime> {
        // Just check that wine runs — we don't care where it is
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
            bundle_dir: PathBuf::new(), // system wine is already on PATH
            source: RuntimeSource::System,
            graphics: Vec::new(),
            installed_at: chrono::Utc::now().to_rfc3339(),
        })
    }

    pub fn ensure_system_runtime(&mut self) {
        if !self.runtimes.iter().any(|r| matches!(r.source, RuntimeSource::System)) {
            if let Some(sys) = Self::detect_system() {
                if self.default_id.is_empty() {
                    self.default_id = sys.id.clone();
                }
                self.runtimes.push(sys);
            }
        }
    }
}

impl Default for RuntimeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Run `wine --version` on a specific wine binary.
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
        println!("Fail to fetch version");
        None
    }
}

/// Discover the wine binary in a user-provided directory or .app bundle.
fn discover_wine_binary(path: &Path) -> Option<PathBuf> {
    // 1. Direct bin/wine
    let candidate = path.join("bin").join("wine");
    if candidate.is_file() {
        return Some(candidate);
    }

    // 2. Path itself is a .app bundle
    if path.to_string_lossy().ends_with("app") {
        let wine = path.join("Contents").join("Resources").join("wine").join("bin").join("wine");
        if wine.is_file() {
            return Some(wine);
        }
    }

    // 3. Walk as last resort
    for entry in walkdir::WalkDir::new(path).max_depth(6).into_iter().flatten() {
        if entry.file_type().is_file() && entry.file_name() == "wine" {
            let parent = entry.path().parent()?;
            if parent.file_name().map(|n| n == "bin").unwrap_or(false) {
                return Some(entry.path().to_path_buf());
            }
        }
    }

    None
}

/// Sanitize a user label for use in a runtime id.
fn sanitize_label(label: &str) -> String {
    label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Symlink `src` into `dest`. On failure, fall back to recursive copy.
fn symlink_or_copy(src: &Path, dest: &Path) -> Result<(), String> {
    if std::os::unix::fs::symlink(src, dest).is_ok() {
        return Ok(());
    }

    // Fallback: recursive copy
    copy_dir_recursive(src, dest)
        .map_err(|e| format!("Failed to copy runtime: {}", e))
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
