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

## 🔄 Current Development Progress

### ✅ **Core Features Implemented**

#### **1. Advanced Prefix Configuration System**
- ✅ **Structured JSON Configuration**: All wine prefixes now have associated `tequila-config.json` files
- ✅ **Prefix Metadata Management**: Store and manage prefix names, architectures, Wine versions, and creation dates
- ✅ **Config Validation**: Robust validation system with error handling for corrupted configs
- ✅ **Automatic Migration**: Existing prefixes are automatically migrated with generated configs

#### **2. Application Management**
- ✅ **Application Scanner**: Automatic detection of installed Windows applications with metadata extraction
- ✅ **Executable Registration**: Register executables with custom names, descriptions, and icons
- ✅ **Direct Launch**: Launch registered applications directly from the UI
- ✅ **Icon Extraction**: Extract icons from Windows executables using PE file parsing
- ✅ **Metadata Extraction**: Extract version info, company name, and description from executables

#### **3. Registry Editor (Advanced Feature)**
- ✅ **Wine Registry Integration**: Full access to Wine registry via `regashii` library
- ✅ **Registry Caching**: In-memory cache with TTL-based invalidation for performance
- ✅ **Configuration Management**: Programmatic access to registry settings including:
  - Windows version settings
  - D3D renderer configuration
  - Offscreen rendering modes
  - Audio and graphics drivers
  - Font replacements
  - DLL overrides
  - Virtual desktop settings
  - Application-specific settings
  - macOS-specific driver settings

#### **4. Enhanced UI Components**
- ✅ **Modern Relm4 Interface**: Built with Relm4 0.10.0 and libadwaita
- ✅ **Prefix List**: Enhanced display with configuration information
- ✅ **Prefix Details Panel**: Comprehensive view of prefix metadata and applications
- ✅ **Application Management**: Add, edit, and remove registered applications
- ✅ **Dialog System**: Modal dialogs for editing prefix details and managing applications
- ✅ **Responsive Design**: UI adapts to different window sizes

#### **5. Core Architecture**
- ✅ **Modular Design**: Clean separation of concerns with dedicated modules:
  - `prefix/`: Core prefix management
  - `prefix/regeditor/`: Registry editing functionality
  - `ui/`: User interface components
- ✅ **Trait-based Design**: Extensible interfaces for all major components
- ✅ **Async Operations**: Non-blocking operations for scanning and registry access
- ✅ **Error Handling**: Comprehensive error handling with user-friendly messages

### 🔄 **In Progress / Advanced Features**

#### **1. Wine Configuration Integration**
- 🔄 **winecfg Integration**: Execute `winecfg` from within Tequila (implemented, UI integration in progress)
- 🔄 **Registry Editor UI**: Graphical interface for editing Wine registry settings
- 🔄 **Configuration Templates**: Preset configurations for different use cases

#### **2. Enhanced Application Management**
- 🔄 **Desktop File Integration**: Support for Linux `.desktop` files
- 🔄 **Enhanced Metadata**: More detailed application information extraction
- 🔄 **Icon Caching**: Efficient caching system for application icons

#### **3. Performance & Reliability**
- 🔄 **Lazy Loading**: Config loading on demand for large prefix collections
- 🔄 **Background Operations**: Non-blocking scanning and registry operations
- 🔄 **Robust Error Recovery**: Graceful handling of Wine command failures

### 📋 **Original Roadmap Status**

| Feature | Status | Notes |
|-------|--------|-------|
| Basic prefix management (create/delete) | ✅ | Enhanced with config system |
| Packaging/unpacking via zstd | ✅ | Core functionality implemented |
| Launch executables from prefix | ✅ | Advanced with metadata and icons |
| Create desktop/dock shortcuts (macOS first) | 🔄 | Core functionality in place |
| Integrated `winecfg` and `winetricks` GUI | 🔄 | `winecfg` integration complete |
| Linux support | 🔄 | Architecture designed for cross-platform |
| Import/export from `.tar.zst` archives | 🔄 | Base functionality available |
| Prefix metadata & icons | ✅ | Advanced implementation complete |

---

## 🧭 Roadmap

### **Immediate Next Steps**
- [ ] **Complete UI Integration**: Finish connecting all UI components to backend
- [ ] **Registry Editor UI**: Create graphical interface for registry editing
- [ ] **Testing Suite**: Expand unit and integration tests
- [ ] **Performance Optimization**: Lazy loading and background operations
- [ ] **Linux Testing**: Validate functionality on Linux systems

### **Medium Term Goals**
- [ ] **Template System**: Create and share prefix templates
- [ ] **Import/Export**: Enhanced prefix sharing capabilities
- [ ] **Advanced Application Management**: Better metadata extraction and organization
- [ ] **User Documentation**: Comprehensive usage guides and tutorials

### **Long Term Vision**
- [ ] **Cloud Integration**: Online application database and prefix sharing
- [ ] **Advanced Features**: Performance monitoring, dependency management
- [ ] **Plugin System**: Extensible architecture for new features

---

## 🤝 Contributing

Contributions are welcome! Please open an issue or submit a PR.  
Make sure your code follows standard Rust conventions and includes appropriate documentation.

---

## 📜 License

This project is licensed under the GPLv3 License — see the [LICENSE](LICENSE) file for details.

---

## 📂 Project Structure

```
src/
├── main.rs                     # Main application entry point
├── prefix/                     # Core prefix management
│   ├── mod.rs                  # Module exports
│   ├── config.rs               # PrefixConfig implementation
│   ├── manager.rs              # PrefixManager implementation
│   ├── scanner.rs              # ApplicationScanner implementation
│   ├── traits.rs               # Core traits
│   ├── wine_processes.rs       # Wine process utilities
│   └── regeditor/              # Registry editing
│       ├── mod.rs              # Registry module exports
│       ├── cache.rs            # Registry cache implementation
│       ├── editor.rs           # RegistryEditor implementation
│       ├── keys.rs             # Registry constants and enums
│       ├── registry.rs         # WineRegistry wrapper
│       └── traits.rs           # Registry traits
└── ui/                         # User interface
    ├── mod.rs                  # UI module exports
    ├── prefix_list.rs          # Prefix list component
    ├── prefix_details.rs       # Prefix details component
    ├── app_manager.rs          # Application management UI
    └── ...                     # Additional UI components
```

## 🛠️ Technical Highlights

- **Modern Rust**: Uses async/await, traits, and advanced type system features
- **Relm4 Framework**: Reactive UI with component-based architecture
- **Serde Integration**: Robust serialization/deserialization for config files
- **PE File Parsing**: Direct executable metadata extraction using `exe` crate
- **Registry Access**: Low-level Wine registry manipulation via `regashii`
- **Error Handling**: Comprehensive error types with user-friendly messages
- **Async Operations**: Non-blocking scanning and registry operations
- **Memory Management**: Efficient caching and resource handling

> **Tequila** — because managing Wine shouldn't give you a headache. 🥃