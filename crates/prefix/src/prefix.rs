use base::config::{PrefixConfig, RegisteredExecutable};
use base::error::{PrefixError, Result};
use base::{PrefixInfo, WinePrefix};
use log::{debug, error, info};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Arc, RwLock};

use runtime::RuntimeManager;
use store::PrefixStore;

/// A self-contained Wine prefix with shared access to global services.
///
/// Unlike the passive [`WinePrefix`] data struct, `Prefix` owns its path and
/// configuration while holding cheaply-clonable [`Arc`] references to the
/// scanner, runtime manager, and persistent store. This lets it perform
/// most operations without depending on an external [`Manager`].
///
/// Equality is based on path and config only (the semantic data),
/// not the shared service Arcs.
#[derive(Clone)]
pub struct Prefix {
    /// On-disk prefix directory path.
    pub(crate) path: PathBuf,
    /// The prefix's configuration.
    pub(crate) config: PrefixConfig,

    // ── Shared global services ──────────────────────────────────────
    pub(crate) scanner: Arc<scan::ApplicationScanner>,
    pub(crate) runtime_manager: Arc<RwLock<RuntimeManager>>,
    #[allow(dead_code)]
    pub(crate) store: Arc<PrefixStore>,
}

impl PartialEq for Prefix {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.config == other.config
    }
}

impl Prefix {
    /// Build a `Prefix` from an already-loaded [`WinePrefix`] and a [`Manager`].
    pub fn from_wine_prefix(prefix: &WinePrefix, mgr: &super::Manager) -> Self {
        Self {
            path: prefix.path.clone(),
            config: prefix.config.clone(),
            scanner: Arc::clone(&mgr.scanner),
            runtime_manager: Arc::clone(&mgr.runtime_manager),
            store: Arc::clone(&mgr.store),
        }
    }

    /// Build a `Prefix` from a path, config, and manager reference.
    ///
    /// The shared service Arcs (`scanner`, `runtime_manager`, `store`) are
    /// cloned from the manager so the resulting `Prefix` is self-contained.
    pub fn from_parts(
        path: PathBuf,
        config: PrefixConfig,
        mgr: &super::Manager,
    ) -> Self {
        Self {
            path,
            config,
            scanner: Arc::clone(&mgr.scanner),
            runtime_manager: Arc::clone(&mgr.runtime_manager),
            store: Arc::clone(&mgr.store),
        }
    }

    // ── Basic accessors ─────────────────────────────────────────────

    /// The on-disk path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Mutable path reference (for internal use within the prefix crate).
    pub(crate) fn path_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }

    /// The prefix configuration.
    pub fn config(&self) -> &PrefixConfig {
        &self.config
    }

    /// Mutable prefix configuration.
    pub fn config_mut(&mut self) -> &mut PrefixConfig {
        &mut self.config
    }

    /// Replace the path (e.g. when the prefix is moved).
    pub fn set_path(&mut self, path: PathBuf) {
        self.path = path;
    }

    /// Replace the configuration.
    pub fn set_config(&mut self, config: PrefixConfig) {
        self.config = config;
    }

    /// The prefix display name (from config).
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// UUID derived from the directory name.
    pub fn uuid(&self) -> Option<&str> {
        self.path.file_name().and_then(OsStr::to_str)
    }

    // ─── Size & info ────────────────────────────────────────────────

    /// Calculate the total on-disk size of the prefix directory.
    pub fn calculate_size(&self) -> Result<u64> {
        let total_size = walkdir::WalkDir::new(&self.path)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| entry.metadata().ok())
            .map(|metadata| metadata.len())
            .sum();
        Ok(total_size)
    }

    /// Build a [`PrefixInfo`] summary.
    pub fn to_info(&self) -> Result<PrefixInfo> {
        Ok(PrefixInfo {
            name: self.config.name.clone(),
            path: self.path.clone(),
            size: self.calculate_size()?,
            executable_count: self.config.registered_executables.len(),
            wine_version: self.config.wine_version.clone(),
            architecture: self.config.architecture.clone(),
            creation_date: self.config.creation_date,
            last_modified: self.config.last_modified,
        })
    }

    // ─── Config persistence ─────────────────────────────────────────

    /// Save the current config to the prefix directory.
    pub fn save_config(&self) -> Result<()> {
        self.config.save_to_file(&self.path)
    }

    /// Add an executable and persist.
    pub fn add_executable(&mut self, executable: RegisteredExecutable) -> Result<()> {
        self.config.add_executable(executable);
        self.save_config()
    }

    /// Remove an executable by index and persist.
    pub fn remove_executable(&mut self, index: usize) -> Result<()> {
        self.config.remove_executable(index);
        self.save_config()
    }

    // ─── Application scanning ───────────────────────────────────────

    /// Scan the prefix for installed applications.
    pub fn scan_applications(&self) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        executables.extend(self.scanner.scan_prefix(&self.path)?);
        executables.extend(self.scanner.scan_for_desktop_files(&self.path)?);
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        Ok(executables)
    }

    /// Scan the prefix asynchronously.
    pub async fn scan_applications_async(&self) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        executables.extend(self.scanner.scan_prefix_async(&self.path).await?);
        executables.extend(
            self.scanner
                .scan_for_desktop_files_async(&self.path)
                .await?,
        );
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        Ok(executables)
    }

    /// Enrich executables with extracted metadata and icons.
    ///
    /// Returns `true` if any executable was modified.
    pub fn enrich_executables(&mut self) -> bool {
        let ic = self.scanner.icon_cache();
        let mut changed = false;
        for exe in &mut self.config.registered_executables {
            if let Some(resolved) = resolve_or_extract_icon(exe, &self.path, ic) {
                if exe.icon_path.as_ref() != Some(&resolved) {
                    exe.icon_path = Some(resolved);
                    changed = true;
                }
            } else if exe.icon_path.is_some() {
                exe.icon_path = None;
                changed = true;
            }
            if exe.file_description.is_none() {
                let meta = scan::extract_metadata_for_exe(&exe.executable_path);
                if meta.file_version.is_some() || meta.file_description.is_some() {
                    exe.file_version = meta.file_version;
                    exe.product_version = meta.product_version;
                    exe.company_name = meta.company_name;
                    exe.file_description = meta.file_description;
                    exe.product_name = meta.product_name;
                    exe.imported_modules = meta.imported_modules;
                    changed = true;
                }
            }
        }
        changed
    }

    // ─── Runtime ────────────────────────────────────────────────────

    /// Resolve the runtime configured for this prefix.
    pub fn runtime(&self) -> Option<runtime::Runtime> {
        self.runtime_manager
            .read()
            .unwrap()
            .resolve(self.config.wine_version.as_deref())
            .cloned()
    }

    // ─── Wine process helpers ───────────────────────────────────────

    /// Check that a wine binary (`"wine"`, `"winecfg"`, …) is available.
    pub fn check_wine_available(&self, binary_name: &str) -> Result<()> {
        if let Some(runtime) = self.runtime() {
            if runtime.source == runtime::RuntimeSource::System {
                if find_in_path(binary_name).is_some() {
                    return Ok(());
                }
                if binary_name == "wine"
                    && (Path::new("/usr/bin/wine").exists()
                        || Path::new("/usr/local/bin/wine").exists())
                {
                    return Ok(());
                }
                return Err(PrefixError::NotFound(format!(
                    "Wine runtime 'System Wine' is configured but '{}' was not found in PATH.\n\
                     Install Wine through your package manager, or add a managed runtime \
                     in Settings → Wine Runtime.",
                    binary_name,
                )));
            }

            let bundle_bin = runtime.bundle_dir.join("bin").join(binary_name);
            if bundle_bin.exists() {
                return Ok(());
            }
            let dir = runtime.bundle_dir.display();
            return Err(PrefixError::NotFound(format!(
                "Wine runtime '{}' is configured but not found at {}.\n\
                 The runtime directory may have been deleted or moved.\n\
                 Please go to Settings → Wine Runtime and reinstall \
                 or select a different runtime.",
                runtime.name, dir,
            )));
        }

        if find_in_path(binary_name).is_some() {
            return Ok(());
        }

        if binary_name == "wine"
            && (Path::new("/usr/bin/wine").exists() || Path::new("/usr/local/bin/wine").exists())
        {
            return Ok(());
        }

        Err(PrefixError::NotFound(format!(
            "'{}' was not found on your system and no Wine runtime is configured.\n\
             Install Wine through your package manager, or add a managed runtime \
             in Settings → Wine Runtime.",
            binary_name,
        )))
    }

    /// Build a `Command` with runtime env applied (WINEPREFIX, PATH, …).
    pub fn build_wine_command(&self) -> Command {
        let mut cmd = Command::new("wine");
        if let Some(runtime) = self.runtime() {
            crate::wine_processes::apply_runtime_env(&mut cmd, &runtime, &self.path);
        } else {
            cmd.env("WINEPREFIX", &self.path);
        }
        cmd
    }

    /// Build a wine command for a named binary (e.g. `"winecfg"`).
    pub fn build_wine_command_for_exe(&self, exe: &str) -> Command {
        let mut cmd = Command::new(exe);
        if let Some(runtime) = self.runtime() {
            crate::wine_processes::apply_runtime_env(&mut cmd, &runtime, &self.path);
        } else {
            cmd.env("WINEPREFIX", &self.path);
        }
        cmd
    }

    /// Build a wine command with additional arguments.
    pub fn build_wine_command_with_args(&self, args: &[&str]) -> Command {
        let mut cmd = self.build_wine_command();
        for arg in args {
            cmd.arg(arg);
        }
        cmd
    }

    // ─── Launch operations ──────────────────────────────────────────

    /// Launch a registered executable in this prefix.
    pub fn launch_executable(&self, executable: &RegisteredExecutable) -> Result<Child> {
        if !executable.executable_path.exists() {
            error!(
                "[launch] Executable not found: {}",
                executable.executable_path.display()
            );
            return Err(PrefixError::NotFound(
                "Executable file does not exist".to_string(),
            ));
        }

        self.check_wine_available("wine")?;

        let mut cmd = self.build_wine_command_with_args(
            &[&executable.executable_path.to_string_lossy()],
        );

        info!(
            "[launch] launching '{}' in prefix '{}'",
            executable.name, self.config.name
        );

        let cmd_line: Vec<String> =
            std::iter::once(cmd.get_program().to_string_lossy().to_string())
                .chain(cmd.get_args().map(|a| a.to_string_lossy().to_string()))
                .collect();
        info!("[launch]   {}", cmd_line.join(" "));

        // Apply per-executable environment variables
        for (key, value) in &executable.env_vars {
            cmd.env(key, value);
            info!("[launch]   {}={}", key, value);
        }

        // Apply per-executable working directory (fall back to prefix path)
        if let Some(cwd) = &executable.cwd {
            cmd.current_dir(cwd);
        } else {
            cmd.current_dir(&self.path);
        }

        match cmd.spawn() {
            Ok(child) => {
                info!(
                    "[launch] '{}' started (PID: {})",
                    executable.name,
                    child.id()
                );
                Ok(child)
            }
            Err(e) => {
                error!("[launch] failed to launch '{}': {}", executable.name, e);
                Err(PrefixError::Process(format!(
                    "Failed to launch executable: {}",
                    e
                )))
            }
        }
    }

    /// Run `winecfg` for this prefix.
    pub fn run_winecfg(&self) -> Result<Child> {
        self.check_wine_available("winecfg")?;

        info!("[launch] opening winecfg for prefix '{}'", self.config.name);
        let child = self
            .build_wine_command_for_exe("winecfg")
            .current_dir(&self.path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run winecfg: {}", e)))?;
        Ok(child)
    }

    /// Run `regedit` for this prefix.
    pub fn run_regedit(&self) -> Result<Child> {
        self.check_wine_available("wine")?;

        info!("[launch] opening regedit for prefix '{}'", self.config.name);
        let child = self
            .build_wine_command_with_args(&["regedit"])
            .current_dir(&self.path)
            .spawn()
            .map_err(|e| PrefixError::Process(format!("Failed to run regedit: {}", e)))?;
        Ok(child)
    }
}

/// Search PATH for a named executable using `which`.
pub(crate) fn find_in_path(name: &str) -> Option<PathBuf> {
    let output = std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()?;
    if output.status.success() {
        Some(PathBuf::from(String::from_utf8(output.stdout).ok()?.trim()))
    } else {
        None
    }
}

/// Resolve the icon for a registered executable.
///
/// 1. Honour the user-provided `icon_path`:
///    * absolute → use directly,
///    * relative → join with `prefix_path`.
///    Returned only when the resulting file actually exists on disk.
/// 2. Otherwise fall back to extracting an icon from the executable itself
///    (using the shared `IconCache`).
pub fn resolve_or_extract_icon(
    exe: &RegisteredExecutable,
    prefix_path: &Path,
    icon_cache: &scan::IconCache,
) -> Option<PathBuf> {
    if let Some(resolved) = exe.resolve_icon_path(prefix_path) {
        debug!(
            "[apps] Using configured icon for '{}': {}",
            exe.name,
            resolved.display()
        );
        return Some(resolved);
    }
    if exe.icon_path.is_some() {
        debug!(
            "[apps] Configured icon for '{}' is missing, attempting extraction",
            exe.name
        );
    }
    scan::extract_icon_for_exe(&exe.executable_path, icon_cache)
}

/// Create a UUID display label from a prefix path (short form).
pub fn prefix_label(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| {
            if s.len() > 8 {
                format!("{}…", &s[..8])
            } else {
                s.to_string()
            }
        })
        .unwrap_or_else(|| "prefix".to_string())
}
