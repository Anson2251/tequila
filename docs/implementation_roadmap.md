# Wine Prefix Configuration Implementation Roadmap

## Project Overview

This document provides a comprehensive roadmap for implementing the wine prefix configuration system for Tequila. The design phase has been completed, with detailed specifications for the data structures, UI components, and integration points.

## Completed Design Work

### 1. Data Structure Design ✅
- Defined `PrefixConfig` and `RegisteredExecutable` structures
- Specified JSON schema matching user requirements
- Included serialization/deserialization support
- Added methods for config manipulation

### 2. Module Architecture ✅
- Designed modular structure with separate concerns
- Created `prefix/` module with config, manager, and scanner submodules
- Planned UI components in `ui/` module
- Defined clear interfaces between components

### 3. Application Scanning ✅
- Designed automatic detection of installed applications
- Planned icon extraction and metadata gathering
- Specified common Windows application directories to scan
- Created workflow for user selection of detected apps

### 4. UI Design ✅
- Enhanced prefix list with configuration information
- Detailed prefix information panel
- Application management interface
- Dialogs for editing and adding applications

## Implementation Phases

### Phase 1: Core Infrastructure
**Estimated Time: 2-3 days**

1. **Update Dependencies**
   - Add serde, serde_json, chrono, walkdir to Cargo.toml
   - Update version numbers if needed

2. **Create Module Structure**
   - Create `src/prefix/` directory
   - Implement `mod.rs`, `config.rs`, `manager.rs`, `scanner.rs`
   - Create `src/ui/` directory
   - Implement `mod.rs`, `prefix_details.rs`, `app_manager.rs`

3. **Implement Core Data Structures**
   - Implement `PrefixConfig` with serialization
   - Implement `RegisteredExecutable` structure
   - Add validation methods and error handling

### Phase 2: Configuration Management
**Estimated Time: 2-3 days**

1. **Config I/O Operations**
   - Implement file reading/writing for tequila-config.json
   - Add error handling for corrupted configs
   - Create config migration system for future versions

2. **Prefix Manager Implementation**
   - Implement prefix detection with config loading
   - Add config generation for existing prefixes
   - Create CRUD operations for prefix configs

3. **Application Scanner**
   - Implement directory scanning logic
   - Add executable detection
   - Create icon extraction functionality

### Phase 3: UI Integration
**Estimated Time: 3-4 days**

1. **Update Main Application**
   - Integrate new prefix manager into main.rs
   - Update `WinePrefix` structure to include config
   - Modify message handling for new operations

2. **Implement UI Components**
   - Create enhanced prefix list items
   - Implement prefix details panel
   - Add application management dialogs
   - Create edit prefix details dialog

3. **Style and Polish**
   - Add CSS styling for new components
   - Implement responsive design
   - Add accessibility features

### Phase 4: Testing and Refinement
**Estimated Time: 2-3 days**

1. **Unit Testing**
   - Test config serialization/deserialization
   - Test scanner functionality
   - Test manager operations

2. **Integration Testing**
   - Test complete workflow
   - Test with existing prefixes
   - Test error conditions

3. **User Testing**
   - Test with various wine prefixes
   - Verify application detection
   - Check UI usability

## Key Implementation Files

### New Files to Create
```
src/
├── prefix/
│   ├── mod.rs             # Module exports
│   ├── config.rs          # PrefixConfig implementation
│   ├── manager.rs         # PrefixManager implementation
│   └── scanner.rs         # ApplicationScanner implementation
├── ui/
│   ├── mod.rs             # UI module exports
│   ├── prefix_details.rs   # Prefix details component
│   └── app_manager.rs     # Application management UI
└── tests/
    ├── prefix_config_tests.rs
    └── ui_tests.rs
```

### Files to Modify
```
Cargo.toml                 # Add dependencies
src/main.rs               # Integrate new modules
```

## Integration Points

### 1. Existing Code Integration
- Replace simple `WinePrefix` with enhanced version
- Update `scan_wine_prefixes` to use `PrefixManager`
- Modify message handling for new operations

### 2. Data Flow
- Prefix detection → Config loading → UI display
- User actions → Config updates → File saving
- Application scanning → User selection → Config updates

### 3. Error Handling
- File I/O errors for config operations
- JSON parsing errors
- Wine command execution errors
- UI error states and user feedback

## Migration Strategy

### For Existing Installations
1. Detect prefixes without configs on startup
2. Generate basic configs with available metadata
3. Save configs to prefix directories
4. Optionally notify users about new features

### Config Versioning
1. Include version field in config
2. Implement migration functions for future changes
3. Backward compatibility for older configs

## Testing Strategy

### 1. Unit Tests
- Config serialization/deserialization
- Scanner functionality
- Manager operations

### 2. Integration Tests
- Complete prefix management workflow
- UI interaction testing
- Error condition handling

### 3. Manual Testing
- Test with real wine prefixes
- Verify application detection
- Check UI responsiveness

## Success Criteria

1. ✅ All wine prefixes have associated JSON configs
2. ✅ Users can edit prefix metadata through UI
3. ✅ Applications can be detected and registered
4. ✅ Registered applications can be launched directly
5. ✅ Existing prefixes are migrated seamlessly
6. ✅ Error handling is robust and user-friendly
7. ✅ UI is intuitive and responsive

## Future Enhancements

### Short Term
- Import/export of prefix configurations
- Template system for common prefix types
- Enhanced application metadata extraction

### Long Term
- Online application database integration
- Automatic icon caching
- Prefix sharing and collaboration features

## Implementation Notes

1. **Performance**: Consider lazy loading of configs for large prefix collections
2. **Memory**: Implement efficient caching for scanned applications
3. **Threading**: Use async operations for scanning to prevent UI blocking
4. **Compatibility**: Ensure compatibility with different wine versions and configurations

## Conclusion

The wine prefix configuration system will significantly enhance Tequila's functionality by providing structured metadata management and improved application handling. The modular design ensures maintainability and extensibility for future features.

The implementation roadmap provides a clear path from design to deployment, with estimated timelines and success criteria to track progress.