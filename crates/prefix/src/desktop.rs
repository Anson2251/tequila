use base::error::Result;
use log::info;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

// ── Trait ──────────────────────────────────────────────────────────────

/// Platform-specific desktop integration (Linux `.desktop` files,
/// macOS `.app` bundles / aliases, etc.).
pub trait DesktopIntegration {
    /// Create or update a launcher for an executable.
    ///
    /// * `prefix_path` — full path to the prefix directory.
    /// * `prefix_display_name` — human-readable prefix name (for the comment).
    /// * `exe_name` — display name for the launcher.
    /// * `exe_path` — full on-disk path to the `.exe`.
    /// * `icon_path` — optional path to a PNG icon to embed.
    fn create_launcher(
        &self,
        prefix_path: &Path,
        prefix_display_name: &str,
        exe_name: &str,
        exe_path: &Path,
        icon_path: Option<&Path>,
    ) -> Result<PathBuf>;

    /// Remove a launcher for the given prefix and exe path.
    fn remove_launcher(&self, prefix_path: &Path, exe_path: &Path) -> Result<()>;

    /// Check whether a launcher already exists.
    fn launcher_exists(&self, prefix_path: &Path, exe_path: &Path) -> bool;

    /// List all managed launcher paths for a prefix.
    fn list_launchers(&self, prefix_path: &Path) -> Result<Vec<PathBuf>>;
}

// ── Linux implementation ───────────────────────────────────────────────

/// Linux desktop integration using XDG `.desktop` files.
///
/// Managed files are stored under `~/.local/share/tequila/desktop/`.
/// Symlinks are placed in `~/.local/share/applications/` so entries
/// appear in the system application menu.
pub struct LinuxDesktop;

impl DesktopIntegration for LinuxDesktop {
    fn create_launcher(
        &self,
        prefix_path: &Path,
        prefix_display_name: &str,
        exe_name: &str,
        exe_path: &Path,
        icon_path: Option<&Path>,
    ) -> Result<PathBuf> {
        create_desktop_launcher(
            prefix_path,
            prefix_display_name,
            exe_name,
            exe_path,
            icon_path,
        )
    }

    fn remove_launcher(&self, prefix_path: &Path, exe_path: &Path) -> Result<()> {
        remove_desktop_launcher(prefix_path, exe_path)
    }

    fn launcher_exists(&self, prefix_path: &Path, exe_path: &Path) -> bool {
        desktop_launcher_exists(prefix_path, exe_path)
    }

    fn list_launchers(&self, prefix_path: &Path) -> Result<Vec<PathBuf>> {
        list_desktop_launchers(prefix_path)
    }
}

/// Return the platform's default desktop integration.
///
/// On Linux this returns `LinuxDesktop`; on other platforms it returns
/// a no-op implementation that logs and skips.
pub fn default_integration() -> Box<dyn DesktopIntegration + Send + Sync> {
    Box::new(LinuxDesktop)
}

// ── Helper functions (shared logic) ────────────────────────────────────

/// Default base directory for Tequila-managed desktop files.
/// Desktop files are stored under `<base>/<prefix_uuid>/<sha256>.desktop`.
/// A symlink is placed in `~/.local/share/applications/` so the entry
/// appears in the system application menu.
pub fn desktop_base_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".local/share")
        })
        .join("tequila")
        .join("desktop")
}

/// System applications directory where symlinks are placed.
fn system_applications_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".local/share")
        })
        .join("applications")
}

/// Compute a deterministic filename-safe hash (SHA-256, hex-encoded) from
/// the UTF-8 bytes of `input`.
pub fn hash_path(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

/// Create (or update) a `.desktop` launcher for the given executable.
///
/// * `prefix_path` — full path to the prefix directory (UUID-named).
/// * `prefix_display_name` — human-readable prefix name (for the comment).
/// * `exe_name` — display name for the launcher.
/// * `exe_path` — full on-disk path to the `.exe`.
/// * `icon_path` — optional path to a PNG icon file. If provided, it will be
///   copied into the desktop management directory and referenced in the
///   `.desktop` file as an absolute path. If `None`, the `Icon` field is
///   left empty.
///
/// The generated desktop file calls `tequila run --uuid <prefix_uuid> <rel_exe>`.
/// It is stored under `desktop_base_dir() / <prefix_uuid> / <hash>.desktop`
/// and symlinked into `~/.local/share/applications/`.
///
/// Returns the path of the symlink in the system applications directory.
pub fn create_desktop_launcher(
    prefix_path: &Path,
    prefix_display_name: &str,
    exe_name: &str,
    exe_path: &Path,
    icon_path: Option<&Path>,
) -> Result<PathBuf> {
    let prefix_uuid = prefix_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            base::error::PrefixError::InvalidPath("prefix path has no directory name".to_string())
        })?;

    // Compute the exe path relative to the prefix root.
    let relative_exe = exe_path.strip_prefix(prefix_path).unwrap_or(exe_path);
    let relative_str = relative_exe.to_string_lossy();
    let quoted_exe = format!("'{}'", relative_str.replace('\'', "'\\''"));

    // Hash of the relative exe path for deterministic naming.
    let exe_hash = hash_path(&relative_str);

    // Target desktop file path: <base>/<prefix_uuid>/<hash>.desktop
    let desktop_dir = desktop_base_dir().join(prefix_uuid);
    fs::create_dir_all(&desktop_dir)?;
    let desktop_path = desktop_dir.join(format!("{}.desktop", exe_hash));

    // Copy icon to the desktop management directory (if provided)
    let desktop_icon = if let Some(src) = icon_path {
        if src.exists() {
            let ico_path = desktop_dir.join(format!("{}.png", exe_hash));
            let _ = fs::copy(src, &ico_path);
            if ico_path.exists() {
                Some(ico_path.to_string_lossy().to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // Build the .desktop content (XDG Desktop Entry spec)
    let tequila_bin = find_tequila_bin();
    let icon_ref = desktop_icon.as_deref().unwrap_or("");
    let content = format!(
        r#"[Desktop Entry]
Type=Application
Version=1.0
Name={name}
Comment=Wine prefix: {prefix}
Exec={tequila} run --uuid {uuid} {rel_exe}
Icon={icon}
Terminal=false
Categories=Game;
StartupNotify=true
"#,
        name = exe_name,
        prefix = prefix_display_name,
        tequila = tequila_bin,
        uuid = prefix_uuid,
        rel_exe = quoted_exe,
        icon = icon_ref,
    );

    fs::write(&desktop_path, &content)?;
    info!("[desktop] created launcher: {}", desktop_path.display());

    // Symlink into the system applications directory
    let apps_dir = system_applications_dir();
    fs::create_dir_all(&apps_dir)?;
    let symlink_path = apps_dir.join(format!("tequila-{}-{}.desktop", prefix_uuid, exe_hash));
    let _ = fs::remove_file(&symlink_path);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&desktop_path, &symlink_path)
            .map_err(|e| base::error::PrefixError::Io(e))?;
    }
    #[cfg(not(unix))]
    {
        fs::copy(&desktop_path, &symlink_path).map_err(|e| base::error::PrefixError::Io(e))?;
    }

    info!(
        "[desktop] symlinked launcher to: {}",
        symlink_path.display()
    );

    Ok(symlink_path)
}

/// Remove a desktop launcher for the given prefix and exe path.
pub fn remove_desktop_launcher(prefix_path: &Path, exe_path: &Path) -> Result<()> {
    let prefix_uuid = match prefix_path.file_name().and_then(|n| n.to_str()) {
        Some(uuid) => uuid,
        None => return Ok(()),
    };
    let relative = exe_path.strip_prefix(prefix_path).unwrap_or(exe_path);
    let exe_hash = hash_path(&relative.to_string_lossy());

    // Remove the managed desktop file
    let desktop_path = desktop_base_dir()
        .join(prefix_uuid)
        .join(format!("{}.desktop", exe_hash));
    if desktop_path.exists() {
        fs::remove_file(&desktop_path)?;
        info!("[desktop] removed launcher: {}", desktop_path.display());
    }

    // Remove the system symlink
    let symlink_path =
        system_applications_dir().join(format!("tequila-{}-{}.desktop", prefix_uuid, exe_hash));
    if symlink_path.exists() || symlink_path.is_symlink() {
        let _ = fs::remove_file(&symlink_path);
        info!("[desktop] removed symlink: {}", symlink_path.display());
    }

    Ok(())
}

/// Check whether a desktop launcher already exists for the given prefix and exe.
pub fn desktop_launcher_exists(prefix_path: &Path, exe_path: &Path) -> bool {
    let prefix_uuid = match prefix_path.file_name().and_then(|n| n.to_str()) {
        Some(uuid) => uuid,
        None => return false,
    };
    let relative = exe_path.strip_prefix(prefix_path).unwrap_or(exe_path);
    let exe_hash = hash_path(&relative.to_string_lossy());
    desktop_base_dir()
        .join(prefix_uuid)
        .join(format!("{}.desktop", exe_hash))
        .exists()
}

/// List all managed desktop launcher paths for a given prefix.
pub fn list_desktop_launchers(prefix_path: &Path) -> Result<Vec<PathBuf>> {
    let prefix_uuid = match prefix_path.file_name().and_then(|n| n.to_str()) {
        Some(uuid) => uuid,
        None => return Ok(vec![]),
    };
    let dir = desktop_base_dir().join(prefix_uuid);
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut launchers = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "desktop").unwrap_or(false) {
            launchers.push(path);
        }
    }
    Ok(launchers)
}

/// Locate the `tequila` binary.
fn find_tequila_bin() -> String {
    // 1. Same directory as the running binary
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let candidate = dir.join("tequila");
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }

    // 2. ~/.cargo/bin/tequila
    if let Some(home) = dirs::home_dir() {
        let candidate = home.join(".cargo").join("bin").join("tequila");
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }

    // 3–4. Standard system paths
    for p in &["/usr/local/bin/tequila", "/usr/bin/tequila"] {
        if Path::new(p).exists() {
            return p.to_string();
        }
    }

    // 5. Fall back to PATH lookup
    "tequila".to_string()
}
