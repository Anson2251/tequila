# Wine Runtime Management Design

## Overview

Tequila allows managing multiple Wine installations ("runtimes"). Each prefix is assigned a runtime at creation time; the global default runtime applies to new prefixes. Per-prefix runtime selection is done via PATH environment variable injection at process spawn time, avoiding per-call changes to individual `wine`/`winecfg`/`regedit` invocation points.

Dependencies (GStreamer on macOS) follow the same approach via `GST_PLUGIN_PATH` and the platform's library path variable.

## Source

Runtimes can come from:

| Source | Description | Platform |
|---|---|---|
| System | Detected from PATH | All |
| Managed | Downloaded from the internet | All |
| Imported | User-provided path to a Wine directory or `.app` bundle | All |

### macOS

Managed runtimes are channel-based via Homebrew Cask API, one version per channel:

| Channel | Cask name | Source |
|---|---|---|
| `wine-stable` | `wine-stable` | `github.com/Gcenx/macOS_Wine_builds` |
| `wine-devel` | `wine@devel` | (same repo, different tag) |
| `wine-staging` | `wine@staging` | (same repo, different tag) |

Base URL for cask API: `https://formulae.brew.sh/api/cask/`. Each cask JSON provides `url`, `version`, and `sha256` for the latest release. Only the latest version is available per channel — no version listing.

Wine on macOS also requires GStreamer at runtime, downloaded from `api/cask/gstreamer-runtime.json` (same base URL).

### Linux

Managed runtimes have full version management. Sources:

| Source | Format | Version Listing |
|---|---|---|
| WineHQ official tarballs | `.tar.xz` | Directory listing at `https://dl.winehq.org/wine-builds/linux/` |
| Wine-GE / Proton (GitHub) | `.tar.xz` / `.tar.gz` | GitHub Releases API |

WineHQ provides prebuilt Linux tarballs organized by version at `dl.winehq.org/wine-builds/`. For gaming-oriented builds, GloriousEggroll's Wine-GE and Valve's Proton are available via GitHub Releases.

On Linux, GStreamer is a system library — no extra management needed.

## Naming Convention

Every runtime has a unique `id` that doubles as its directory name under `runtimes/`.

```
# System (always the same id)
wine-system

# Managed (macOS) — channel-based, one per channel, updated in place
wine-stable      → stable channel, latest version
wine-devel       → devel channel, latest version
wine-staging     → staging channel, latest version

# Managed (Linux) — version-based, each version is a separate runtime
wine-9.0         → WineHQ 9.0
wine-10.0        → WineHQ 10.0
wine-ge-8.0      → Wine-GE 8.0

# Imported — multiple allowed, user-chosen label
wine-imported-crossover
wine-imported-proton-9.0
```

| Source | Pattern | Example |
|---|---|---|
| System | `wine-system` | `wine-system` |
| Managed (macOS) | `wine-{channel}` | `wine-stable`, `wine-devel` |
| Managed (Linux) | `wine-{version}` | `wine-9.0`, `wine-ge-8.0` |
| Imported | `wine-imported-{label}` | `wine-imported-crossover` |

`channel` is always lowercase. `label` is user-chosen, sanitized to lowercase alphanumeric + hyphens.

## Data Model

### `Runtime`

```rust
struct Runtime {
    id: String,                    // "wine-system", "wine-stable", "wine-devel", "wine-staging", "wine-imported-xxx"
    name: String,                  // display name
    wine_version: String,          // from running `wine --version` on bundle_dir/bin/wine
    bundle_dir: PathBuf,           // runtime root; bundle_dir/bin is prepended to PATH
    source: RuntimeSource,
    graphics: Vec<GraphicsBackend>, // installed backends for this runtime
    installed_at: String,          // ISO date
}

enum RuntimeSource {
    System,                        // detected from PATH
    ManagedChannel {
        channel: Channel,          // macOS: stable / devel / staging
        installed_cask_version: String, // version from cask JSON at install time, for update checks
    },
    ManagedVersion {
        source_url: String,        // Linux: WineHQ or GitHub release URL
    },
    Imported {
        label: String,             // user-chosen label
        original_path: PathBuf,    // where the user picked it from
    },
}

enum Channel {
    Stable,   // wine-stable
    Devel,    // wine@devel
    Staging,  // wine@staging
}

enum GraphicsBackend {
    Dxmt { version: String },                     // macOS: D3D10/11 → Metal
    D3DMetal { version: String },                   // macOS: D3D11/12 → Metal (GPTK)
    DxvkVkd3d { dxvk_version: String, vkd3d_version: String }, // Linux: both installed together
}
```

### `RuntimeManager`

```rust
struct RuntimeManager {
    runtimes: Vec<Runtime>,
    default_id: String,  // which runtime is the global default
}
```

### Per-prefix Runtime

The Wine runtime is chosen at prefix creation time and stored as `wine_version` in `tequila-config.json`. It is immutable after creation — switching runtimes on an existing prefix is not supported because Wine ABIs differ across versions and graphics backends are tightly coupled to the runtime.

If the runtime no longer exists (e.g. managed runtime was deleted), fall back to `RuntimeManager.default_id` and show a warning.

#### `tequila-config.json` Spec

```jsonc
{
  "version": "1.0.0",
  "name": "My App",
  "creation_date": "2026-05-17T08:00:00Z",
  "last_modified": "2026-05-17T09:30:00Z",
  "wine_version": "wine-stable",       // runtime id, immutable after creation
  "architecture": "win64",
  "description": "",
  "graphics": {                         // optional, absent if no backend active
    "backend": "dxmt",                  // "dxmt" | "d3dmetal" | "dxvk-vkd3d"
    "version": "1.5"                    // backend version, string to preserve upstream format
  },
  "registered_executables": [
    {
      "name": "MyApp",
      "description": "",
      "icon_path": "/path/to/icon.png",
      "executable_path": "/path/to/app.exe",
      "file_version": "",
      "product_version": "",
      "company_name": "",
      "file_description": "",
      "product_name": "",
      "imported_modules": []
    }
  ]
}
```

- `wine_version`: runtime id, set at prefix creation, never changed by the app.
- `graphics`: absent when no backend is active. `backend` identifies the backend type, `version` is always a string to preserve upstream format.

### Persistence

`$XDG_CONFIG_HOME/tequila/settings.json`:

```rust
struct Settings {
    runtimes: Vec<Runtime>,
    default_id: String,
}
```

## Storage Layout

```
$XDG_DATA_HOME/tequila/
  state.db                       # PrefixStore (existing)
  icons/                         # IconCache (existing)
  runtimes/
    gstreamer/                   # shared, macOS only
      bin/  lib/  version.txt  env  setup_env.sh
    wine-stable/
      bin/       → wine, winecfg, wineserver, regedit, msiexec, ...
      lib/       → libwine, ...
    wine-devel/
      bin/  lib/  ...
    wine-imported-mybuild/
      bin/  lib/  ...
  graphics/
    dxmt-1.x/
      lib/wine/x86_64-unix/winemetal.so
      lib/wine/x86_64-windows/winemetal.dll d3d11.dll dxgi.dll
    d3dmetal-1.0/
      D3DMetal.framework/
      libxremetal.so
    dxvk-vkd3d-1.x/
      lib/wine/x86_64-unix/...
      lib/wine/x86_64-windows/...

Linux runtimes are version-keyed (e.g. `wine-9.0`, `wine-10.0`, `wine-ge-8.0`) instead of channel-keyed.
```

Imported runtimes are symlinked into the same structure for uniform access. If symlink fails (e.g. cross-filesystem), fall back to recursive copy and show a warning.

## Download & Extraction

### macOS: Channel Download

For each channel, the cask JSON directly provides the download URL and SHA256:

1. Fetch `https://formulae.brew.sh/api/cask/{cask-name}.json` (e.g. cask `wine-stable` → channel `stable`, cask `wine@devel` → channel `devel`)
2. Get `url` + `sha256` + `version` from the response
3. Download → verify SHA256 → extract → register as runtime id `wine-{channel}`

**Update detection**: At startup, fetch each installed channel's cask JSON and compare `version` against `installed_cask_version`. If a newer version is available, show an "Update available" badge in the UI. The user can trigger the download (same as initial install, reuses the download + extract flow, overwriting the existing runtime directory).

### Linux: Versioned Download

The user picks a version from a list fetched from WineHQ or GitHub (Wine-GE / Proton):

```
// WineHQ tarballs — fetch directory listing to discover versions
GET https://dl.winehq.org/wine-builds/linux/ → parse HTML directory
Download https://dl.winehq.org/wine-builds/linux/wine-{version}.tar.xz

// GitHub releases — use Releases API
GET https://api.github.com/repos/GloriousEggroll/wine-ge-custom/releases
```

Each download is registered as a separate runtime: `wine-{version}` (e.g. `wine-9.0`, `wine-10.0`).

### Download Safety

- **Temp directory**: Downloads go to `runtimes/.tmp-{id}/` first; on success, atomically renamed to `runtimes/{id}/`. Failed downloads leave a `.tmp-*` directory cleaned up on next startup.
- **Per-runtime lock**: A file lock (`runtimes/.lock-{id}`) prevents concurrent downloads/extractions for the same runtime id.

### GStreamer (macOS only)

Single shared installation. Downloaded from `https://formulae.brew.sh/api/cask/gstreamer-runtime.json`, extracted via `scripts/extract-gstreamer-pkg.sh` (embedded via `include_str!`) into `runtimes/gstreamer/`.

### Import

User picks a Wine directory (or `.app` bundle) via file chooser. The app discovers `bin/wine` (or `.app/Contents/Resources/wine/bin/wine`), runs `wine --version`, and registers as `wine-imported-{label}`.

## Process Spawn Changes

Modify `src/prefix/wine_processes.rs` to inject environment variables:

```rust
fn apply_runtime_env(cmd: &mut Command, runtime: &Runtime) {
    cmd.env("WINEPREFIX", prefix_path);

    let system_path = env::var("PATH").unwrap_or_default();
    let mut path = format!("{}:{}", runtime.bundle_dir.join("bin").display(), system_path);

    // GStreamer (macOS) — read env vars from a key=val file generated at install time
    // (not a shell script — avoids forking bash or hand-writing a sh parser)
    if let Some(gst_dir) = find_gstreamer_dir() {
        if let Ok(content) = std::fs::read_to_string(gst_dir.join("env")) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    if k == "PATH_PREPEND" {
                        path = format!("{}:{}", v, path);
                    } else {
                        cmd.env(k, v);
                    }
                }
            }
        }
    }

    cmd.env("PATH", &path);
}
```

### macOS: Rosetta Check

Wine binaries are x86_64, on Apple Silicon they require Rosetta 2. First detect the architecture, then check Rosetta:

```bash
if [[ "$(uname -m)" != "arm64" ]]; then
    echo "Intel Mac — Rosetta not needed"
    exit 0
fi

if pkgutil --files com.apple.pkg.RosettaUpdateAuto &>/dev/null; then
    echo "Rosetta installed"
else
    echo "Rosetta not installed"
fi
```

In Rust, check `std::env::consts::ARCH == "aarch64"` first. If not aarch64, skip entirely. If aarch64 and Rosetta is missing, show a dialog:

- Link to Apple SLA: `https://www.apple.com/legal/sla/`
- "Agree and Install" button runs `softwareupdate --install-rosetta --agree-to-license` with an indeterminate progress bar while the command executes.
- "Cancel" dismisses and blocks Wine launch until Rosetta is installed.

## Graphics Backend

Graphics translation layers bridge Direct3D to the platform's native graphics API. These are installed per-prefix as DLL overrides.

On macOS, DXMT and D3DMetal are alternatives — the user picks one per prefix. On Linux, DXVK and VKD3D are installed together as a full D3D translation stack.

### Installation via Symlinks

Graphics backend files are extracted into `graphics/{backend}-{version}/`, then symlinked into the runtime's `lib/` and each prefix's `system32`. This keeps the original files untouched and makes deactivation trivial.

Conflict handling when creating a symlink:

- **Target is a symlink (Tequila-managed)**: remove it, create the new symlink
- **Target is a regular file (user-installed)**: rename to `{name}.old`, create the new symlink

```rust
fn install_symlink(src: &Path, target: &Path) {
    match target.symlink_metadata() {
        Ok(m) if m.file_type().is_symlink() => {
            std::fs::remove_file(target);
        }
        Ok(_) => {
            // regular file — back it up
            let backup = target.with_extension("old");
            std::fs::rename(target, &backup);
        }
        Err(_) => {} // doesn't exist, fine
    }
    std::os::unix::fs::symlink(src, target);
}
```

| | DXMT (macOS) | GPTK / D3DMetal (macOS) | DXVK + VKD3D (Linux) |
|---|---|---|---|
| Scope | D3D10, D3D11 | D3D11, D3D12 | D3D10/11 + D3D12 |
| Source | GitHub Releases | Apple Developer DMG | GitHub Releases |
| License | LGPL | Proprietary | LGPL |

### DXMT

Prebuilt binaries from `github.com/3Shain/dxmt/releases`. Each DXMT version may be tied to a specific Wine ABI, so installation is per-runtime.

**Install** — extracts into the global `graphics/` pool:
```
graphics/dxmt-1.x/
  lib/wine/x86_64-unix/winemetal.so
  lib/wine/x86_64-windows/winemetal.dll d3d11.dll dxgi.dll
```

When activating DXMT for a specific runtime, the `.so` files are symlinked from `graphics/dxmt-1.x/` into the runtime's `lib/`. DLLs are symlinked per-prefix into `drive_c/windows/system32/`. The symlink conflict logic (backup `.old` / replace symlink) handles pre-existing files.

**System Wine**: DXMT `.so` cannot be injected into system Wine (Wine uses its own loader). Only available if manually installed. Detection checks for `winemetal.so` in the system Wine `lib/`.

**Per-prefix** (always, files already in runtime's lib/):
```
prefix/drive_c/windows/system32/
  winemetal.dll d3d11.dll dxgi.dll
```

### GPTK / D3DMetal

User downloads `Game_porting_toolkit.dmg` from Apple Developer (requires an Apple ID, cannot be automated). The user selects the DMG via file chooser. Tequila extracts it:

```
# DMG extraction flow (macOS):
hdiutil attach Game_porting_toolkit.dmg -mountpoint /tmp/gptk_mount
cp -r /tmp/gptk_mount/lib/external/ graphics/d3dmetal-{version}/
hdiutil detach /tmp/gptk_mount
```

The extracted `graphics/d3dmetal-{version}/` contains `D3DMetal.framework/` and `libxremetal.so`. These are then symlinked into the runtime's `lib/` and prefix's `system32`.

### DXVK + VKD3D (Linux)

Prebuilt binaries from GitHub Releases, always installed together as a full D3D stack:

| Component | Source |
|---|---|
| DXVK | `github.com/doitsujin/dxvk/releases` |
| VKD3D-Proton | `github.com/HansKristian-Work/vkd3d-proton/releases` |

Extracted into the global `graphics/` pool as `dxvk-vkd3d-{version}/`, then symlinked into the runtime's `lib/` and each prefix's `system32`.

### Per-prefix Setting

Stored as part of `tequila-config.json` under a `graphics` key. Since the runtime is fixed per prefix, no runtime-switch degradation logic is needed.

```rust
struct GraphicsConfig {
    backend: String,  // "dxmt" | "d3dmetal" | "dxvk-vkd3d"
    version: String,  // upstream version string
}
```

Activation is done via DLL override registry keys in the prefix's `user.reg` (the existing registry editor already handles DLL overrides). The `.so` files are symlinked from `graphics/{backend}/` into the runtime's `lib/`, and `.dll` files into the prefix's `system32`.

## UI

### Runtime Manager (Settings Window)

A new `adw::PreferencesWindow` accessible from the header bar:

- **Runtime list** — all installed runtimes, radio-selected as global default
  - System Wine (auto-detected, version + path shown)
  - Each managed runtime (version + channel + install date)
  - Imported runtimes (custom name + path)
- **Add button** — dropdown: Download / Import
- **Download** — channel selector (Stable/Devel/Staging) → progress bar → auto-register
- **Import** — file chooser (pick a Wine `bin/` directory or `.app` bundle), auto-detect version → register

### Per-prefix Override

"Details" tab's wine version field becomes a read-only label showing the runtime id the prefix was created with.

## Implementation Order

1. **Data model + system detection** — `Runtime` struct, `RuntimeManager`, detect system Wine from PATH, wire PATH injection into `wine_processes.rs`
2. **Homebrew API client** — fetch cask JSON, parse `url` + `version` + `sha256`
3. **Download + extract** — async download, tar xf (Wine), pkg extract script (GStreamer)
4. **Import** — file chooser dialog, detect `bin/wine` and version, register as imported runtime
5. **Graphics backends** — DXMT fetch from GitHub Releases, GPTK extract from DMG, per-prefix DLL override activation
6. **Settings UI** — Runtime manager window with list, add/remove, download, import
7. **Per-prefix display** — read-only runtime label on Details tab

## Data Directory Tree

### macOS

```
$XDG_DATA_HOME/tequila/
  state.db
  icons/
    <sha256>.png
  runtimes/
    gstreamer/                       # shared, macOS only
      bin/  lib/  version.txt  env  setup_env.sh
    wine-stable/                         # managed, stable channel
      bin/  lib/
    wine-devel/                          # managed, devel channel
      bin/  lib/
    wine-imported-crossover/             # user-imported
      bin/  lib/
  graphics/
    dxmt-1.x/
      lib/...
```

### Linux

```
$XDG_DATA_HOME/tequila/
  state.db
  icons/
    <sha256>.png
  runtimes/
    wine-9.0/                         # versioned by tag
      bin/  lib/
    wine-10.0/
      bin/  lib/
  graphics/
    dxvk-vkd3d-1.x/                # DXVK + VKD3D bundled
```

```
$XDG_CONFIG_HOME/tequila/
  settings.json                     # RuntimeManager (runtimes list + default_id)
```
