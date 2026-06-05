use base::error::Result;
use log::{info, warn};
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

// ── macOS implementation ───────────────────────────────────────────────

/// macOS desktop integration using `.app` bundles.
///
/// Managed `.app` bundles are stored under `<base>/<prefix_uuid>/<hash>.app`.
/// Symlinks are placed in `~/Applications/` so entries appear in Launchpad
/// and the system application list.
#[cfg(target_os = "macos")]
pub struct MacOSDesktop;

#[cfg(target_os = "macos")]
impl DesktopIntegration for MacOSDesktop {
    fn create_launcher(
        &self,
        prefix_path: &Path,
        prefix_display_name: &str,
        exe_name: &str,
        exe_path: &Path,
        icon_path: Option<&Path>,
    ) -> Result<PathBuf> {
        create_app_launcher(
            prefix_path,
            prefix_display_name,
            exe_name,
            exe_path,
            icon_path,
        )
    }

    fn remove_launcher(&self, prefix_path: &Path, exe_path: &Path) -> Result<()> {
        remove_app_launcher(prefix_path, exe_path)
    }

    fn launcher_exists(&self, prefix_path: &Path, exe_path: &Path) -> bool {
        app_launcher_exists(prefix_path, exe_path)
    }

    fn list_launchers(&self, prefix_path: &Path) -> Result<Vec<PathBuf>> {
        list_app_launchers(prefix_path)
    }
}

/// Return the platform's default desktop integration.
///
/// On macOS this returns `MacOSDesktop`; on Linux it returns
/// `LinuxDesktop`.
#[cfg(target_os = "macos")]
pub fn default_integration() -> Box<dyn DesktopIntegration + Send + Sync> {
    Box::new(MacOSDesktop)
}

#[cfg(not(target_os = "macos"))]
pub fn default_integration() -> Box<dyn DesktopIntegration + Send + Sync> {
    Box::new(LinuxDesktop)
}

// ── Platform-generic convenience API ────────────────────────────────────

/// Create a launcher using the platform's default desktop integration
/// (`.desktop` file on Linux, `.app` bundle on macOS).
///
/// See [`DesktopIntegration::create_launcher`] for parameter details.
pub fn create_launcher(
    prefix_path: &Path,
    prefix_display_name: &str,
    exe_name: &str,
    exe_path: &Path,
    icon_path: Option<&Path>,
) -> Result<PathBuf> {
    default_integration().create_launcher(
        prefix_path,
        prefix_display_name,
        exe_name,
        exe_path,
        icon_path,
    )
}

/// Remove a launcher using the platform's default desktop integration.
///
/// See [`DesktopIntegration::remove_launcher`] for parameter details.
pub fn remove_launcher(prefix_path: &Path, exe_path: &Path) -> Result<()> {
    default_integration().remove_launcher(prefix_path, exe_path)
}

/// Check whether a launcher exists using the platform's default desktop
/// integration.
///
/// See [`DesktopIntegration::launcher_exists`] for parameter details.
pub fn launcher_exists(prefix_path: &Path, exe_path: &Path) -> bool {
    default_integration().launcher_exists(prefix_path, exe_path)
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

// ── macOS helper functions ─────────────────────────────────────────────

/// Tequila Applications directory for a specific prefix.
/// macOS indexes `.app` bundles inside subdirectories of `~/Applications/`,
/// so placing bundles in a per-prefix subdirectory works cleanly.
#[cfg(target_os = "macos")]
fn prefix_applications_dir(prefix_uuid: &str) -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join("Applications")
        .join("Tequila")
        .join(prefix_uuid)
}

/// Sanitize a string for use as a macOS filename component.
/// Replaces only truly forbidden characters (`:`, `/`, NUL).
#[cfg(target_os = "macos")]
fn sanitize_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '/' | ':' | '\0' => '_',
            _ => c,
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "App".to_string()
    } else {
        s
    }
}

/// Find an `.app` bundle in the given directory whose `Contents/.tequila_hash`
/// marker file matches `hash`.  Returns `None` when no match is found.
#[cfg(target_os = "macos")]
fn find_bundle_by_hash(dir: &Path, hash: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.extension().map(|e| e == "app").unwrap_or(false) && path.is_dir() {
            let hash_file = path.join("Contents").join(".tequila_hash");
            if let Ok(content) = fs::read_to_string(&hash_file) {
                if content.trim() == hash {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Create (or update) an `.app` bundle launcher for the given executable.
///
/// * `prefix_path` — full path to the prefix directory (UUID-named).
/// * `prefix_display_name` — human-readable prefix name (for the comment).
/// * `exe_name` — display name for the launcher.
/// * `exe_path` — full on-disk path to the `.exe`.
/// * `icon_path` — optional path to a PNG icon file. If provided, it will be
///   resized to standard icon sizes via `sips`, then assembled into an ICNS
///   file via the `icns` crate and placed in the bundle's Resources
///   directory.
///
/// The generated bundle calls `tequila run --uuid <prefix_uuid> <rel_exe>` via
/// a shell script. It is stored as `~/Applications/Tequila/<prefix_uuid>/<display_name>.app`
/// so macOS Launch Services picks it up automatically with a readable name.
///
/// Returns the path of the created `.app` bundle.
#[cfg(target_os = "macos")]
pub fn create_app_launcher(
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
    let exe_hash = hash_path(&relative_str);
    let tequila_bin = find_tequila_bin();

    // Prefix subdirectory: ~/Applications/Tequila/<uuid>/
    let prefix_dir = prefix_applications_dir(prefix_uuid);
    fs::create_dir_all(&prefix_dir)?;

    // If we already have a bundle for this exe path, remove it first
    if let Some(existing) = find_bundle_by_hash(&prefix_dir, &exe_hash) {
        let _ = fs::remove_dir_all(&existing);
    }

    // Determine bundle name: sanitized display name, with collision suffix
    let safe_name = sanitize_name(exe_name);
    let bundle_name = if !prefix_dir.join(format!("{}.app", safe_name)).exists() {
        format!("{}.app", safe_name)
    } else {
        let mut counter = 1;
        loop {
            let candidate = format!("{}-{}.app", safe_name, counter);
            if !prefix_dir.join(&candidate).exists() {
                break candidate;
            }
            counter += 1;
        }
    };
    let bundle_dir = prefix_dir.join(&bundle_name);
    let contents_dir = bundle_dir.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");

    fs::create_dir_all(&macos_dir)?;
    fs::create_dir_all(&resources_dir)?;

    // Create the runner shell script
    let runner_path = macos_dir.join("tequila-runner");
    let escaped_rel = relative_str.replace('\'', "'\\''");
    let runner_content = format!(
        r#"#!/bin/bash
# Launcher for {name} in Tequila prefix: {prefix}
exec "{tequila}" run --uuid {uuid} '{rel}' "$@"
"#,
        name = exe_name,
        prefix = prefix_display_name,
        tequila = tequila_bin,
        uuid = prefix_uuid,
        rel = escaped_rel,
    );
    fs::write(&runner_path, &runner_content)?;

    // Make the runner script executable
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&runner_path, fs::Permissions::from_mode(0o755))?;

    // Convert PNG icon to ICNS via icns crate + sips resizing
    let has_icon = if let Some(src) = icon_path {
        if src.exists() {
            let tmp_dir = resources_dir.join(".icon_tmp");
            let icns_path = resources_dir.join("icon.icns");

            if fs::create_dir_all(&tmp_dir).is_err() {
                warn!("[desktop] failed to create temp icon directory");
                false
            } else {
                // Generate standard icon sizes using sips for resizing
                let sizes: &[(&str, u32)] = &[
                    ("16.png", 16),
                    ("32.png", 32),
                    ("64.png", 64),
                    ("128.png", 128),
                    ("256.png", 256),
                    ("512.png", 512),
                    ("1024.png", 1024),
                ];

                let mut resized = Vec::new();
                let mut all_ok = true;
                for (name, size) in sizes {
                    let dst = tmp_dir.join(name);
                    let result = std::process::Command::new("sips")
                        .arg("-z")
                        .arg(size.to_string())
                        .arg(size.to_string())
                        .arg(src.as_os_str())
                        .arg("--out")
                        .arg(&dst)
                        .output();
                    match result {
                        Ok(out) if out.status.success() && dst.exists() => {
                            resized.push(dst);
                        }
                        _ => {
                            all_ok = false;
                            break;
                        }
                    }
                }

                if !all_ok || resized.is_empty() {
                    warn!("[desktop] failed to resize icon for {}", exe_name);
                    let _ = fs::remove_dir_all(&tmp_dir);
                    false
                } else {
                    // Build ICNS via icns crate
                    let mut family = icns::IconFamily::new();
                    let mut any_added = false;
                    for png_path in &resized {
                        match std::fs::File::open(png_path) {
                            Ok(file) => {
                                use std::io::BufReader;
                                let reader = BufReader::new(file);
                                match icns::Image::read_png(reader) {
                                    Ok(image) => {
                                        if family.add_icon(&image).is_ok() {
                                            any_added = true;
                                        }
                                    }
                                    Err(e) => {
                                        warn!(
                                            "[desktop] failed to read resized PNG {}: {}",
                                            png_path.display(),
                                            e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "[desktop] failed to open {}: {}",
                                    png_path.display(),
                                    e
                                );
                            }
                        }
                    }

                    // Clean up temp directory
                    let _ = fs::remove_dir_all(&tmp_dir);

                    if !any_added {
                        warn!("[desktop] no valid icon sizes for {}", exe_name);
                        false
                    } else if let Ok(file) = std::fs::File::create(&icns_path) {
                        use std::io::BufWriter;
                        let writer = BufWriter::new(file);
                        match family.write(writer) {
                            Ok(()) => {
                                info!("[desktop] generated ICNS icon for {}", exe_name);
                                true
                            }
                            Err(e) => {
                                warn!("[desktop] failed to write ICNS for {}: {}", exe_name, e);
                                false
                            }
                        }
                    } else {
                        warn!("[desktop] failed to create ICNS file for {}", exe_name);
                        false
                    }
                }
            }
        } else {
            false
        }
    } else {
        false
    };

    // Build Info.plist
    let bundle_id = format!("com.tequila.launcher.{}.{}", prefix_uuid, exe_hash);
    let icon_key = if has_icon {
        "    <key>CFBundleIconFile</key>\n    <string>icon</string>\n"
    } else {
        ""
    };
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>tequila-runner</string>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id}</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundleDisplayName</key>
    <string>{name}</string>
    <key>CFBundleVersion</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>{icon}
</dict>
</plist>
"#,
        bundle_id = bundle_id,
        name = exe_name,
        icon = icon_key,
    );

    fs::write(contents_dir.join("Info.plist"), &plist)?;

    // Write marker file so we can find this bundle again by exe_path hash
    fs::write(contents_dir.join(".tequila_hash"), &exe_hash)?;

    info!("[desktop] created .app bundle: {}", bundle_dir.display());

    Ok(bundle_dir)
}

/// Remove an `.app` bundle launcher for the given prefix and exe path.
#[cfg(target_os = "macos")]
pub fn remove_app_launcher(prefix_path: &Path, exe_path: &Path) -> Result<()> {
    let prefix_uuid = match prefix_path.file_name().and_then(|n| n.to_str()) {
        Some(uuid) => uuid,
        None => return Ok(()),
    };
    let relative = exe_path.strip_prefix(prefix_path).unwrap_or(exe_path);
    let exe_hash = hash_path(&relative.to_string_lossy());

    let prefix_dir = prefix_applications_dir(prefix_uuid);
    if let Some(bundle_dir) = find_bundle_by_hash(&prefix_dir, &exe_hash) {
        fs::remove_dir_all(&bundle_dir)?;
        info!("[desktop] removed .app bundle: {}", bundle_dir.display());
    }

    Ok(())
}

/// Check whether an `.app` bundle launcher exists for the given prefix and exe.
#[cfg(target_os = "macos")]
pub fn app_launcher_exists(prefix_path: &Path, exe_path: &Path) -> bool {
    let prefix_uuid = match prefix_path.file_name().and_then(|n| n.to_str()) {
        Some(uuid) => uuid,
        None => return false,
    };
    let relative = exe_path.strip_prefix(prefix_path).unwrap_or(exe_path);
    let exe_hash = hash_path(&relative.to_string_lossy());
    let prefix_dir = prefix_applications_dir(prefix_uuid);
    find_bundle_by_hash(&prefix_dir, &exe_hash).is_some()
}

/// List all managed `.app` bundle paths for a given prefix.
#[cfg(target_os = "macos")]
pub fn list_app_launchers(prefix_path: &Path) -> Result<Vec<PathBuf>> {
    let prefix_uuid = match prefix_path.file_name().and_then(|n| n.to_str()) {
        Some(uuid) => uuid,
        None => return Ok(vec![]),
    };
    let prefix_dir = prefix_applications_dir(prefix_uuid);
    if !prefix_dir.is_dir() {
        return Ok(vec![]);
    }
    let mut launchers = Vec::new();
    for entry in fs::read_dir(&prefix_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "app").unwrap_or(false) && path.is_dir() {
            launchers.push(path);
        }
    }
    Ok(launchers)
}
