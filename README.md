# Tequila

A modern, user-friendly GUI application for managing Wine prefixes (also known as "Wine disks")—built with Rust and [Relm4](https://relm4.org/).

Tequila simplifies working with Wine on macOS and Linux by providing an intuitive interface to create, organize, share, and launch isolated Windows environments.

> **Note**: Tequila is currently in early development. macOS support is the initial target, with Linux support to follow.

---

## 🚀 Features

- **Manage Wine Prefixes**: Create, rename, and delete isolated Wine environments (prefixes).
- **Share Prefixes Easily**: Package the entire `drive_c` and registry (`system.reg`, `user.reg`, etc.) into a compressed `.zst` archive using [zstd](https://facebook.github.io/zstd/).
- **Launch Applications**: Open a file browser or run executables directly from your chosen Wine prefix.
- **Desktop & Dock Integration**: Create convenient desktop or dock shortcuts for quick access to your Wine apps.
- **(Planned)** Built-in `winecfg` and `winetricks` integration via GUI.

---

## 🛠️ Requirements

- **Rust** (latest stable)
- **Wine** (installed and in your `PATH`)
- **zstd** compression library (for packaging/unpacking prefixes)
- **GTK 4** (required by Relm4)

> On macOS, you may need to install Wine via [Homebrew](https://brew.sh/) (`brew install --cask wine-stable`) or another method that provides a working `wine` command.

---

## 📦 Installation

### From Source

1. Clone the repository:
   ```sh
   git clone https://github.com/your-username/tequila.git
   cd tequila
   ```

2. Build the project:
   ```sh
   cargo build --release
   ```

3. Run Tequila:
   ```sh
   ./target/release/tequila
   ```

> Binaries and installers will be provided in future releases.

---

## 🧭 Roadmap

- [x] Basic prefix management (create/delete)
- [x] Packaging/unpacking via zstd
- [ ] Launch executables from prefix
- [ ] Create desktop/dock shortcuts (macOS first)
- [ ] Integrated `winecfg` and `winetricks` GUI
- [ ] Linux support
- [ ] Import/export from `.tar.zst` archives
- [ ] Prefix metadata & icons

---

## 🤝 Contributing

Contributions are welcome! Please open an issue or submit a PR.  
Make sure your code follows standard Rust conventions and includes appropriate documentation.

---

## 📜 License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

---

> **Tequila** — because managing Wine shouldn’t give you a headache. 🥃