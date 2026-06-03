use crate::download;
use base::GraphicsBackend;
use base::GraphicsConfig;
use base::error::{PrefixError, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn graphics_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tequila")
        .join("graphics")
}

pub fn install_symlink(src: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    match target.symlink_metadata() {
        Ok(m) if m.file_type().is_symlink() => {
            fs::remove_file(target)?;
        }
        Ok(_) => {
            let backup = target.with_extension("old");
            fs::rename(target, &backup)?;
        }
        Err(_) => {}
    }
    std::os::unix::fs::symlink(src, target)?;
    Ok(())
}

pub fn remove_symlinks(dir: &Path, filenames: &[&str]) -> Result<()> {
    for name in filenames {
        let target = dir.join(name);
        if let Ok(m) = target.symlink_metadata() {
            if m.file_type().is_symlink() {
                fs::remove_file(&target)?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
}
#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

async fn fetch_latest_release(owner: &str, repo: &str) -> Result<GitHubRelease> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| PrefixError::Process(format!("Failed to build HTTP client: {}", e)))?;
    let response = client.get(&url).send().await.map_err(|e| {
        PrefixError::Process(format!(
            "Network error fetching {} release info: {}. \
             Please check your internet connection or VPN/proxy settings.",
            repo, e
        ))
    })?;

    let status = response.status();
    if !status.is_success() {
        // Try to extract a human-readable error message from the response body
        let body_text = response.text().await.unwrap_or_default();
        let msg = serde_json::from_str::<serde_json::Value>(&body_text)
            .ok()
            .and_then(|v| v.get("message")?.as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| format!("HTTP {}", status));

        return Err(PrefixError::Process(format!(
            "Failed to fetch {} release information: {}\n\n\
             If you are using a VPN or proxy, try switching to a different node \
             or disabling it temporarily, as shared IPs are often rate-limited by GitHub.",
            repo, msg,
        )));
    }

    let release = response.json::<GitHubRelease>().await.map_err(|e| {
        PrefixError::Process(format!(
            "Failed to parse {} release data: {}. \
             If this persists, the release format may have changed.",
            repo, e,
        ))
    })?;
    Ok(release)
}

// All DXMT DLLs are symlinked into prefix system32 as native overrides.
// We don't patch Wine's bundle, so .so files go via WINEDLLPATH and .dll
// files go into the prefix with native,builtin overrides.
const DXMT_DLLS: &[&str] = &["winemetal.dll", "d3d11.dll", "dxgi.dll", "d3d10core.dll"];

pub async fn fetch_dxmt_release() -> Result<(String, String)> {
    let release = fetch_latest_release("3Shain", "dxmt").await?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".tar.gz"))
        .ok_or_else(|| PrefixError::NotFound("No tar.gz asset in DXMT release".to_string()))?;
    Ok((release.tag_name.clone(), asset.browser_download_url.clone()))
}

pub async fn download_dxmt(
    version: &str,
    download_url: &str,
    progress: &download::ProgressFn,
) -> Result<PathBuf> {
    let gfx_dir = graphics_dir();
    let dest_dir = gfx_dir.join(format!("dxmt-{}", version));
    let tmp_dir = gfx_dir.join(format!(".tmp-dxmt-{}", version));
    if dest_dir.exists() {
        return Ok(dest_dir);
    }
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;
    let archive_path = tmp_dir.join("dxmt.tar.gz");
    download::download_file(download_url, &archive_path, progress).await?;
    download::extract_tar(&archive_path, &tmp_dir)?;
    let content_dir = find_content_dir(&tmp_dir)?;
    fs::rename(&content_dir, &dest_dir)?;
    let _ = fs::remove_dir_all(&tmp_dir);
    Ok(dest_dir)
}

fn find_dxmt_dll_dir(dxmt_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        dxmt_dir.join("lib").join("wine").join("x86_64-windows"),
        dxmt_dir.join("x86_64-windows"),
    ];
    candidates.into_iter().find(|d| d.is_dir())
}

pub fn activate_dxmt_for_prefix(dxmt_dir: &Path, prefix_path: &Path) -> Result<()> {
    let dll_src = find_dxmt_dll_dir(dxmt_dir).ok_or_else(|| {
        PrefixError::NotFound("No x86_64-windows dir in DXMT archive".to_string())
    })?;
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    fs::create_dir_all(&system32)?;
    for name in DXMT_DLLS {
        let src = dll_src.join(name);
        if src.exists() {
            install_symlink(&src, &system32.join(name))?;
        }
    }
    Ok(())
}

pub fn deactivate_dxmt_for_prefix(prefix_path: &Path) -> Result<()> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    remove_symlinks(&system32, DXMT_DLLS)?;
    Ok(())
}

pub fn extract_d3dmetal_from_dmg(dmg_path: &Path, version: &str) -> Result<PathBuf> {
    let gfx_dir = graphics_dir();
    let dest_dir = gfx_dir.join(format!("d3dmetal-{}", version));
    if dest_dir.exists() {
        return Ok(dest_dir);
    }
    let base_mount = PathBuf::from("/tmp").join(format!("gptk_mount_{}", std::process::id()));
    let output = Command::new("hdiutil")
        .arg("attach")
        .arg(dmg_path)
        .arg("-mountpoint")
        .arg(&base_mount)
        .arg("-nobrowse")
        .output()
        .map_err(|e| PrefixError::Process(format!("Failed to run hdiutil: {}", e)))?;
    if !output.status.success() {
        return Err(PrefixError::Process(format!(
            "hdiutil attach failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let external_src = find_external_in_dmg(&base_mount);
    let result = match external_src {
        Some(src) => {
            fs::create_dir_all(&dest_dir)?;
            let status = Command::new("cp")
                .arg("-r")
                .arg(&src)
                .arg(dest_dir.to_string_lossy().as_ref())
                .status()
                .map_err(|e| PrefixError::Process(format!("Failed to copy files: {}", e)))?;
            if !status.success() {
                Err(PrefixError::Process(
                    "Failed to copy D3DMetal files".to_string(),
                ))
            } else {
                Ok(dest_dir.clone())
            }
        }
        None => Err(PrefixError::NotFound(
            "Could not find lib/external or redist/lib/external in DMG".to_string(),
        )),
    };
    let _ = Command::new("hdiutil")
        .arg("detach")
        .arg(&base_mount)
        .status();
    result
}

fn find_external_in_dmg(mount: &Path) -> Option<PathBuf> {
    let direct = mount.join("lib").join("external");
    if direct.is_dir() {
        return Some(direct);
    }
    let redist = mount.join("redist").join("lib").join("external");
    if redist.is_dir() {
        return Some(redist);
    }
    if let Some(inner_dmg) = find_inner_dmg(mount) {
        let inner_mount = mount
            .parent()?
            .join(format!("gptk_inner_{}", std::process::id()));
        if std::fs::create_dir(&inner_mount).is_ok() {
            let ok = Command::new("hdiutil")
                .arg("attach")
                .arg(&inner_dmg)
                .arg("-mountpoint")
                .arg(&inner_mount)
                .arg("-nobrowse")
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if ok {
                let inner_external = inner_mount.join("redist").join("lib").join("external");
                let inner_direct = inner_mount.join("lib").join("external");
                let result = if inner_external.is_dir() {
                    Some(inner_external)
                } else if inner_direct.is_dir() {
                    Some(inner_direct)
                } else {
                    None
                };
                if result.is_some() {
                    return result;
                }
                let _ = Command::new("hdiutil")
                    .arg("detach")
                    .arg(&inner_mount)
                    .status();
            }
            let _ = std::fs::remove_dir(&inner_mount);
        }
    }
    None
}

fn find_inner_dmg(mount: &Path) -> Option<PathBuf> {
    walkdir::WalkDir::new(mount)
        .max_depth(2)
        .into_iter()
        .flatten()
        .find(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.ends_with(".dmg"))
                .unwrap_or(false)
        })
        .map(|e| e.path().to_path_buf())
}

pub fn activate_d3dmetal_for_prefix(d3dmetal_dir: &Path, prefix_path: &Path) -> Result<()> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    fs::create_dir_all(&system32)?;
    for entry in walkdir::WalkDir::new(d3dmetal_dir)
        .max_depth(4)
        .into_iter()
        .flatten()
    {
        let path = entry.path();
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if fname.ends_with(".dll") || fname.ends_with(".framework") {
            let rel = path.strip_prefix(d3dmetal_dir).unwrap();
            install_symlink(path, &system32.join(rel))?;
        }
    }
    Ok(())
}

const DXVK_DLLS: &[&str] = &[
    "d3d8.dll",
    "d3d9.dll",
    "d3d10core.dll",
    "d3d11.dll",
    "dxgi.dll",
];
const VKD3D_DLLS: &[&str] = &["d3d12.dll", "d3d12core.dll"];

pub async fn fetch_dxvk_release() -> Result<(String, String)> {
    let release = fetch_latest_release("doitsujin", "dxvk").await?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".tar.gz") && !a.name.contains("native"))
        .ok_or_else(|| PrefixError::NotFound("No tar.gz asset in DXVK release".to_string()))?;
    Ok((release.tag_name.clone(), asset.browser_download_url.clone()))
}

pub async fn fetch_vkd3d_release() -> Result<(String, String)> {
    let release = fetch_latest_release("HansKristian-Work", "vkd3d-proton").await?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".tar.zst"))
        .ok_or_else(|| {
            PrefixError::NotFound("No tar.zst asset in VKD3D-Proton release".to_string())
        })?;
    Ok((release.tag_name.clone(), asset.browser_download_url.clone()))
}

pub async fn download_linux_backend(
    name: &str,
    version: &str,
    download_url: &str,
    is_zst: bool,
    progress: &download::ProgressFn,
) -> Result<PathBuf> {
    let gfx_dir = graphics_dir();
    let dest_dir = gfx_dir.join(format!("{}-{}", name, version));
    let tmp_dir = gfx_dir.join(format!(".tmp-{}-{}", name, version));
    if dest_dir.exists() {
        return Ok(dest_dir);
    }
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;
    let ext = if is_zst { "tar.zst" } else { "tar.gz" };
    let archive_path = tmp_dir.join(format!("{}.{}", name, ext));
    download::download_file(download_url, &archive_path, progress).await?;
    if is_zst {
        download::extract_tar_zst(&archive_path, &tmp_dir)?;
    } else {
        download::extract_tar(&archive_path, &tmp_dir)?;
    }
    let content_dir = find_content_dir(&tmp_dir)?;
    fs::rename(&content_dir, &dest_dir)?;
    let _ = fs::remove_dir_all(&tmp_dir);
    Ok(dest_dir)
}

pub fn activate_for_prefix(
    backend: &GraphicsBackend,
    prefix_path: &Path,
) -> Result<GraphicsConfig> {
    let gfx_dir = graphics_dir();
    match backend {
        GraphicsBackend::Dxmt { version } => {
            let dxmt_dir = gfx_dir.join(format!("dxmt-{}", version));
            if !dxmt_dir.exists() {
                return Err(PrefixError::NotFound(format!(
                    "DXMT {} not installed",
                    version
                )));
            }
            activate_dxmt_for_prefix(&dxmt_dir, prefix_path)?;
            Ok(GraphicsConfig {
                backend: "dxmt".to_string(),
                version: version.clone(),
            })
        }
        GraphicsBackend::D3DMetal { version } => {
            let d3dmetal_dir = gfx_dir.join(format!("d3dmetal-{}", version));
            if !d3dmetal_dir.exists() {
                return Err(PrefixError::NotFound(format!(
                    "D3DMetal {} not installed",
                    version
                )));
            }
            activate_d3dmetal_for_prefix(&d3dmetal_dir, prefix_path)?;
            Ok(GraphicsConfig {
                backend: "d3dmetal".to_string(),
                version: version.clone(),
            })
        }
        GraphicsBackend::DxvkVkd3d {
            dxvk_version,
            vkd3d_version,
        } => {
            let dxvk_dir = gfx_dir.join(format!("dxvk-{}", dxvk_version));
            let vkd3d_dir = gfx_dir.join(format!("vkd3d-{}", vkd3d_version));
            if !dxvk_dir.exists() || !vkd3d_dir.exists() {
                return Err(PrefixError::NotFound(format!(
                    "DXVK {} or VKD3D {} not installed",
                    dxvk_version, vkd3d_version
                )));
            }
            // Only symlink DLLs here. Config files and state cache
            // are handled by the caller (patch_prefix_with_dxvk_vkd3d
            // or activate_graphics_backend).
            activate_dxvk_for_prefix(&dxvk_dir, prefix_path)?;
            activate_vkd3d_for_prefix(&vkd3d_dir, prefix_path)?;
            Ok(GraphicsConfig {
                backend: "dxvk-vkd3d".to_string(),
                version: format!("dxvk-{}+vkd3d-{}", dxvk_version, vkd3d_version),
            })
        }
    }
}

pub fn deactivate_for_prefix(config: &GraphicsConfig, prefix_path: &Path) -> Result<()> {
    match config.backend.as_str() {
        "dxmt" => deactivate_dxmt_for_prefix(prefix_path),
        "d3dmetal" => {
            // D3DMetal places .dll and .framework symlinks into system32.
            // We can't enumerate a fixed DLL list (the framework dir has
            // variable structure), so we scan for symlinks pointing into
            // the D3DMetal graphics directory.
            let system32 = prefix_path.join("drive_c").join("windows").join("system32");
            let gfx_dir = graphics_dir();
            if let Ok(entries) = fs::read_dir(&system32) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Ok(m) = path.symlink_metadata() {
                        if m.file_type().is_symlink() {
                            // Resolve the symlink target and check if it
                            // points into the D3DMetal installation dir
                            if let Ok(target) = fs::read_link(&path) {
                                let target = if target.is_absolute() {
                                    target
                                } else {
                                    // Relative symlink — resolve against parent
                                    path.parent().unwrap().join(&target)
                                };
                                if target.starts_with(&gfx_dir)
                                    && target.to_string_lossy().contains("d3dmetal-")
                                {
                                    let _ = fs::remove_file(&path);
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        }
        "dxvk-vkd3d" => {
            let system32 = prefix_path.join("drive_c").join("windows").join("system32");
            let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
            for dll in DXVK_DLLS {
                for dir in [&system32, &syswow64] {
                    if let Ok(m) = dir.join(dll).symlink_metadata() {
                        if m.file_type().is_symlink() {
                            let _ = fs::remove_file(dir.join(dll));
                        }
                    }
                }
            }
            for dll in VKD3D_DLLS {
                for dir in [&system32, &syswow64] {
                    if let Ok(m) = dir.join(dll).symlink_metadata() {
                        if m.file_type().is_symlink() {
                            let _ = fs::remove_file(dir.join(dll));
                        }
                    }
                }
            }
            // Remove config files and state cache written by patch_dxvk_vkd3d_for_prefix
            let _ = unpatch_dxvk_vkd3d_config(prefix_path);
            Ok(())
        }
        _ => Ok(()),
    }
}

// ── DXVK+VKD3D configuration & state cache ──────────────────────

/// Default content of `dxvk.conf` written to the prefix's `drive_c/`.
///
/// These are sensible defaults that improve compatibility and
/// performance for most games.  Users can edit this file to
/// fine-tune per-prefix behaviour.
pub const DEFAULT_DXVK_CONF: &str = r"# DXVK configuration file
# Auto-generated by Tequila.  See:
#   https://github.com/doitsujin/dxvk/wiki/Configuration

# Enable async pipeline compilation for reduced stutter
# dxvk.enableAsync = True

# Max number of compiler threads (0 = auto)
# dxvk.numCompilerThreads = 0

# HUD is controlled via DXVK_HUD environment variable
# (set by Tequila at runtime; defaults to 0/disabled)

# Default state cache path – set by Tequila via DXVK_STATE_CACHE_PATH
dxvk.enableStateCache = True

# Tear-free vsync controls (leave at default unless you need immediate
# presentation for latency-sensitive games)
# dxvk.tearFree = True
";

/// Default content of `vkd3d_proton.conf` written to the prefix.
pub const DEFAULT_VKD3D_CONF: &str = r"# VKD3D-Proton configuration file
# Auto-generated by Tequila.  See:
#   https://github.com/HansKristian-Work/vkd3d-proton#configuration

# Enable DXR ray tracing support (requires Vulkan 1.2 with ray tracing)
# dxr.enabled = True

# Number of pipeline library variants to compile (higher = less stutter)
# vkd3d.pipelineLibraryVariants = 2
";

/// Write `dxvk.conf` into the prefix for per-prefix DXVK tuning.
///
/// The file is placed at `drive_c/dxvk.conf` inside the prefix.
/// If the file already exists it is NOT overwritten so the user
/// can customise it without their changes being lost.
pub fn write_dxvk_config(prefix_path: &Path) -> Result<()> {
    let config_path = prefix_path.join("drive_c").join("dxvk.conf");
    if config_path.exists() {
        // Preserve existing user customisations
        return Ok(());
    }
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&config_path, DEFAULT_DXVK_CONF)?;
    Ok(())
}

/// Write `vkd3d_proton.conf` into the prefix.
///
/// If the file already exists it is NOT overwritten.
pub fn write_vkd3d_config(prefix_path: &Path) -> Result<()> {
    let config_path = prefix_path.join("drive_c").join("vkd3d_proton.conf");
    if config_path.exists() {
        return Ok(());
    }
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&config_path, DEFAULT_VKD3D_CONF)?;
    Ok(())
}

/// Setup DXVK state cache directory inside the prefix.
///
/// The state cache stores compiled pipeline states between runs,
/// eliminating shader compilation stutter on subsequent launches.
/// The cache lives at `drive_c/dxvk_state_cache/`.
pub fn setup_state_cache(prefix_path: &Path) -> Result<()> {
    let cache_dir = prefix_path.join("drive_c").join("dxvk_state_cache");
    fs::create_dir_all(&cache_dir)?;
    Ok(())
}

// ── Unified DXVK+VKD3D download & patch ─────────────────────────

/// Download the latest DXVK and VKD3D-Proton releases in sequence,
/// reporting progress via the combined callback.
///
/// Returns `(dxvk_dir, vkd3d_dir, dxvk_version, vkd3d_version)`
/// on success.
pub async fn download_dxvk_vkd3d(
    progress: download::PhaseProgressFn,
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Result<(PathBuf, PathBuf, String, String)> {
    use std::sync::{Arc, Mutex};

    // Wrap in Arc<Mutex<>> so we can share the progress callback across
    // multiple closures that must be Send+Sync (required by &ProgressFn).
    let progress = Arc::new(Mutex::new(progress));

    // ── Step 1: Fetch & download DXVK ──
    {
        let p = progress.lock().unwrap();
        p(0, 2, download::InstallPhase::Download);
    }
    let (dxvk_version, dxvk_url) = fetch_dxvk_release().await?;

    if let Some(ref cancel) = cancel {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(PrefixError::Process("Cancelled".to_string()));
        }
    }

    let p = Arc::clone(&progress);
    let simple_prog: download::ProgressFn = Box::new(move |d, t| {
        let cb = p.lock().unwrap();
        cb(d, t, download::InstallPhase::Download);
    });

    let dxvk_dir =
        download_linux_backend("dxvk", &dxvk_version, &dxvk_url, false, &simple_prog).await?;

    if let Some(ref cancel) = cancel {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(PrefixError::Process("Cancelled".to_string()));
        }
    }

    // ── Step 2: Fetch & download VKD3D-Proton ──
    {
        let p = progress.lock().unwrap();
        p(1, 2, download::InstallPhase::Download);
    }
    let (vkd3d_version, vkd3d_url) = fetch_vkd3d_release().await?;

    if let Some(ref cancel) = cancel {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(PrefixError::Process("Cancelled".to_string()));
        }
    }

    let p = Arc::clone(&progress);
    let simple_prog2: download::ProgressFn = Box::new(move |d, t| {
        let cb = p.lock().unwrap();
        cb(d, t, download::InstallPhase::Download);
    });

    let vkd3d_dir =
        download_linux_backend("vkd3d", &vkd3d_version, &vkd3d_url, true, &simple_prog2).await?;

    Ok((dxvk_dir, vkd3d_dir, dxvk_version, vkd3d_version))
}

/// Apply the full DXVK+VKD3D patch to a Wine prefix.
///
/// This is the equivalent of `gameportingtoolkit patch` for
/// the DXVK+VKD3D backend.  It:
///
/// 1. Creates symlinks for DXVK DLLs (x64 → system32, x32 → syswow64)
/// 2. Creates symlinks for VKD3D-Proton DLLs
/// 3. Writes `dxvk.conf` with sensible defaults
/// 4. Writes `vkd3d_proton.conf` with sensible defaults
/// 5. Creates the DXVK state cache directory
///
/// The prefix must already exist.  Registry overrides and config file
/// saving are handled by the caller (`activate_graphics_backend`).
pub fn patch_dxvk_vkd3d_for_prefix(
    dxvk_dir: &Path,
    vkd3d_dir: &Path,
    prefix_path: &Path,
) -> Result<()> {
    activate_dxvk_for_prefix(dxvk_dir, prefix_path)?;
    activate_vkd3d_for_prefix(vkd3d_dir, prefix_path)?;
    write_dxvk_config(prefix_path)?;
    write_vkd3d_config(prefix_path)?;
    setup_state_cache(prefix_path)?;
    Ok(())
}

/// Remove DXVK+VKD3D config files that were written by
/// `patch_dxvk_vkd3d_for_prefix`.  Returns without error if
/// the files don't exist.
pub fn unpatch_dxvk_vkd3d_config(prefix_path: &Path) -> Result<()> {
    for name in &["dxvk.conf", "vkd3d_proton.conf"] {
        let path = prefix_path.join("drive_c").join(name);
        if path.exists() {
            // Only remove configs we recognise (matching our header comment)
            if let Ok(content) = fs::read_to_string(&path) {
                if content.contains("Auto-generated by Tequila") {
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }
    // Remove the state cache directory created by setup_state_cache
    let cache_dir = prefix_path.join("drive_c").join("dxvk_state_cache");
    if cache_dir.exists() {
        let _ = fs::remove_dir_all(&cache_dir);
    }
    Ok(())
}

fn find_content_dir(tmp_dir: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(tmp_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
            return Ok(entry.path());
        }
    }
    Ok(tmp_dir.to_path_buf())
}

/// Activate DXVK DLLs for a prefix.
///
/// DXVK releases use `x32` for 32-bit DLLs. Also fall back to `x86`
/// for compatibility with forks that may use the other naming convention.
fn activate_dxvk_for_prefix(dxvk_dir: &Path, prefix_path: &Path) -> Result<()> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    fs::create_dir_all(&system32)?;
    fs::create_dir_all(&syswow64)?;
    for arch_dir in ["x64", "x32", "x86"] {
        let src_dir = dxvk_dir.join(arch_dir);
        if !src_dir.is_dir() {
            continue;
        }
        let target = if arch_dir == "x64" {
            &system32
        } else {
            &syswow64
        };
        for dll in DXVK_DLLS {
            let src = src_dir.join(dll);
            if src.exists() {
                install_symlink(&src, &target.join(dll))?;
            }
        }
    }
    Ok(())
}

/// Activate VKD3D-Proton DLLs for a prefix.
///
/// Tries both `x32` and `x86` as the 32-bit arch directory name since
/// different VKD3D-Proton releases may use either convention.
fn activate_vkd3d_for_prefix(vkd3d_dir: &Path, prefix_path: &Path) -> Result<()> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    fs::create_dir_all(&system32)?;
    fs::create_dir_all(&syswow64)?;
    // VKD3D-Proton uses "x86" for 32-bit, while DXVK uses "x32".
    // Try both to be compatible with different releases.
    for arch_dir in ["x64", "x32", "x86"] {
        let src_dir = vkd3d_dir.join(arch_dir);
        if !src_dir.is_dir() {
            continue;
        }
        let target = if arch_dir == "x64" {
            &system32
        } else {
            &syswow64
        };
        for dll in VKD3D_DLLS {
            let src = src_dir.join(dll);
            if src.exists() {
                install_symlink(&src, &target.join(dll))?;
            }
        }
    }
    Ok(())
}

/// Compare dotted version strings in descending order (newest first).
/// Handles version strings that may contain non-numeric prefixes (e.g.
/// "v2.13.0" is treated as "2.13.0").
fn compare_versions_desc(a: &str, b: &str) -> std::cmp::Ordering {
    fn parse_part(s: &str) -> u32 {
        // Strip leading non-digit characters (e.g. "v2" → "2")
        let digits: String = s.chars().skip_while(|c| !c.is_ascii_digit()).collect();
        digits.parse::<u32>().unwrap_or(0)
    }

    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();
    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        let a_num = parse_part(a_part);
        let b_num = parse_part(b_part);
        if a_num != b_num {
            return b_num.cmp(&a_num);
        }
    }
    b_parts.len().cmp(&a_parts.len())
}

/// Scan the graphics pool directory and return a list of installed backends.
pub fn installed_backends() -> Vec<GraphicsBackend> {
    let dir = graphics_dir();
    if !dir.is_dir() {
        return vec![];
    }

    let mut result: Vec<GraphicsBackend> = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Some(ver) = name.strip_prefix("dxmt-") {
                        result.push(GraphicsBackend::Dxmt {
                            version: ver.to_string(),
                        });
                    } else if let Some(ver) = name.strip_prefix("d3dmetal-") {
                        result.push(GraphicsBackend::D3DMetal {
                            version: ver.to_string(),
                        });
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let mut dxvk_versions: Vec<String> = Vec::new();
        let mut vkd3d_versions: Vec<String> = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    if let Some(ver) = name.strip_prefix("dxvk-") {
                        dxvk_versions.push(ver.to_string());
                    } else if let Some(ver) = name.strip_prefix("vkd3d-") {
                        vkd3d_versions.push(ver.to_string());
                    }
                }
            }
        }
        // Sort descending so the latest versions pair together
        dxvk_versions.sort_by(|a, b| compare_versions_desc(a, b));
        vkd3d_versions.sort_by(|a, b| compare_versions_desc(a, b));
        // Pair the top N versions (matching the shorter list length)
        let n = std::cmp::min(dxvk_versions.len(), vkd3d_versions.len());
        for i in 0..n {
            result.push(GraphicsBackend::DxvkVkd3d {
                dxvk_version: dxvk_versions[i].clone(),
                vkd3d_version: vkd3d_versions[i].clone(),
            });
        }
    }

    result
}

/// Import D3DMetal from a GPTK `.dmg` file.
///
/// Mounts the DMG, finds the inner evaluation-environment DMG if present,
/// copies the `lib/` tree to `graphics_dir/d3dmetal-imported-{ts}`, and
/// unmounts everything.  This is a **blocking** function — call it from a
/// background thread.
pub fn import_d3dmetal_from_dmg(dmg_path: &Path) -> Result<PathBuf> {
    let gfx_dir = graphics_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let dest_dir = gfx_dir.join(format!("d3dmetal-imported-{}", ts));

    // ── Mount the outer DMG ──
    let base_mount = PathBuf::from("/tmp").join(format!("gptk_import_{}", std::process::id()));
    let output = Command::new("hdiutil")
        .arg("attach")
        .arg(dmg_path)
        .arg("-mountpoint")
        .arg(&base_mount)
        .arg("-nobrowse")
        .output()
        .map_err(|e| PrefixError::Process(format!("Failed to run hdiutil: {}", e)))?;
    if !output.status.success() {
        return Err(PrefixError::Process(format!(
            "hdiutil attach failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    // ── Try inner DMG first (GPTK 3.0 layout) ──
    let result = (|| -> Result<PathBuf> {
        if let Some(inner_dmg) = find_inner_dmg(&base_mount) {
            let inner_mount =
                PathBuf::from("/tmp").join(format!("gptk_import_inner_{}", std::process::id()));
            if fs::create_dir(&inner_mount).is_ok() {
                let ok = Command::new("hdiutil")
                    .arg("attach")
                    .arg(&inner_dmg)
                    .arg("-mountpoint")
                    .arg(&inner_mount)
                    .arg("-nobrowse")
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if ok {
                    let inner_result = copy_lib_dir(&inner_mount, &dest_dir);
                    let _ = Command::new("hdiutil")
                        .arg("detach")
                        .arg(&inner_mount)
                        .status();
                    let _ = fs::remove_dir(&inner_mount);
                    return inner_result.map(|_| dest_dir.clone());
                }
                let _ = fs::remove_dir(&inner_mount);
            }
        }
        // No inner DMG — copy from the outer mount directly
        copy_lib_dir(&base_mount, &dest_dir).map(|_| dest_dir.clone())
    })();

    let _ = Command::new("hdiutil")
        .arg("detach")
        .arg(&base_mount)
        .status();
    result
}

/// Import D3DMetal from a local folder (already extracted GPTK).
///
/// Copies the `lib/` directory to `graphics_dir/d3dmetal-imported-{ts}`.
pub fn import_d3dmetal_from_folder(path: &Path) -> Result<PathBuf> {
    let dest = d3dmetal_dest_dir();
    copy_lib_dir(path, &dest)?;
    Ok(dest)
}

/// Remove all installed graphics backends whose directory name starts with the given prefix.
///
/// Scans `graphics_dir()` for subdirectories matching the prefix and deletes them.
pub fn remove_backends(prefix: &str) -> Result<()> {
    let dir = graphics_dir();
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(prefix) && entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            fs::remove_dir_all(&entry.path())?;
        }
    }
    Ok(())
}

fn d3dmetal_dest_dir() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    graphics_dir().join(format!("d3dmetal-imported-{}", ts))
}

fn copy_lib_dir(mount: &Path, dest: &Path) -> Result<()> {
    let lib = [mount.join("lib"), mount.join("redist").join("lib")]
        .iter()
        .find(|p| p.is_dir())
        .cloned()
        .ok_or_else(|| {
            PrefixError::NotFound(
                "Could not find lib directory in the DMG. \
                 Make sure you selected a valid Game Porting Toolkit DMG."
                    .to_string(),
            )
        })?;

    fs::create_dir_all(dest)?;
    let status = Command::new("cp")
        .arg("-R")
        .arg(&lib)
        .arg(dest)
        .status()
        .map_err(|e| PrefixError::Process(format!("cp failed: {}", e)))?;
    if !status.success() {
        return Err(PrefixError::Process(
            "Failed to copy GPTK files to graphics directory.".to_string(),
        ));
    }
    Ok(())
}
