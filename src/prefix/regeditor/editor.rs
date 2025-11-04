//! Registry editor implementation
//! 
//! This module provides the main implementation of the RegEditor trait,
//! handling all Wine registry operations with validation and caching.

use crate::prefix::error::{Result, PrefixError};
use crate::prefix::regeditor::traits::{RegEditor, RegistryCache};
use crate::prefix::regeditor::keys::*;
use crate::prefix::regeditor::registry::WineRegistry;
use async_trait::async_trait;
use regashii::{Value, ValueName};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Main registry editor implementation
///
/// This struct provides a high-level interface for editing Wine registry files,
/// with built-in validation and caching support.
pub struct RegistryEditor {
    /// The underlying registry
    registry: WineRegistry,
    /// Cache for registry operations
    cache: Arc<dyn RegistryCache>,
    /// Path to the current prefix
    prefix_path: Option<PathBuf>,
}

impl std::fmt::Debug for RegistryEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryEditor")
            .field("registry", &"WineRegistry")
            .field("cache", &"RegistryCache")
            .field("prefix_path", &self.prefix_path)
            .finish()
    }
}

impl RegistryEditor {
    /// Create a new registry editor
    /// 
    /// # Arguments
    /// * `cache` - Cache implementation for registry operations
    /// 
    /// # Returns
    /// A new registry editor instance
    pub fn new(cache: Arc<dyn RegistryCache>) -> Self {
        Self {
            registry: WineRegistry::new(),
            cache,
            prefix_path: None,
        }
    }
    
    
    /// Create a registry editor with a specific cache
    ///
    /// # Arguments
    /// * `cache` - Cache implementation
    /// * `prefix_path` - Path to Wine prefix
    ///
    /// # Returns
    /// Result with registry editor or error
    pub async fn with_prefix(
        cache: Arc<dyn RegistryCache>,
        prefix_path: &PathBuf,
    ) -> Result<Self> {
        // Try to get from cache first
        if let Some(cached_registry) = cache.get_cached_registry(prefix_path).await? {
            return Ok(Self {
                registry: cached_registry,
                cache,
                prefix_path: Some(prefix_path.clone()),
            });
        }

        // Load from prefix (loads system.reg, user.reg, and userdef.reg)
        let registry = WineRegistry::load_from_prefix(prefix_path).await?;

        // Cache the loaded registry
        cache.cache_registry(prefix_path, registry.clone()).await?;

        Ok(Self {
            registry,
            cache,
            prefix_path: Some(prefix_path.clone()),
        })
    }

    /// Get the path to the user.reg file in a Wine prefix
    fn get_registry_path(prefix_path: &PathBuf) -> Result<PathBuf> {
        let reg_path = prefix_path
            .join("user.reg")
            .canonicalize()
            .map_err(|e| PrefixError::InvalidPath(format!("Invalid prefix path: {}", e)))?;
        
        Ok(reg_path)
    }

    /// Helper method to get a string value from registry
    async fn get_string_value(&self, key_path: &str, value_name: &str) -> Result<Option<String>> {
        println!("DEBUG: Getting string value - Key: {}, Value: {}", key_path, value_name);

        if let Some(value) = self.registry.get_value(key_path, value_name).await? {
            println!("DEBUG: Found value: {:?}", value);
            match value {
                Value::Sz(s) => Ok(Some(s)),
                Value::ExpandSz(s) => Ok(Some(s)),
                _ => {
                    println!("DEBUG: Value type mismatch, returning None");
                    Ok(None)
                },
            }
        } else {
            println!("DEBUG: No value found for key: {}, value: {}", key_path, value_name);
            Ok(None)
        }
    }

    /// Helper method to set a string value in registry
    async fn set_string_value(&mut self, key_path: &str, value_name: &str, value: &str) -> Result<()> {
        let reg_value = Value::Sz(value.to_string());
        self.registry.set_value(key_path, value_name, reg_value).await
    }

    /// Helper method to get a DWORD value from registry
    async fn get_dword_value(&self, key_path: &str, value_name: &str) -> Result<Option<u32>> {
        if let Some(value) = self.registry.get_value(key_path, value_name).await? {
            match value {
                Value::Dword(d) => Ok(Some(d)),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Helper method to set a DWORD value in registry
    async fn set_dword_value(&mut self, key_path: &str, value_name: &str, value: u32) -> Result<()> {
        let reg_value = Value::Dword(value);
        self.registry.set_value(key_path, value_name, reg_value).await
    }

    /// Validate a registry key path
    fn validate_key_path(key_path: &str) -> Result<()> {
        if key_path.is_empty() {
            return Err(PrefixError::ValidationError("Key path cannot be empty".to_string()));
        }

        // For our use case, we're using HKEY_CURRENT_USER keys without the prefix
        // So we just validate that it starts with "Software"
        if !key_path.starts_with("Software") {
            return Err(PrefixError::ValidationError(format!(
                "Invalid key path format: {}. Expected to start with 'Software'",
                key_path
            )));
        }

        Ok(())
    }

    /// Validate a value name
    fn validate_value_name(value_name: &str) -> Result<()> {
        if value_name.is_empty() {
            return Err(PrefixError::ValidationError("Value name cannot be empty".to_string()));
        }

        // Check for invalid characters
        let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
        for c in value_name.chars() {
            if invalid_chars.contains(&c) {
                return Err(PrefixError::ValidationError(format!(
                    "Invalid character '{}' in value name",
                    c
                )));
            }
        }

        Ok(())
    }
}

#[async_trait]
impl RegEditor for RegistryEditor {
    /// Load registry from a Wine prefix
    async fn load_registry(&mut self, prefix_path: &PathBuf) -> Result<()> {
        let registry_path = Self::get_registry_path(prefix_path)?;
        self.registry = WineRegistry::load_from_file(&registry_path).await?;
        self.prefix_path = Some(prefix_path.clone());
        
        // Update cache
        self.cache.cache_registry(prefix_path, self.registry.clone()).await?;
        
        Ok(())
    }

    /// Save registry to a Wine prefix
    async fn save_registry(&self, prefix_path: &PathBuf) -> Result<()> {
        let registry_path = Self::get_registry_path(prefix_path)?;
        self.registry.save_to_file(&registry_path).await?;
        
        // Invalidate cache since file was modified
        self.cache.invalidate_cache(prefix_path).await?;
        
        Ok(())
    }

    /// Get a Windows version setting
    async fn get_windows_version(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine", "Version").await
    }

    /// Set a Windows version setting
    async fn set_windows_version(&mut self, version: &str) -> Result<()> {
        let key_path = "Software\\Wine";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("Version")?;
        
        if let Some(parsed_version) = WindowsVersion::from_string(version) {
            self.set_string_value(key_path, "Version", parsed_version.to_string()).await
        } else {
            Err(PrefixError::ValidationError(format!("Invalid Windows version: {}", version)))
        }
    }

    /// Get Direct3D renderer setting
    async fn get_d3d_renderer(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\Direct3D", "renderer").await
    }

    /// Set Direct3D renderer setting
    async fn set_d3d_renderer(&mut self, renderer: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("renderer")?;
        
        if let Some(parsed_renderer) = D3DRenderer::from_string(renderer) {
            self.set_string_value(key_path, "renderer", parsed_renderer.to_string()).await
        } else {
            Err(PrefixError::ValidationError(format!("Invalid Direct3D renderer: {}", renderer)))
        }
    }

    /// Get Direct3D CSMT setting
    async fn get_d3d_csmt(&self) -> Result<Option<u32>> {
        self.get_dword_value("Software\\Wine\\Direct3D", "csmt").await
    }

    /// Set Direct3D CSMT setting
    async fn set_d3d_csmt(&mut self, enabled: bool) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("csmt")?;
        
        let value = if enabled { 1 } else { 0 };
        self.set_dword_value(key_path, "csmt", value).await
    }

    /// Get offscreen rendering mode
    async fn get_offscreen_rendering_mode(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\Direct3D", "OffscreenRenderingMode").await
    }

    /// Set offscreen rendering mode
    async fn set_offscreen_rendering_mode(&mut self, mode: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("OffscreenRenderingMode")?;
        
        if let Some(parsed_mode) = OffscreenRenderingMode::from_string(mode) {
            self.set_string_value(key_path, "OffscreenRenderingMode", parsed_mode.to_string()).await
        } else {
            Err(PrefixError::ValidationError(format!("Invalid offscreen rendering mode: {}", mode)))
        }
    }

    /// Get mouse warp override setting
    async fn get_mouse_warp_override(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\DirectInput", "MouseWarpOverride").await
    }

    /// Set mouse warp override setting
    async fn set_mouse_warp_override(&mut self, mode: &str) -> Result<()> {
        let key_path = "Software\\Wine\\DirectInput";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("MouseWarpOverride")?;
        
        if let Some(parsed_mode) = MouseWarpOverride::from_string(mode) {
            self.set_string_value(key_path, "MouseWarpOverride", parsed_mode.to_string()).await
        } else {
            Err(PrefixError::ValidationError(format!("Invalid mouse warp override: {}", mode)))
        }
    }

    /// Get audio driver setting
    async fn get_audio_driver(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\Drivers\\Audio", "").await
    }

    /// Set audio driver setting
    async fn set_audio_driver(&mut self, driver: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Drivers\\Audio";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("")?;
        
        if let Some(parsed_driver) = AudioDriver::from_string(driver) {
            self.set_string_value(key_path, "", parsed_driver.to_string()).await
        } else {
            Err(PrefixError::ValidationError(format!("Invalid audio driver: {}", driver)))
        }
    }

    /// Get graphics driver setting
    async fn get_graphics_driver(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\Drivers\\Graphics", "").await
    }

    /// Set graphics driver setting
    async fn set_graphics_driver(&mut self, driver: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Drivers\\Graphics";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("")?;
        
        if let Some(parsed_driver) = GraphicsDriver::from_string(driver) {
            self.set_string_value(key_path, "", parsed_driver.to_string()).await
        } else {
            Err(PrefixError::ValidationError(format!("Invalid graphics driver: {}", driver)))
        }
    }

    /// Get desktop settings
    async fn get_desktop_settings(&self) -> Result<Option<DesktopSettings>> {
        let key_path = "Software\\Wine\\Explorer";
        
        let desktop = self.get_string_value(key_path, "Desktop").await?;
        let show_systray = self.get_dword_value(key_path, "ShowSystray").await?.unwrap_or(1) != 0;
        
        // Get desktops
        let desktops_path = format!("{}\\Desktops", key_path);
        let mut desktops = HashMap::new();
        
        let values = self.registry.get_key_values(&desktops_path).await?;
        if !values.is_empty() {
            for (name, value) in values {
                if let Value::Sz(size_str) = value {
                    if let Some(size) = DesktopSize::from_string(&size_str) {
                        desktops.insert(name, size);
                    }
                }
            }
        }
        
        Ok(Some(DesktopSettings {
            desktop,
            desktops,
            show_systray,
        }))
    }

    /// Set desktop settings
    async fn set_desktop_settings(&mut self, settings: &DesktopSettings) -> Result<()> {
        let key_path = "Software\\Wine\\Explorer";
        Self::validate_key_path(key_path)?;
        
        // Set desktop
        if let Some(desktop) = &settings.desktop {
            self.set_string_value(key_path, "Desktop", desktop).await?;
        }
        
        // Set show systray
        let systray_value = if settings.show_systray { 1 } else { 0 };
        self.set_dword_value(key_path, "ShowSystray", systray_value).await?;
        
        // Set desktops
        let desktops_path = format!("{}\\Desktops", key_path);
        for (name, size) in &settings.desktops {
            Self::validate_value_name(name)?;
            self.set_string_value(&desktops_path, name, &size.to_string()).await?;
        }
        
        Ok(())
    }

    /// Get font replacement settings
    async fn get_font_replacements(&self) -> Result<Vec<FontReplacement>> {
        let key_path = "Software\\Wine\\Fonts\\Replacements";
        let mut replacements = Vec::new();
        
        let values = self.registry.get_key_values(key_path).await?;
        if !values.is_empty() {
            for (original, value) in values {
                if let Value::Sz(replacement) = value {
                    replacements.push(FontReplacement::new(original, replacement));
                }
            }
        }
        
        Ok(replacements)
    }

    /// Add a font replacement
    async fn add_font_replacement(&mut self, original: &str, replacement: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Fonts\\Replacements";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name(original)?;
        
        self.set_string_value(key_path, original, replacement).await
    }

    /// Remove a font replacement
    async fn remove_font_replacement(&mut self, original: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Fonts\\Replacements";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name(original)?;
        
        self.registry.delete_value(key_path, original).await
    }

    /// Get DLL overrides
    async fn get_dll_overrides(&self) -> Result<Vec<DllOverride>> {
        let key_path = "Software\\Wine\\DllOverrides";
        let mut overrides = Vec::new();
        
        let values = self.registry.get_key_values(key_path).await?;
        if !values.is_empty() {
            for (dll, value) in values {
                if let Value::Sz(setting_str) = value {
                    if let Some(setting) = DllOverrideSetting::from_string(&setting_str) {
                        overrides.push(DllOverride { dll, setting });
                    }
                }
            }
        }
        
        Ok(overrides)
    }

    /// Add a DLL override
    async fn add_dll_override(&mut self, dll: &str, setting: DllOverrideSetting) -> Result<()> {
        let key_path = "Software\\Wine\\DllOverrides";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name(dll)?;
        
        self.set_string_value(key_path, dll, setting.to_string()).await
    }

    /// Remove a DLL override
    async fn remove_dll_override(&mut self, dll: &str) -> Result<()> {
        let key_path = "Software\\Wine\\DllOverrides";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name(dll)?;
        
        self.registry.delete_value(key_path, dll).await
    }

    /// Get video memory size setting
    async fn get_video_memory_size(&self) -> Result<Option<u32>> {
        self.get_dword_value("Software\\Wine\\Direct3D", "VideoMemorySize").await
    }

    /// Set video memory size setting
    async fn set_video_memory_size(&mut self, size_mb: u32) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("VideoMemorySize")?;
        
        if size_mb == 0 || size_mb > 16384 {
            return Err(PrefixError::ValidationError(
                "Video memory size must be between 1 and 16384 MB".to_string(),
            ));
        }
        
        self.set_dword_value(key_path, "VideoMemorySize", size_mb).await
    }

    /// Get shader model settings
    async fn get_shader_model_settings(&self) -> Result<Option<ShaderModelSettings>> {
        let key_path = "Software\\Wine\\Direct3D";
        
        let max_shader_model_vs = self.get_dword_value(key_path, "MaxShaderModelVS").await?;
        let max_shader_model_ps = self.get_dword_value(key_path, "MaxShaderModelPS").await?;
        let max_shader_model_gs = self.get_dword_value(key_path, "MaxShaderModelGS").await?;
        let max_shader_model_hs = self.get_dword_value(key_path, "MaxShaderModelHS").await?;
        let max_shader_model_ds = self.get_dword_value(key_path, "MaxShaderModelDS").await?;
        let max_shader_model_cs = self.get_dword_value(key_path, "MaxShaderModelCS").await?;
        
        if max_shader_model_vs.is_none() && max_shader_model_ps.is_none() && max_shader_model_gs.is_none()
            && max_shader_model_hs.is_none() && max_shader_model_ds.is_none() && max_shader_model_cs.is_none() {
            return Ok(None);
        }
        
        Ok(Some(ShaderModelSettings {
            max_shader_model_vs,
            max_shader_model_ps,
            max_shader_model_gs,
            max_shader_model_hs,
            max_shader_model_ds,
            max_shader_model_cs,
        }))
    }

    /// Set shader model settings
    async fn set_shader_model_settings(&mut self, settings: &ShaderModelSettings) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        
        if let Some(vs) = settings.max_shader_model_vs {
            Self::validate_value_name("MaxShaderModelVS")?;
            self.set_dword_value(key_path, "MaxShaderModelVS", vs).await?;
        }
        
        if let Some(ps) = settings.max_shader_model_ps {
            Self::validate_value_name("MaxShaderModelPS")?;
            self.set_dword_value(key_path, "MaxShaderModelPS", ps).await?;
        }
        
        if let Some(gs) = settings.max_shader_model_gs {
            Self::validate_value_name("MaxShaderModelGS")?;
            self.set_dword_value(key_path, "MaxShaderModelGS", gs).await?;
        }
        
        if let Some(hs) = settings.max_shader_model_hs {
            Self::validate_value_name("MaxShaderModelHS")?;
            self.set_dword_value(key_path, "MaxShaderModelHS", hs).await?;
        }
        
        if let Some(ds) = settings.max_shader_model_ds {
            Self::validate_value_name("MaxShaderModelDS")?;
            self.set_dword_value(key_path, "MaxShaderModelDS", ds).await?;
        }
        
        if let Some(cs) = settings.max_shader_model_cs {
            Self::validate_value_name("MaxShaderModelCS")?;
            self.set_dword_value(key_path, "MaxShaderModelCS", cs).await?;
        }
        
        Ok(())
    }

    /// Get virtual desktop settings
    async fn get_virtual_desktop(&self) -> Result<Option<VirtualDesktopSettings>> {
        let desktop_settings = self.get_desktop_settings().await?;
        
        if let Some(settings) = desktop_settings {
            if let Some(desktop_name) = settings.desktop {
                if let Some(size) = settings.desktops.get(&desktop_name) {
                    return Ok(Some(VirtualDesktopSettings {
                        enabled: true,
                        width: size.width,
                        height: size.height,
                    }));
                }
            }
        }
        
        Ok(None)
    }

    /// Set virtual desktop settings
    async fn set_virtual_desktop(&mut self, settings: &VirtualDesktopSettings) -> Result<()> {
        if settings.enabled {
            let desktop_settings = DesktopSettings {
                desktop: Some("Default".to_string()),
                desktops: {
                    let mut desktops = HashMap::new();
                    desktops.insert("Default".to_string(), DesktopSize::new(settings.width, settings.height));
                    desktops
                },
                show_systray: false,
            };
            
            self.set_desktop_settings(&desktop_settings).await
        } else {
            // Disable virtual desktop
            let key_path = "Software\\Wine\\Explorer";
            Self::validate_key_path(key_path)?;
            
            self.registry.delete_value(key_path, "Desktop").await?;
            
            Ok(())
        }
    }

    /// Get application-specific settings
    async fn get_app_settings(&self, app_name: &str) -> Result<Option<AppSettings>> {
        let key_path = format!("Software\\Wine\\AppDefaults\\{}", app_name);
        
        if !self.registry.key_exists(&key_path).await? {
            return Ok(None);
        }
        
        let mut app_settings = AppSettings::new(app_name.to_string());
        
        // Get DLL overrides for this app
        let dll_overrides_path = format!("{}\\DllOverrides", key_path);
        let values = self.registry.get_key_values(&dll_overrides_path).await?;
        if !values.is_empty() {
            for (dll, value) in values {
                if let Value::Sz(setting_str) = value {
                    if let Some(setting) = DllOverrideSetting::from_string(&setting_str) {
                        app_settings.dll_overrides.push(DllOverride { dll, setting });
                    }
                }
            }
        }
        
        // Get Direct3D renderer
        if let Some(renderer_str) = self.get_string_value(&format!("{}\\Direct3D", key_path), "renderer").await? {
            if let Some(renderer) = D3DRenderer::from_string(&renderer_str) {
                app_settings.d3d_renderer = Some(renderer);
            }
        }
        
        // Get offscreen rendering mode
        if let Some(mode_str) = self.get_string_value(&format!("{}\\Direct3D", key_path), "OffscreenRenderingMode").await? {
            if let Some(mode) = OffscreenRenderingMode::from_string(&mode_str) {
                app_settings.offscreen_rendering_mode = Some(mode);
            }
        }
        
        Ok(Some(app_settings))
    }

    /// Set application-specific settings
    async fn set_app_settings(&mut self, app_name: &str, settings: &AppSettings) -> Result<()> {
        let base_key_path = format!("Software\\Wine\\AppDefaults\\{}", app_name);
        Self::validate_key_path(&base_key_path)?;
        
        // Set DLL overrides
        if !settings.dll_overrides.is_empty() {
            let dll_overrides_path = format!("{}\\DllOverrides", base_key_path);
            for dll_override in &settings.dll_overrides {
                Self::validate_value_name(&dll_override.dll)?;
                self.set_string_value(&dll_overrides_path, &dll_override.dll, dll_override.setting.to_string()).await?;
            }
        }
        
        // Set Direct3D renderer
        if let Some(renderer) = &settings.d3d_renderer {
            let d3d_path = format!("{}\\Direct3D", base_key_path);
            Self::validate_value_name("renderer")?;
            self.set_string_value(&d3d_path, "renderer", renderer.to_string()).await?;
        }
        
        // Set offscreen rendering mode
        if let Some(mode) = &settings.offscreen_rendering_mode {
            let d3d_path = format!("{}\\Direct3D", base_key_path);
            Self::validate_value_name("OffscreenRenderingMode")?;
            self.set_string_value(&d3d_path, "OffscreenRenderingMode", mode.to_string()).await?;
        }
        
        // Set custom settings
        for (key, value) in &settings.custom_settings {
            Self::validate_value_name(key)?;
            self.set_string_value(&base_key_path, key, value).await?;
        }
        
        Ok(())
    }

    /// Remove application-specific settings
    async fn remove_app_settings(&mut self, app_name: &str) -> Result<()> {
        let key_path = format!("Software\\Wine\\AppDefaults\\{}", app_name);
        Self::validate_key_path(&key_path)?;
        
        self.registry.delete_key(&key_path).await
    }

    /// Get X11 Driver settings
    async fn get_x11_driver_settings(&self) -> Result<Option<X11DriverSettings>> {
        let key_path = "Software\\Wine\\X11 Driver";

        let decorated = self.get_string_value(key_path, "Decorated").await?;
        let client_side_graphics = self.get_string_value(key_path, "ClientSideGraphics").await?;
        let client_side_with_render = self.get_string_value(key_path, "ClientSideWithRender").await?;
        let client_side_antialias_with_render = self.get_string_value(key_path, "ClientSideAntiAliasWithRender").await?;
        let client_side_antialias_with_core = self.get_string_value(key_path, "ClientSideAntiAliasWithCore").await?;
        let grab_fullscreen = self.get_string_value(key_path, "GrabFullscreen").await?;
        let grab_pointer = self.get_string_value(key_path, "GrabPointer").await?;
        let managed = self.get_string_value(key_path, "Managed").await?;
        let use_xrandr = self.get_string_value(key_path, "UseXRandR").await?;
        let use_xvid_mode = self.get_string_value(key_path, "UseXVidMode").await?;

        // If none of the settings exist, return None
        if decorated.is_none() && client_side_graphics.is_none() && client_side_with_render.is_none()
            && client_side_antialias_with_render.is_none() && client_side_antialias_with_core.is_none()
            && grab_fullscreen.is_none() && grab_pointer.is_none() && managed.is_none()
            && use_xrandr.is_none() && use_xvid_mode.is_none() {
            return Ok(None);
        }

        let mut settings = X11DriverSettings::new();

        if let Some(decorated_str) = decorated {
            settings.decorated = Some(decorated_str != "N");
        }

        if let Some(csg_str) = client_side_graphics {
            settings.client_side_graphics = Some(csg_str != "N");
        }

        if let Some(cswr_str) = client_side_with_render {
            settings.client_side_with_render = Some(cswr_str != "N");
        }

        if let Some(caar_str) = client_side_antialias_with_render {
            settings.client_side_antialias_with_render = Some(caar_str != "N");
        }

        if let Some(caac_str) = client_side_antialias_with_core {
            settings.client_side_antialias_with_core = Some(caac_str != "N");
        }

        if let Some(grab_fs_str) = grab_fullscreen {
            settings.grab_fullscreen = Some(grab_fs_str == "Y");
        }

        if let Some(grab_ptr_str) = grab_pointer {
            settings.grab_pointer = Some(grab_ptr_str != "N");
        }

        if let Some(managed_str) = managed {
            settings.managed = Some(managed_str != "N");
        }

        if let Some(xrandr_str) = use_xrandr {
            settings.use_xrandr = Some(xrandr_str != "N");
        }

        if let Some(xvid_str) = use_xvid_mode {
            settings.use_xvid_mode = Some(xvid_str == "Y");
        }

        Ok(Some(settings))
    }

    /// Set X11 Driver settings
    async fn set_x11_driver_settings(&mut self, settings: &X11DriverSettings) -> Result<()> {
        let key_path = "Software\\Wine\\X11 Driver";
        Self::validate_key_path(key_path)?;

        // Set Decorated
        if let Some(decorated) = settings.decorated {
            let decorated_str = if decorated { "Y" } else { "N" };
            Self::validate_value_name("Decorated")?;
            self.set_string_value(key_path, "Decorated", decorated_str).await?;
        }

        // Set ClientSideGraphics
        if let Some(csg) = settings.client_side_graphics {
            let csg_str = if csg { "Y" } else { "N" };
            Self::validate_value_name("ClientSideGraphics")?;
            self.set_string_value(key_path, "ClientSideGraphics", csg_str).await?;
        }

        // Set ClientSideWithRender
        if let Some(cswr) = settings.client_side_with_render {
            let cswr_str = if cswr { "Y" } else { "N" };
            Self::validate_value_name("ClientSideWithRender")?;
            self.set_string_value(key_path, "ClientSideWithRender", cswr_str).await?;
        }

        // Set ClientSideAntiAliasWithRender
        if let Some(caar) = settings.client_side_antialias_with_render {
            let caar_str = if caar { "Y" } else { "N" };
            Self::validate_value_name("ClientSideAntiAliasWithRender")?;
            self.set_string_value(key_path, "ClientSideAntiAliasWithRender", caar_str).await?;
        }

        // Set ClientSideAntiAliasWithCore
        if let Some(caac) = settings.client_side_antialias_with_core {
            let caac_str = if caac { "Y" } else { "N" };
            Self::validate_value_name("ClientSideAntiAliasWithCore")?;
            self.set_string_value(key_path, "ClientSideAntiAliasWithCore", caac_str).await?;
        }

        // Set GrabFullscreen
        if let Some(grab_fs) = settings.grab_fullscreen {
            let grab_fs_str = if grab_fs { "Y" } else { "N" };
            Self::validate_value_name("GrabFullscreen")?;
            self.set_string_value(key_path, "GrabFullscreen", grab_fs_str).await?;
        }

        // Set GrabPointer
        if let Some(grab_ptr) = settings.grab_pointer {
            let grab_ptr_str = if grab_ptr { "Y" } else { "N" };
            Self::validate_value_name("GrabPointer")?;
            self.set_string_value(key_path, "GrabPointer", grab_ptr_str).await?;
        }

        // Set Managed
        if let Some(managed) = settings.managed {
            let managed_str = if managed { "Y" } else { "N" };
            Self::validate_value_name("Managed")?;
            self.set_string_value(key_path, "Managed", managed_str).await?;
        }

        // Set UseXRandR
        if let Some(xrandr) = settings.use_xrandr {
            let xrandr_str = if xrandr { "Y" } else { "N" };
            Self::validate_value_name("UseXRandR")?;
            self.set_string_value(key_path, "UseXRandR", xrandr_str).await?;
        }

        // Set UseXVidMode
        if let Some(xvid) = settings.use_xvid_mode {
            let xvid_str = if xvid { "Y" } else { "N" };
            Self::validate_value_name("UseXVidMode")?;
            self.set_string_value(key_path, "UseXVidMode", xvid_str).await?;
        }

        Ok(())
    }

    /// Get DPI settings
    async fn get_dpi_settings(&self) -> Result<Option<DpiSettings>> {
        let key_path = "Control Panel\\Desktop";
        let log_pixels = self.get_dword_value(key_path, "LogPixels").await?;

        if log_pixels.is_none() {
            return Ok(None);
        }

        Ok(Some(DpiSettings { log_pixels }))
    }

    /// Set DPI settings
    async fn set_dpi_settings(&mut self, settings: &DpiSettings) -> Result<()> {
        let key_path = "Control Panel\\Desktop";
        Self::validate_key_path(key_path)?;

        if let Some(dpi) = settings.log_pixels {
            if dpi < 96 {
                return Err(PrefixError::ValidationError(
                    "DPI must be at least 96".to_string(),
                ));
            }
            Self::validate_value_name("LogPixels")?;
            self.set_dword_value(key_path, "LogPixels", dpi).await?;
        }

        Ok(())
    }

    /// Get Mac Driver settings
    async fn get_mac_driver_settings(&self) -> Result<Option<MacDriverSettings>> {
        let key_path = "Software\\Wine\\Mac Driver";

        println!("DEBUG: Looking for Mac Driver settings at path: {}", key_path);

        let allow_vertical_sync = self.get_string_value(key_path, "AllowVerticalSync").await?;
        let capture_displays_for_fullscreen = self.get_string_value(key_path, "CaptureDisplaysForFullscreen").await?;
        let use_precise_scrolling = self.get_string_value(key_path, "UsePreciseScrolling").await?;
        let retina_mode = self.get_string_value(key_path, "RetinaMode").await?;
        let windows_float_when_inactive = self.get_string_value(key_path, "WindowsFloatWhenInactive").await?;

        println!("DEBUG: Retrieved Mac Driver settings - RetinaMode: {:?}", retina_mode);

        // If none of the settings exist, return None
        if allow_vertical_sync.is_none() && capture_displays_for_fullscreen.is_none()
            && use_precise_scrolling.is_none() && retina_mode.is_none() && windows_float_when_inactive.is_none() {
            return Ok(None);
        }
        
        let mut settings = MacDriverSettings::new();
        
        if let Some(sync_str) = allow_vertical_sync {
            settings.allow_vertical_sync = Some(sync_str == "y");
        }
        
        if let Some(capture_str) = capture_displays_for_fullscreen {
            settings.capture_displays_for_fullscreen = Some(capture_str == "y");
        }
        
        if let Some(scroll_str) = use_precise_scrolling {
            settings.use_precise_scrolling = Some(scroll_str == "y");
        }

        if let Some(retina_str) = retina_mode {
            settings.retina_mode = Some(retina_str == "Y");
        }

        if let Some(float_str) = windows_float_when_inactive {
            if let Some(float_mode) = WindowsFloatWhenInactive::from_string(&float_str) {
                settings.windows_float_when_inactive = Some(float_mode);
            }
        }
        
        Ok(Some(settings))
    }

    /// Set Mac Driver settings
    async fn set_mac_driver_settings(&mut self, settings: &MacDriverSettings) -> Result<()> {
        let key_path = "Software\\Wine\\Mac Driver";
        Self::validate_key_path(key_path)?;
        
        // Set AllowVerticalSync
        if let Some(sync) = settings.allow_vertical_sync {
            let sync_str = if sync { "y" } else { "n" };
            Self::validate_value_name("AllowVerticalSync")?;
            self.set_string_value(key_path, "AllowVerticalSync", sync_str).await?;
        }
        
        // Set CaptureDisplaysForFullscreen
        if let Some(capture) = settings.capture_displays_for_fullscreen {
            let capture_str = if capture { "y" } else { "n" };
            Self::validate_value_name("CaptureDisplaysForFullscreen")?;
            self.set_string_value(key_path, "CaptureDisplaysForFullscreen", capture_str).await?;
        }
        
        // Set UsePreciseScrolling
        if let Some(scrolling) = settings.use_precise_scrolling {
            let scroll_str = if scrolling { "y" } else { "n" };
            Self::validate_value_name("UsePreciseScrolling")?;
            self.set_string_value(key_path, "UsePreciseScrolling", scroll_str).await?;
        }

        // Set RetinaMode
        if let Some(retina) = settings.retina_mode {
            let retina_str = if retina { "Y" } else { "n" };
            Self::validate_value_name("RetinaMode")?;
            self.set_string_value(key_path, "RetinaMode", retina_str).await?;
        }
        
        // Set WindowsFloatWhenInactive
        if let Some(float_mode) = &settings.windows_float_when_inactive {
            Self::validate_value_name("WindowsFloatWhenInactive")?;
            self.set_string_value(key_path, "WindowsFloatWhenInactive", float_mode.to_string()).await?;
        }
        
        Ok(())
    }

    /// Validate registry entries
    async fn validate_registry(&self) -> Result<Vec<ValidationError>> {
        let mut errors = Vec::new();
        
        // Get all keys and validate them
        let all_keys = self.registry.find_keys("").await?;
        
        for key_path in all_keys {
            // Validate key path format
            if let Err(e) = Self::validate_key_path(&key_path) {
                errors.push(ValidationError::new(key_path.clone(), None, e.to_string()));
                continue;
            }
            
            // Get all values in this key
            if let Ok(values) = self.registry.get_key_values(&key_path).await {
                for (value_name, _) in &values {
                    if let Err(e) = Self::validate_value_name(value_name) {
                        errors.push(ValidationError::new(key_path.clone(), Some(value_name.clone()), e.to_string()));
                    }
                }
            }
        }
        
        Ok(errors)
    }

    /// Get all registry keys for debugging
    async fn get_all_keys(&self) -> Result<Vec<String>> {
        self.registry.find_keys("").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefix::regeditor::cache::InMemoryRegistryCache;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_registry_editor_basic() {
        let cache = Arc::new(InMemoryRegistryCache::with_default_ttl());
        let mut editor = RegistryEditor::new(cache);
        
        // Test setting and getting Windows version
        editor.set_windows_version("win10").await.unwrap();
        let version = editor.get_windows_version().await.unwrap();
        assert_eq!(version, Some("win10".to_string()));
        
        // Test setting and getting Direct3D renderer
        editor.set_d3d_renderer("vulkan").await.unwrap();
        let renderer = editor.get_d3d_renderer().await.unwrap();
        assert_eq!(renderer, Some("vulkan".to_string()));
        
        // Test CSMT setting
        editor.set_d3d_csmt(true).await.unwrap();
        let csmt = editor.get_d3d_csmt().await.unwrap();
        assert_eq!(csmt, Some(1));
        
        editor.set_d3d_csmt(false).await.unwrap();
        let csmt = editor.get_d3d_csmt().await.unwrap();
        assert_eq!(csmt, Some(0));
    }

    #[tokio::test]
    async fn test_mac_driver_settings() {
        let cache = Arc::new(InMemoryRegistryCache::with_default_ttl());
        let mut editor = RegistryEditor::new(cache);
        
        // Test setting and getting Mac Driver settings
        let mut mac_settings = MacDriverSettings::new();
        mac_settings.allow_vertical_sync = Some(true);
        mac_settings.capture_displays_for_fullscreen = Some(false);
        mac_settings.use_precise_scrolling = Some(true);
        mac_settings.windows_float_when_inactive = Some(WindowsFloatWhenInactive::NonFullscreen);
        
        editor.set_mac_driver_settings(&mac_settings).await.unwrap();
        
        let retrieved_settings = editor.get_mac_driver_settings().await.unwrap();
        assert!(retrieved_settings.is_some());
        
        let settings = retrieved_settings.unwrap();
        assert_eq!(settings.allow_vertical_sync, Some(true));
        assert_eq!(settings.capture_displays_for_fullscreen, Some(false));
        assert_eq!(settings.use_precise_scrolling, Some(true));
        assert_eq!(settings.windows_float_when_inactive, Some(WindowsFloatWhenInactive::NonFullscreen));
        
        // Test individual settings - create a new editor to avoid state conflicts
        let cache2 = Arc::new(InMemoryRegistryCache::with_default_ttl());
        let mut editor2 = RegistryEditor::new(cache2);
        
        let mut individual_settings = MacDriverSettings::new();
        individual_settings.allow_vertical_sync = Some(false);
        
        editor2.set_mac_driver_settings(&individual_settings).await.unwrap();
        
        let retrieved_individual = editor2.get_mac_driver_settings().await.unwrap();
        assert!(retrieved_individual.is_some());
        
        let individual = retrieved_individual.unwrap();
        assert_eq!(individual.allow_vertical_sync, Some(false));
        assert_eq!(individual.capture_displays_for_fullscreen, None);
        assert_eq!(individual.use_precise_scrolling, None);
        assert_eq!(individual.windows_float_when_inactive, None);
    }

    #[tokio::test]
    async fn test_font_replacements() {
        let cache = Arc::new(InMemoryRegistryCache::with_default_ttl());
        let mut editor = RegistryEditor::new(cache);

        // Test adding Japanese font replacements
        editor.add_font_replacement("MS UI Gothic", "ヒラギノ丸ゴ ProN W4").await.unwrap();
        editor.add_font_replacement("ＭＳ ゴシック", "ヒラギノ丸ゴ ProN W4").await.unwrap();

        // Test getting font replacements
        let replacements = editor.get_font_replacements().await.unwrap();
        assert_eq!(replacements.len(), 2);
        
        // Check that both replacements were added correctly
        let ms_ui_gothic = replacements.iter().find(|r| r.original == "MS UI Gothic");
        let ms_gothic = replacements.iter().find(|r| r.original == "ＭＳ ゴシック");
        
        assert!(ms_ui_gothic.is_some());
        assert!(ms_gothic.is_some());
        assert_eq!(ms_ui_gothic.unwrap().replacement, "ヒラギノ丸ゴ ProN W4");
        assert_eq!(ms_gothic.unwrap().replacement, "ヒラギノ丸ゴ ProN W4");

        // Test removing a font replacement
        editor.remove_font_replacement("MS UI Gothic").await.unwrap();
        let replacements_after_remove = editor.get_font_replacements().await.unwrap();
        assert_eq!(replacements_after_remove.len(), 1);
        assert!(replacements_after_remove.iter().any(|r| r.original == "ＭＳ ゴシック"));
    }
}