use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::prefix::error::{Result, PrefixError};
use crate::prefix::runtime::{GraphicsBackend, GraphicsConfig, Runtime};

/// Directory for graphics backends under the data dir.
pub fn graphics_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tequila")
        .join("graphics")
}

/// Conflict-aware symlink: replace existing symlinks, back up regular files.
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
        Err(_) => {} // doesn't exist
    }

    std::os::unix::fs::symlink(src, target)?;
    Ok(())
}

/// Remove symlinks matching a set of filenames from a directory.
/// Only removes symlinks, never regular files.
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

// ── GitHub Release helpers ──────────────────────────────────────────

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

    let release = client
        .get(&url)
        .send()
        .await
        .map_err(|e| PrefixError::Process(format!("GitHub API request failed: {}", e)))?
        .json::<GitHubRelease>()
        .await
        .map_err(|e| PrefixError::Process(format!("Failed to parse GitHub release: {}", e)))?;

    Ok(release)
}

// ── DXMT ─────────────────────────────────────────────────────────────

/// Known DXMT DLL filenames.
const DXMT_DLLS: &[&str] = &["winemetal.dll", "d3d11.dll", "dxgi.dll", "d3d10core.dll"];

/// Fetch the latest DXMT release info from GitHub.
/// DXMT releases use assets named `dxmt-v{VERSION}-builtin.tar.gz`.
pub async fn fetch_dxmt_release() -> Result<(String, String)> {
    let release = fetch_latest_release("3Shain", "dxmt").await?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".tar.gz"))
        .ok_or_else(|| PrefixError::NotFound("No tar.gz asset in DXMT release".to_string()))?;

    Ok((release.tag_name.clone(), asset.browser_download_url.clone()))
}

/// Download and extract DXMT into graphics/dxmt-{version}/.
pub async fn download_dxmt(
    version: &str,
    download_url: &str,
    progress: &crate::prefix::download::ProgressFn,
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
    crate::prefix::download::download_file(download_url, &archive_path, progress).await?;
    crate::prefix::download::extract_tar(&archive_path, &tmp_dir)?;

    let content_dir = find_content_dir(&tmp_dir)?;
    fs::rename(&content_dir, &dest_dir)?;
    let _ = fs::remove_dir_all(&tmp_dir);

    Ok(dest_dir)
}

/// Locate the Unix `.so` source directory inside a DXMT archive.
/// Handles both `lib/wine/x86_64-unix/` (nested) and `x86_64-unix/` (flat).
fn find_dxmt_so_dir(dxmt_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        dxmt_dir.join("lib").join("wine").join("x86_64-unix"),
        dxmt_dir.join("x86_64-unix"),
    ];
    candidates.into_iter().find(|d| d.is_dir())
}

/// Locate the Windows `.dll` source directory inside a DXMT archive.
fn find_dxmt_dll_dir(dxmt_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        dxmt_dir.join("lib").join("wine").join("x86_64-windows"),
        dxmt_dir.join("x86_64-windows"),
    ];
    candidates.into_iter().find(|d| d.is_dir())
}

/// Activate DXMT for a runtime: symlink `.so` into `<runtime>/lib/wine/x86_64-unix/`.
pub fn activate_dxmt_for_runtime(dxmt_dir: &Path, runtime: &Runtime) -> Result<()> {
    let so_src = find_dxmt_so_dir(dxmt_dir)
        .ok_or_else(|| PrefixError::NotFound("No x86_64-unix dir in DXMT archive".to_string()))?;

    let target_dir = runtime
        .bundle_dir
        .join("lib")
        .join("wine")
        .join("x86_64-unix");
    fs::create_dir_all(&target_dir)?;

    for entry in fs::read_dir(&so_src)? {
        let entry = entry?;
        let fname = entry.file_name();
        if fname.to_string_lossy().ends_with(".so") {
            install_symlink(&entry.path(), &target_dir.join(&fname))?;
        }
    }
    Ok(())
}

/// Activate DXMT for a prefix: symlink .dll files into drive_c/windows/system32/.
pub fn activate_dxmt_for_prefix(dxmt_dir: &Path, prefix_path: &Path) -> Result<()> {
    let dll_src = find_dxmt_dll_dir(dxmt_dir)
        .ok_or_else(|| PrefixError::NotFound("No x86_64-windows dir in DXMT archive".to_string()))?;

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

/// Deactivate DXMT for a prefix: remove symlinked dlls.
pub fn deactivate_dxmt_for_prefix(prefix_path: &Path) -> Result<()> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    remove_symlinks(&system32, DXMT_DLLS)?;
    Ok(())
}

// ── D3DMetal (GPTK) ──────────────────────────────────────────────────

/// Extract D3DMetal from a Game Porting Toolkit DMG file.
///
/// Handles both single DMG (GPTK 1.x/2.x: `lib/external/`) and nested DMG
/// (GPTK 3.x: outer DMG contains inner `Evaluation environment for Windows games`
/// DMG which has `redist/lib/external/`).
/// Returns the path to the extracted graphics/d3dmetal-{version}/ directory.
pub fn extract_d3dmetal_from_dmg(dmg_path: &Path, version: &str) -> Result<PathBuf> {
    let gfx_dir = graphics_dir();
    let dest_dir = gfx_dir.join(format!("d3dmetal-{}", version));

    if dest_dir.exists() {
        return Ok(dest_dir);
    }

    let base_mount = PathBuf::from("/tmp").join(format!("gptk_mount_{}", std::process::id()));

    // Mount the outer DMG
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

    // Look for `redist/lib/external/` (GPTK 3.x nested DMG) or `lib/external/` (GPTK 1.x/2.x)
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
                Err(PrefixError::Process("Failed to copy D3DMetal files".to_string()))
            } else {
                Ok(dest_dir.clone())
            }
        }
        None => Err(PrefixError::NotFound(
            "Could not find lib/external or redist/lib/external in DMG".to_string(),
        )),
    };

    // Always detach
    let _ = Command::new("hdiutil").arg("detach").arg(&base_mount).status();

    result
}

/// Search for the `lib/external/` directory inside a mounted DMG.
/// GPTK 3.x nests an inner DMG; this function handles both cases.
fn find_external_in_dmg(mount: &Path) -> Option<PathBuf> {
    // 1. Direct: lib/external (GPTK 1.x/2.x)
    let direct = mount.join("lib").join("external");
    if direct.is_dir() {
        return Some(direct);
    }

    // 2. Nested: redist/lib/external (GPTK 3.x)
    let redist = mount.join("redist").join("lib").join("external");
    if redist.is_dir() {
        return Some(redist);
    }

    // 3. Search for an inner DMG and try mounting it
    if let Some(inner_dmg) = find_inner_dmg(mount) {
        let inner_mount = mount.parent()?.join(format!("gptk_inner_{}", std::process::id()));
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
                // Check redist/lib/external inside inner DMG
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
                    // Copy the content out to a temp location before detaching
                    return result;
                }

                // Detach inner mount before returning None
                let _ = Command::new("hdiutil").arg("detach").arg(&inner_mount).status();
            }
            let _ = std::fs::remove_dir(&inner_mount);
        }
    }

    None
}

/// Find a nested .dmg file inside a mounted volume.
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

/// Activate D3DMetal for a runtime: symlink framework and .so into runtime's lib/.
pub fn activate_d3dmetal_for_runtime(d3dmetal_dir: &Path, runtime: &Runtime) -> Result<()> {
    let runtime_lib = runtime.bundle_dir.join("lib");
    fs::create_dir_all(&runtime_lib)?;

    for entry in walkdir::WalkDir::new(d3dmetal_dir)
        .max_depth(4)
        .into_iter()
        .flatten()
    {
        let path = entry.path();
        let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if fname.ends_with(".so") || fname.ends_with(".framework") {
            let rel = path.strip_prefix(d3dmetal_dir).unwrap();
            let target = runtime_lib.join(rel);
            install_symlink(path, &target)?;
        }
    }

    Ok(())
}

/// Activate D3DMetal for a prefix: symlink framework and dlls into system32.
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
            let target = system32.join(rel);
            install_symlink(path, &target)?;
        }
    }

    Ok(())
}

// ── DXVK + VKD3D (Linux) ─────────────────────────────────────────────

// DXVK archives contain x64/ and x32/ subdirectories with DLLs only (no .so).
const DXVK_DLLS: &[&str] = &[
    "d3d8.dll", "d3d9.dll", "d3d10core.dll", "d3d11.dll", "dxgi.dll",
];
// VKD3D-Proton archives contain x64/ and x32/ subdirectories.
const VKD3D_DLLS: &[&str] = &["d3d12.dll", "d3d12core.dll"];

/// Fetch latest DXVK release from GitHub. Asset: `dxvk-{version}.tar.gz`.
pub async fn fetch_dxvk_release() -> Result<(String, String)> {
    let release = fetch_latest_release("doitsujin", "dxvk").await?;
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".tar.gz") && !a.name.contains("native"))
        .ok_or_else(|| PrefixError::NotFound("No tar.gz asset in DXVK release".to_string()))?;
    Ok((release.tag_name.clone(), asset.browser_download_url.clone()))
}

/// Fetch latest VKD3D-Proton release from GitHub. Asset: `vkd3d-proton-{version}.tar.zst`.
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

/// Download and extract a DXVK or VKD3D-Proton archive into graphics/.
/// Uses `.tar.zst` decompression for VKD3D, normal tar for DXVK.
pub async fn download_linux_backend(
    name: &str,
    version: &str,
    download_url: &str,
    is_zst: bool,
    progress: &crate::prefix::download::ProgressFn,
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
    crate::prefix::download::download_file(download_url, &archive_path, progress).await?;

    if is_zst {
        crate::prefix::download::extract_tar_zst(&archive_path, &tmp_dir)?;
    } else {
        crate::prefix::download::extract_tar(&archive_path, &tmp_dir)?;
    }

    let content_dir = find_content_dir(&tmp_dir)?;
    fs::rename(&content_dir, &dest_dir)?;
    let _ = fs::remove_dir_all(&tmp_dir);

    Ok(dest_dir)
}

// ── Per-prefix Activation ─────────────────────────────────────────────

/// Activate a graphics backend for a specific prefix.
/// Symlinks the backend's files into the prefix's system32 and the runtime's lib/.
pub fn activate_for_prefix(
    backend: &GraphicsBackend,
    runtime: &Runtime,
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
            activate_dxmt_for_runtime(&dxmt_dir, runtime)?;
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
            activate_d3dmetal_for_runtime(&d3dmetal_dir, runtime)?;
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
            // DXVK: DLLs only (no .so) — prefix system32/syswow64 only
            activate_dxvk_for_prefix(&dxvk_dir, prefix_path)?;
            // VKD3D-Proton: .so → runtime lib/; .dll → prefix system32/syswow64
            activate_vkd3d_for_runtime(&vkd3d_dir, runtime)?;
            activate_vkd3d_for_prefix(&vkd3d_dir, prefix_path)?;
            Ok(GraphicsConfig {
                backend: "dxvk-vkd3d".to_string(),
                version: format!("dxvk-{}+vkd3d-{}", dxvk_version, vkd3d_version),
            })
        }
    }
}

/// Deactivate the current graphics backend for a prefix.
pub fn deactivate_for_prefix(config: &GraphicsConfig, prefix_path: &Path) -> Result<()> {
    match config.backend.as_str() {
        "dxmt" => deactivate_dxmt_for_prefix(prefix_path),
        "d3dmetal" => {
            let system32 = prefix_path.join("drive_c").join("windows").join("system32");
            if let Ok(entries) = fs::read_dir(&system32) {
                for entry in entries.flatten() {
                    let fname = entry.file_name().to_string_lossy().into_owned();
                    let path = entry.path();
                    if let Ok(m) = path.symlink_metadata() {
                        if m.file_type().is_symlink()
                            && (fname.contains("D3DMetal")
                                || fname.contains("xremetal")
                                || fname.contains("nvngx"))
                        {
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
            Ok(())
        }
        "dxvk-vkd3d" => {
            let system32 = prefix_path.join("drive_c").join("windows").join("system32");
            let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
            // Remove DXVK symlinks
            for dll in DXVK_DLLS {
                for dir in [&system32, &syswow64] {
                    if let Ok(m) = dir.join(dll).symlink_metadata() {
                        if m.file_type().is_symlink() {
                            let _ = fs::remove_file(dir.join(dll));
                        }
                    }
                }
            }
            // Remove VKD3D symlinks
            for dll in VKD3D_DLLS {
                for dir in [&system32, &syswow64] {
                    if let Ok(m) = dir.join(dll).symlink_metadata() {
                        if m.file_type().is_symlink() {
                            let _ = fs::remove_file(dir.join(dll));
                        }
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────

/// Find the content directory inside an extracted archive.
fn find_content_dir(tmp_dir: &Path) -> Result<PathBuf> {
    for entry in fs::read_dir(tmp_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
            return Ok(entry.path());
        }
    }
    Ok(tmp_dir.to_path_buf())
}

/// Activate DXVK .dll files for a prefix. DXVK has only DLLs (no native .so).
/// x64/*.dll → system32/, x32/*.dll → syswow64/
fn activate_dxvk_for_prefix(dxvk_dir: &Path, prefix_path: &Path) -> Result<()> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    fs::create_dir_all(&system32)?;
    fs::create_dir_all(&syswow64)?;

    for arch_dir in ["x64", "x32"] {
        let src_dir = dxvk_dir.join(arch_dir);
        if !src_dir.is_dir() {
            continue;
        }
        let target = if arch_dir == "x64" { &system32 } else { &syswow64 };
        for dll in DXVK_DLLS {
            let src = src_dir.join(dll);
            if src.exists() {
                install_symlink(&src, &target.join(dll))?;
            }
        }
    }
    Ok(())
}

/// Activate VKD3D-Proton for a runtime: symlink native .so into runtime lib/.
fn activate_vkd3d_for_runtime(vkd3d_dir: &Path, runtime: &Runtime) -> Result<()> {
    let runtime_lib = runtime.bundle_dir.join("lib");
    fs::create_dir_all(&runtime_lib)?;

    // Look for libvkd3d-proton.so in x64/ or top-level
    for entry in walkdir::WalkDir::new(vkd3d_dir).max_depth(3).into_iter().flatten() {
        let fname = entry.file_name().to_string_lossy();
        if fname.ends_with(".so") {
            let target = runtime_lib.join(entry.file_name());
            install_symlink(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Activate VKD3D-Proton .dll files for a prefix.
fn activate_vkd3d_for_prefix(vkd3d_dir: &Path, prefix_path: &Path) -> Result<()> {
    let system32 = prefix_path.join("drive_c").join("windows").join("system32");
    let syswow64 = prefix_path.join("drive_c").join("windows").join("syswow64");
    fs::create_dir_all(&system32)?;
    fs::create_dir_all(&syswow64)?;

    for arch_dir in ["x64", "x32"] {
        let src_dir = vkd3d_dir.join(arch_dir);
        if !src_dir.is_dir() {
            continue;
        }
        let target = if arch_dir == "x64" { &system32 } else { &syswow64 };
        for dll in VKD3D_DLLS {
            let src = src_dir.join(dll);
            if src.exists() {
                install_symlink(&src, &target.join(dll))?;
            }
        }
    }
    Ok(())
}
