use base::config::PrefixConfig;
use base::GraphicsBackend;
use log::{info, warn};
use runtime::Runtime;
use runtime::graphics;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn apply_runtime_env(cmd: &mut Command, runtime: &Runtime, prefix_path: &Path) {
    cmd.env("WINEPREFIX", prefix_path);
    info!("[spawn] WINEPREFIX={}", prefix_path.to_string_lossy());
    let system_path = std::env::var("PATH").unwrap_or_default();
    let path = if runtime.bundle_dir.as_os_str().is_empty() {
        system_path.clone()
    } else if runtime.bundle_dir.join("bin").join("wine").exists() {
        // Fast path: standard bin/wine layout
        format!(
            "{}:{}",
            runtime.bundle_dir.join("bin").display(),
            system_path
        )
    } else if let Some(wine_bin) = runtime::discover_wine_binary(&runtime.bundle_dir) {
        // Handles macOS .app bundles where wine is nested inside Contents/Resources/wine/bin/wine
        let wine_dir = wine_bin
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| runtime.bundle_dir.join("bin"));
        format!("{}:{}", wine_dir.display(), system_path)
    } else {
        // Fallback: assume standard layout even if wine binary wasn't found yet
        format!(
            "{}:{}",
            runtime.bundle_dir.join("bin").display(),
            system_path
        )
    };
    let mut path = path;

    if let Some(gst_dir) = find_gstreamer_dir() {
        if let Ok(content) = std::fs::read_to_string(gst_dir.join("env")) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    if k == "PATH_PREPEND" {
                        path = format!("{}:{}", v, path);
                    } else {
                        cmd.env(k, v);
                        info!("[spawn] {}={}", k, v);
                    }
                }
            }
        }
    }

    cmd.env("PATH", &path);
    // info!("[spawn] PATH=...{}", &path[..path.len().saturating_sub(80)]);

    // Graphics backend: inject .so search path and DLL overrides
    apply_graphics_env(cmd, prefix_path);
}

/// Inject WINEDLLPATH and WINEDLLOVERRIDES for the prefix's graphics backend.
///
/// Different backends (dxmt, d3dmetal, dxvk+vkd3d) have different directory
/// layouts for their .so files. We reconstruct the `GraphicsBackend` from the
/// stored config to derive correct paths per-backend. For WINEDLLOVERRIDES,
/// we merge with any existing environment value so user customizations are
/// preserved.
fn apply_graphics_env(cmd: &mut Command, prefix_path: &Path) {
    let pb = prefix_path.to_path_buf();
    let config = match PrefixConfig::load_from_file(&pb) {
        Ok(Some(c)) => c,
        _ => return,
    };
    let gfx = match config.graphics {
        Some(ref g) => g,
        None => return,
    };

    // Reconstruct the backend enum for backend-specific path logic
    let backend = match gfx.to_backend() {
        Some(b) => b,
        None => return,
    };

    // ── WINEDLLPATH ──────────────────────────────────────────────
    // DXMT stores .so files under  dxmt-{version}/lib/wine/x86_64-unix/
    // D3DMetal (GPTK) stores .so files under  d3dmetal-{version}/lib/wine/x86_64-unix/
    // DXVK+VKD3D uses two separate directories and each may have its own .so files.
    let gfx_dir = graphics::graphics_dir();
    let so_dirs: Vec<PathBuf> = match &backend {
        GraphicsBackend::Dxmt { version } => {
            // WINEDLLPATH only for .so (unixlib) files — DXMT provides winemetal.so.
            // All .dll files are symlinked into prefix system32 as native overrides.
            // Some DXMT archives unpack to x86_64-unix/ directly, others nest under
            // lib/wine/x86_64-unix/ — detect and use whichever layout is present.
            let base = gfx_dir.join(format!("dxmt-{}", version));
            let flat = base.join("x86_64-unix");
            let nested = base.join("lib").join("wine").join("x86_64-unix");
            vec![if nested.exists() && !flat.exists() {
                nested
            } else {
                flat
            }]
        }
        GraphicsBackend::D3DMetal { version } => {
            vec![
                gfx_dir
                    .join(format!("d3dmetal-{}", version))
                    .join("lib")
                    .join("wine")
                    .join("x86_64-unix"),
            ]
        }
        GraphicsBackend::DxvkVkd3d {
            dxvk_version,
            vkd3d_version,
        } => {
            vec![
                gfx_dir
                    .join(format!("dxvk-{}", dxvk_version))
                    .join("lib")
                    .join("wine")
                    .join("x86_64-unix"),
                gfx_dir
                    .join(format!("vkd3d-{}", vkd3d_version))
                    .join("lib")
                    .join("wine")
                    .join("x86_64-unix"),
            ]
        }
    };

    // Always set WINEDLLPATH — even if a directory doesn't exist yet, we still
    // set the env var so Wine fails informatively instead of silently falling back.
    let winedllpath = so_dirs
        .into_iter()
        .map(|p| {
            let s = p.to_string_lossy().into_owned();
            if !p.exists() {
                warn!("[spawn] WINEDLLPATH dir missing: {}", s);
            }
            s
        })
        .collect::<Vec<_>>()
        .join(":");
    cmd.env("WINEDLLPATH", &winedllpath);
    info!("[spawn] WINEDLLPATH={}", winedllpath);

    // ── WINEDLLOVERRIDES ──────────────────────────────────────────
    // Ensure native DLLs (symlinked during activation) are loaded before
    // Wine's builtin DLLs. Merge with any existing environment variable
    // so user-set overrides (e.g. winemenubuilder.exe=d) are preserved.
    let override_str = backend.override_env_string();
    match std::env::var("WINEDLLOVERRIDES") {
        Ok(existing) if !existing.is_empty() => {
            // Prepend backend defaults — user's overrides (appended)
            // take precedence in Wine's left-to-right last-match-wins.
            let merged = format!("{};{}", override_str, existing);
            cmd.env("WINEDLLOVERRIDES", &merged);
            info!("[spawn] WINEDLLOVERRIDES={} (merged)", merged);
        }
        _ => {
            cmd.env("WINEDLLOVERRIDES", &override_str);
            info!("[spawn] WINEDLLOVERRIDES={}", override_str);
        }
    }

    // ── DXVK+VKD3D environment variables ──────────────────────────
    // When the backend is DXVK+VKD3D, set config file paths and state
    // cache location so the libraries can find their configuration
    // without requiring the user to set them globally.
    if let GraphicsBackend::DxvkVkd3d { .. } = &backend {
        // Point DXVK at the per-prefix dxvk.conf written during patching
        let dxvk_conf = prefix_path.join("drive_c").join("dxvk.conf");
        if dxvk_conf.exists() {
            let path = dxvk_conf.to_string_lossy().to_string();
            cmd.env("DXVK_CONFIG_FILE", &path);
            info!("[spawn] DXVK_CONFIG_FILE={}", path);
        }

        // Point VKD3D-Proton at the per-prefix vkd3d_proton.conf
        let vkd3d_conf = prefix_path.join("drive_c").join("vkd3d_proton.conf");
        if vkd3d_conf.exists() {
            let path = vkd3d_conf.to_string_lossy().to_string();
            cmd.env("VKD3D_CONFIG_FILE", &path);
            info!("[spawn] VKD3D_CONFIG_FILE={}", path);
        }

        // Enable state cache and point it to the per-prefix directory
        let cache_dir = prefix_path.join("drive_c").join("dxvk_state_cache");
        if cache_dir.is_dir() {
            cmd.env("DXVK_STATE_CACHE", "1");
            let path = cache_dir.to_string_lossy().to_string();
            cmd.env("DXVK_STATE_CACHE_PATH", &path);
            info!("[spawn] DXVK_STATE_CACHE=1, DXVK_STATE_CACHE_PATH={}", path);
        }

        // Disable HUD by default (user can override via per-executable env vars)
        if std::env::var("DXVK_HUD").is_err() {
            cmd.env("DXVK_HUD", "0");
            info!("[spawn] DXVK_HUD=0");
        }
    }
}

fn find_gstreamer_dir() -> Option<PathBuf> {
    let data_dir = dirs::data_dir()?;
    let gst_dir = data_dir.join("tequila").join("runtimes").join("gstreamer");
    if gst_dir.is_dir() {
        Some(gst_dir)
    } else {
        None
    }
}