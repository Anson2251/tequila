//! Traits for registry editing operations
//! 
//! This module defines the core traits for registry editing operations,
//! providing async support for handling large registry files efficiently.

use crate::prefix::error::{Result, PrefixError};
use crate::prefix::regeditor::keys::*;
use async_trait::async_trait;
use std::path::PathBuf;

/// Trait for registry editing operations
/// 
/// This trait provides methods for reading, writing, and modifying Wine registry entries.
/// All methods are async to handle large registry files without blocking the UI.
#[async_trait]
#[allow(dead_code)]
pub trait RegEditor: Send + Sync {
    /// Load registry from a Wine prefix
    /// 
    /// # Arguments
    /// * `prefix_path` - Path to the Wine prefix
    /// 
    /// # Returns
    /// `Result<()>` - Success or error
    async fn load_registry(&mut self, prefix_path: &PathBuf) -> Result<()>;

    /// Save registry to a Wine prefix
    /// 
    /// # Arguments
    /// * `prefix_path` - Path to the Wine prefix
    /// 
    /// # Returns
    /// `Result<()>` - Success or error
    async fn save_registry(&self, prefix_path: &PathBuf) -> Result<()>;

    /// Get a Windows version setting
    async fn get_windows_version(&self) -> Result<Option<String>>;

    /// Set a Windows version setting
    async fn set_windows_version(&mut self, version: &str) -> Result<()>;

    /// Get Direct3D renderer setting
    async fn get_d3d_renderer(&self) -> Result<Option<String>>;

    /// Set Direct3D renderer setting
    async fn set_d3d_renderer(&mut self, renderer: &str) -> Result<()>;

    /// Get Direct3D CSMT setting
    async fn get_d3d_csmt(&self) -> Result<Option<u32>>;

    /// Set Direct3D CSMT setting
    async fn set_d3d_csmt(&mut self, enabled: bool) -> Result<()>;

    /// Get offscreen rendering mode
    async fn get_offscreen_rendering_mode(&self) -> Result<Option<String>>;

    /// Set offscreen rendering mode
    async fn set_offscreen_rendering_mode(&mut self, mode: &str) -> Result<()>;

    /// Get mouse warp override setting
    async fn get_mouse_warp_override(&self) -> Result<Option<String>>;

    /// Set mouse warp override setting
    async fn set_mouse_warp_override(&mut self, mode: &str) -> Result<()>;

    /// Get audio driver setting
    async fn get_audio_driver(&self) -> Result<Option<String>>;

    /// Set audio driver setting
    async fn set_audio_driver(&mut self, driver: &str) -> Result<()>;

    /// Get graphics driver setting
    async fn get_graphics_driver(&self) -> Result<Option<String>>;

    /// Set graphics driver setting
    async fn set_graphics_driver(&mut self, driver: &str) -> Result<()>;

    /// Get desktop settings
    async fn get_desktop_settings(&self) -> Result<Option<DesktopSettings>>;

    /// Set desktop settings
    async fn set_desktop_settings(&mut self, settings: &DesktopSettings) -> Result<()>;

    /// Get font replacement settings
    async fn get_font_replacements(&self) -> Result<Vec<FontReplacement>>;

    /// Add a font replacement
    async fn add_font_replacement(&mut self, original: &str, replacement: &str) -> Result<()>;

    /// Remove a font replacement
    async fn remove_font_replacement(&mut self, original: &str) -> Result<()>;

    /// Get DLL overrides
    async fn get_dll_overrides(&self) -> Result<Vec<DllOverride>>;

    /// Add a DLL override
    async fn add_dll_override(&mut self, dll: &str, setting: DllOverrideSetting) -> Result<()>;

    /// Remove a DLL override
    async fn remove_dll_override(&mut self, dll: &str) -> Result<()>;

    /// Get video memory size setting
    async fn get_video_memory_size(&self) -> Result<Option<u32>>;

    /// Set video memory size setting
    async fn set_video_memory_size(&mut self, size_mb: u32) -> Result<()>;

    /// Get shader model settings
    async fn get_shader_model_settings(&self) -> Result<Option<ShaderModelSettings>>;

    /// Set shader model settings
    async fn set_shader_model_settings(&mut self, settings: &ShaderModelSettings) -> Result<()>;

    /// Get virtual desktop settings
    async fn get_virtual_desktop(&self) -> Result<Option<VirtualDesktopSettings>>;

    /// Set virtual desktop settings
    async fn set_virtual_desktop(&mut self, settings: &VirtualDesktopSettings) -> Result<()>;

    /// Get application-specific settings
    async fn get_app_settings(&self, app_name: &str) -> Result<Option<AppSettings>>;

    /// Set application-specific settings
    async fn set_app_settings(&mut self, app_name: &str, settings: &AppSettings) -> Result<()>;

    /// Remove application-specific settings
    async fn remove_app_settings(&mut self, app_name: &str) -> Result<()>;

    /// Get X11 Driver settings
    async fn get_x11_driver_settings(&self) -> Result<Option<X11DriverSettings>>;

    /// Set X11 Driver settings
    async fn set_x11_driver_settings(&mut self, settings: &X11DriverSettings) -> Result<()>;

    /// Get DPI settings
    async fn get_dpi_settings(&self) -> Result<Option<DpiSettings>>;

    /// Set DPI settings
    async fn set_dpi_settings(&mut self, settings: &DpiSettings) -> Result<()>;

    /// Get Mac Driver settings
    async fn get_mac_driver_settings(&self) -> Result<Option<MacDriverSettings>>;

    /// Set Mac Driver settings
    async fn set_mac_driver_settings(&mut self, settings: &MacDriverSettings) -> Result<()>;

    /// Validate registry entries
    async fn validate_registry(&self) -> Result<Vec<ValidationError>>;

    /// Get all registry keys for debugging
    async fn get_all_keys(&self) -> Result<Vec<String>>;
}

/// Trait for registry caching operations
#[async_trait]
pub trait RegistryCache: Send + Sync {
    /// Get cached registry for a prefix
    async fn get_cached_registry(&self, prefix_path: &PathBuf) -> Result<Option<crate::prefix::regeditor::registry::WineRegistry>>;

    /// Cache registry for a prefix
    async fn cache_registry(&self, prefix_path: &PathBuf, registry: crate::prefix::regeditor::registry::WineRegistry) -> Result<()>;

    /// Invalidate cache for a prefix
    async fn invalidate_cache(&self, prefix_path: &PathBuf) -> Result<()>;

    /// Clear all cached registries
    async fn clear_all_cache(&self) -> Result<()>;
}