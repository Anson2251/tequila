# Tequila 🥃

A modern, user‑friendly GUI application for managing Wine prefixes (also known as "Wine bottles")—built with Rust and [Relm4](https://relm4.org/) / libadwaita.

Tequila simplifies working with Wine on **macOS** and **Linux** by providing an intuitive interface to create, organize, and launch isolated Windows environments. It also includes a full registry editor, application scanner, and Wine runtime manager.

---

## Features

### 🍷 Prefix Management
- **Create & Delete** Wine prefixes (win32/win64)
- **Launch executables** from the UI with per‑executable environment variables and working directory
- **Run winecfg and regedit** directly from the interface
- **Process tracker** monitors running Wine processes and disables launch buttons while an app is active

### 🧩 Wine Runtime Manager
- **Detect system Wine** (`wine --version` in `PATH`)
- **Homebrew integration** (macOS): install and manage `wine-stable`, `wine-devel`, `wine-staging`
- **Kron4ek/Wine‑Builds**: browse and download specific Wine versions from GitHub Releases (works on Linux too)
- **Import existing Wine** installations from any directory
- **Switch Wine version** per prefix with automatic reinitialization
- **GStreamer runtime** download for macOS

### 🎨 Graphics Backends
- **DXMT** — DirectX Metal translation
- **D3DMetal** — DirectX 11/12 via Metal
- **DXVK + VKD3D** — DirectX via Vulkan
- Automatic DLL symlink installation and registry override setup per prefix
- Activation / deactivation from the prefix config panel

### 🔧 Registry Editor
Full graphical registry editor built into the application:

- **General settings**: Windows version, D3D renderer, offscreen rendering mode, audio/graphics drivers, font replacements, DLL overrides, virtual desktop, application‑specific settings
- **Graphics settings**: backend‑specific configuration
- **Platform settings**: macOS driver configuration, X11 settings
- **Registry caching** with TTL‑based invalidation
- **File watcher** for live registry updates

### 📁 Application Scanner
- **Auto‑scan** prefixes for Windows executables
- **Icon extraction** from PE files
- **Metadata extraction**: version info, company name, description, imported modules
- **Desktop file scanning** on Linux
- **SQLite cache** for scanned executables and registry data

### 🖥️ Modern UI
- Built with **Relm4 0.11** and **libadwaita**
- **Settings window** with NavigationView subpages
- **Error dialogs** for launch failures with actionable messages
- **macOS native** window controls, file dialogs, and menu integration

---

## Requirements

- **Rust** (latest stable, edition 2024)
- **GTK 4** (≥ 4.10)
- **libadwaita** (≥ 1.7)
- **zstd** — for compressed runtime downloads
- **tar**, **xz** — for archive extraction
- **Wine** — either installed system‑wide (`wine` in `PATH`) or managed via the runtime downloader

> **On macOS**, managed runtimes can be installed via Homebrew (`brew install --cask wine-stable`) or downloaded directly from Kron4ek/Wine‑Builds.
>
> **On Linux**, the Kron4ek downloader is the primary way to get managed Wine builds.

---

## Project Structure

Tequila is organized as a **Cargo workspace** with eight crates:

| Crate | Description |
|-------|-------------|
| [`base`](crates/base) | Core types: `PrefixConfig`, `RegisteredExecutable`, error types, `GraphicsBackend`, `GraphicsConfig`, and traits (`ConfigOperations`, `Scanner`, `PrefixManager`, `ExecutableManager`) |
| [`prefix`](crates/prefix) | Prefix lifecycle: create, delete, scan, launch executables, run winecfg/regedit, runtime environment setup, process tracking |
| [`runtime`](crates/runtime) | Wine runtime management: Homebrew cask integration, Kron4ek Wine‑Builds downloader, archive extraction, graphics backend installation (DXMT, D3DMetal, DXVK+VKD3D), GStreamer download |
| [`registry`](crates/registry) | Wine registry access via [`regashii`](https://crates.io/crates/regashii): `RegistryEditor`, `InMemoryRegistryCache`, `WineRegistry`, key constants, DLL override helpers |
| [`scan`](crates/scan) | Application scanner: PE icon extraction, metadata extraction, desktop file scanning |
| [`store`](crates/store) | Persistent storage: SQLite‑backed `PrefixStore` for registries and scanned executables, JSON `Settings` persistence |
| [`ui`](crates/ui) | GTK4/libadwaita UI: main window, prefix list/config, app manager, registry editor (general / graphics / platform tabs), runtime manager, settings window |
| [`tequila`](crates/tequila) | Binary entry point |

```
tequila/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── base/               # Core types and traits
│   ├── prefix/             # Prefix lifecycle & process management
│   ├── runtime/            # Wine runtime & graphics backend management
│   ├── registry/           # Wine registry editing
│   ├── scan/               # Application scanning & icon extraction
│   ├── store/              # Persistence (SQLite + JSON)
│   ├── ui/                 # GTK4/libadwaita UI
│   └── tequila/            # Binary entry point
├── data/                   # Icon resources
├── docs/                   # Design documents
├── scripts/                # Build helper scripts
└── winetricks.sh           # Bundled winetricks
```

---

## Installation

### From Source

```sh
git clone https://github.com/your-username/tequila.git
cd tequila
cargo build --release
./target/release/tequila
```

> ✅ macOS `.app` bundles and `.dmg` installers can now be built — see [macOS Packaging](#macos-packaging).

---

## Usage

1. **Launch Tequila** — the main window opens with an empty sidebar if no prefixes exist.
2. **Create a prefix** — use the "Create" dialog to set a name and architecture (win32/win64).
3. **Select a prefix** — the prefix config panel opens, showing Wine version, graphics backend, and registered applications.
4. **Install a Wine runtime** — go to **Settings → Wine Runtime** to detect system Wine, download a managed runtime (Homebrew or Kron4ek), or import an existing installation.
5. **Switch Wine versions** — in the prefix config, click "Switch" to select a different runtime. The prefix will automatically reinitialize.
6. **Launch applications** — executables detected by the scanner appear under the Apps list. You can also launch them directly from the prefix config.
7. **Edit the registry** — use the **Registry Editor** button to open the graphical registry editor with general, graphics, and platform tabs.

### Screenshots

*(Screenshots to be added)*

---

## Technical Highlights

- **Rust edition 2024** — full use of modern Rust features
- **Reactive UI** — Relm4's component model with `#[tracker::track]` for efficient updates
- **Async operations** — non‑blocking scanning, downloading, and registry operations via `tokio`
- **Wine runtime abstraction** — uniform API over system Wine, Homebrew casks, and Kron4ek builds
- **Graphics backend pipeline** — automatic DLL symlinking, registry override injection, and config serialization
- **SQLite persistence** — registry cache and scanned executable cache via `rusqlite`
- **Comprehensive error handling** — user‑friendly error dialogs with actionable suggestions (rate‑limiting, missing binaries, VPN issues)
- **macOS integration** — native menus, file dialogs (`NSSavePanel`), Dock integration, and `gdk4-macos`

---

## macOS Packaging

You can create a standalone macOS `.app` bundle (with GTK4/libadwaita dylibs included) using the provided packaging script.

### Prerequisites

```sh
brew install dylibbundler
```

Optionally, for `.dmg` creation, no extra tools are needed — macOS ships with `hdiutil`.

### Build the .app

```sh
# Create Tequila.app in dist/
./scripts/bundle-macos.sh

# Open it to test
open dist/Tequila.app
```

### Build .app + .dmg

```sh
./scripts/bundle-macos.sh --dmg
```

### Code-signing

```sh
# Requires an Apple Developer ID certificate
./scripts/bundle-macos.sh --dmg --sign
```

> **Note**: Without code-signing, macOS will show a security dialog.

### How it works

The script does the following:

1. `cargo build --release`
2. Creates the `.app` directory structure with `Info.plist`
3. Uses [`dylibbundler`](https://github.com/auriamg/macdylibbundler) to recursively bundle all GTK4/libadwaita dylib dependencies into `Contents/Frameworks/`
4. Copies GTK runtime resources (GdkPixbuf loaders, GLib schemas, Adwaita icons)
5. Creates a shell launcher that sets the correct `DYLD_LIBRARY_PATH`, `XDG_DATA_DIRS`, and GTK environment variables
6. Regenerates the GdkPixbuf loaders cache
7. Optionally code-signs and creates a `.dmg`

> For advanced users: [`cargo-packager`](https://docs.crabnebula.dev/packager) (from CrabNebula) is also supported via `Packager.toml`, but the bash script is the recommended way for GTK apps.

---

## Requirements Details

### System Dependencies

- **GTK 4** (≥ 4.10)
- **libadwaita** (≥ 1.7)
- **pkg-config**
- **zstd**
- **tar**, **xz**

**Debian/Ubuntu:**
```sh
sudo apt install libgtk-4-dev libadwaita-1-dev pkg-config zstd
```

**Fedora:**
```sh
sudo dnf install gtk4-devel libadwaita-devel pkg-config zstd
```

**Arch Linux:**
```sh
sudo pacman -S gtk4 libadwaita pkg-config zstd
```

**macOS (Homebrew):**
```sh
brew install gtk4 libadwaita pkg-config zstd
```

---

## Roadmap

### Short Term
- [ ] Binary releases (AppImage, macOS .app bundle)
- [ ] Windows executable drag‑and‑drop registration
- [ ] Prefix export/import as compressed archives
- [ ] Unit and integration test coverage

### Medium Term
- [ ] Winetricks integration (GUI for installing libraries)
- [ ] Prefix templates / starter configurations
- [ ] Application desktop shortcut creation
- [ ] Online application database

### Long Term
- [ ] Plugin system
- [ ] Cloud prefix sharing
- [ ] Performance profiling and Wine debug log viewer

---

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

Make sure your code follows standard Rust conventions and includes appropriate documentation. If you're adding a feature, consider updating the relevant design document in `docs/`.

---

## License

This project is licensed under the **GPLv3 License** — see the [LICENSE](LICENSE) file for details.

---

> **Tequila** — because managing Wine shouldn't give you a headache. 🥃
