# Wine Prefix Configuration Module Design

## Overview

This document outlines the design for a wine prefix management module that uses JSON configuration files to store metadata about each wine prefix. The module will integrate with the existing Tequila application to provide enhanced prefix management capabilities.

## JSON Configuration Schema

The JSON configuration file will be named `tequila-config.json` and stored inside each wine prefix directory. The schema follows this structure:

```json
{
  "version": "1.0.0",
  "name": "prefix_name",
  "creation_date": "2025-10-24T08:00:00Z",
  "last_modified": "2025-10-24T08:30:00Z",
  "wine_version": "wine-10.0",
  "architecture": "win64",
  "description": "Gaming prefix with Steam installed",
  "registered_executables": [
    {
      "name": "Steam",
      "description": "Steam Game Client",
      "icon_path": "drive_c/Program Files (x86)/Steam/steam.icns",
      "executable_path": "drive_c/Program Files (x86)/Steam/steam.exe"
    },
    {
      "name": "Notepad++",
      "description": "Text Editor",
      "icon_path": "drive_c/Program Files/Notepad++/notepad++.icns",
      "executable_path": "drive_c/Program Files/Notepad++/notepad++.exe"
    }
  ]
}
```

## Module Architecture

### Core Components

1. **PrefixConfig Data Structure**
   - Rust structs mirroring the JSON schema
   - Serialization/deserialization support using serde
   - Validation methods for config integrity

2. **Prefix Configuration Manager**
   - CRUD operations for prefix configs
   - Config file I/O operations
   - Integration with existing prefix detection

3. **Application Scanner**
   - Detect installed applications in wine prefixes
   - Extract application metadata (name, icon, executable)
   - Update registered executables in config

4. **UI Integration**
   - Display prefix details from config
   - Edit prefix metadata
   - Manage registered executables
   - "Scan for Applications" functionality

### File Structure

```
src/
├── main.rs                 # Existing main application
├── prefix/
│   ├── mod.rs             # Module definition
│   ├── config.rs          # PrefixConfig data structure and serialization
│   ├── manager.rs         # Prefix configuration manager
│   └── scanner.rs         # Application scanner
└── ui/
    ├── prefix_details.rs  # UI for displaying prefix details
    └── app_manager.rs     # UI for managing registered executables
```

## Implementation Details

### 1. PrefixConfig Data Structure

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixConfig {
    pub version: String,
    pub name: String,
    pub creation_date: DateTime<Utc>,
    pub last_modified: DateTime<Utc>,
    pub wine_version: String,
    pub architecture: String,
    pub description: Option<String>,
    pub registered_executables: Vec<RegisteredExecutable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredExecutable {
    pub name: String,
    pub description: Option<String>,
    pub icon_path: Option<PathBuf>,
    pub executable_path: PathBuf,
}
```

### 2. Prefix Configuration Manager

The manager will handle:
- Creating new prefix configs
- Reading existing configs
- Updating configs with changes
- Validating config integrity
- Migrating configs between versions

### 3. Application Scanner

The scanner will:
- Search common Windows application directories
- Parse desktop files and shortcuts
- Extract icons and metadata
- Identify executable files
- Update the registered_executables list

### 4. Integration with Existing Code

The existing `WinePrefix` struct will be enhanced to include the configuration:

```rust
#[derive(Debug, Clone)]
struct WinePrefix {
    name: String,
    path: PathBuf,
    config: Option<PrefixConfig>, // New field
}
```

## Workflow

### For New Prefixes:
1. User creates a new prefix through the UI
2. System creates the wine prefix directory structure
3. A new PrefixConfig is generated with default values
4. Config is saved as `tequila-config.json` in the prefix directory
5. User can edit the configuration through the UI

### For Existing Prefixes:
1. During prefix scanning, check for existing `tequila-config.json`
2. If not found, generate a default config based on prefix metadata
3. Save the generated config to the prefix directory
4. Load the config into the application

### Application Registration:
1. User clicks "Scan for Applications"
2. Scanner searches the prefix for installed applications
3. Found applications are presented to the user for selection
4. Selected applications are added to registered_executables
5. Config is updated and saved

## Error Handling

The module will handle various error conditions:
- Missing or corrupted config files
- Invalid JSON structure
- File system permission issues
- Incompatible config versions

## Dependencies

Additional dependencies will be needed:
- `serde` and `serde_json` for serialization
- `chrono` for date/time handling
- `walkdir` for directory traversal during scanning

## UI Enhancements

The UI will be enhanced with:
- Prefix details panel showing configuration
- Editable fields for prefix metadata
- List of registered executables with icons
- "Scan for Applications" button
- Add/remove executable functionality
- Launch executable directly from the list

## Migration Path

For existing installations without configs:
1. Detect existing prefixes during startup
2. Generate basic configs with available metadata
3. Save configs to prefix directories
4. Notify users about the new configuration system

## Future Enhancements

Potential future features:
- Config versioning and migration
- Import/export of prefix configurations
- Template system for common prefix types
- Integration with online application databases
- Automatic icon extraction and caching