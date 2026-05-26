use crate::keys::*;
use async_trait::async_trait;
use base::error::Result;
use std::path::PathBuf;

#[async_trait]
pub trait RegEditor: Send + Sync {
    async fn load_registry(&mut self, prefix_path: &PathBuf) -> Result<()>;
    async fn save_registry(&self, prefix_path: &PathBuf) -> Result<()>;
    async fn get_windows_version(&self) -> Result<Option<String>>;
    async fn set_windows_version(&mut self, version: &str) -> Result<()>;
    async fn get_d3d_renderer(&self) -> Result<Option<String>>;
    async fn set_d3d_renderer(&mut self, renderer: &str) -> Result<()>;
    async fn get_d3d_csmt(&self) -> Result<Option<u32>>;
    async fn set_d3d_csmt(&mut self, enabled: bool) -> Result<()>;
    async fn get_offscreen_rendering_mode(&self) -> Result<Option<String>>;
    async fn set_offscreen_rendering_mode(&mut self, mode: &str) -> Result<()>;
    async fn get_mouse_warp_override(&self) -> Result<Option<String>>;
    async fn set_mouse_warp_override(&mut self, mode: &str) -> Result<()>;
    async fn get_audio_driver(&self) -> Result<Option<String>>;
    async fn set_audio_driver(&mut self, driver: &str) -> Result<()>;
    async fn get_graphics_driver(&self) -> Result<Option<String>>;
    async fn set_graphics_driver(&mut self, driver: &str) -> Result<()>;
    async fn get_desktop_settings(&self) -> Result<Option<DesktopSettings>>;
    async fn set_desktop_settings(&mut self, settings: &DesktopSettings) -> Result<()>;
    async fn get_font_replacements(&self) -> Result<Vec<FontReplacement>>;
    async fn add_font_replacement(&mut self, original: &str, replacement: &str) -> Result<()>;
    async fn remove_font_replacement(&mut self, original: &str) -> Result<()>;
    async fn get_dll_overrides(&self) -> Result<Vec<DllOverride>>;
    async fn add_dll_override(&mut self, dll: &str, setting: DllOverrideSetting) -> Result<()>;
    async fn remove_dll_override(&mut self, dll: &str) -> Result<()>;
    async fn get_video_memory_size(&self) -> Result<Option<u32>>;
    async fn set_video_memory_size(&mut self, size_mb: u32) -> Result<()>;
    async fn get_shader_model_settings(&self) -> Result<Option<ShaderModelSettings>>;
    async fn set_shader_model_settings(&mut self, settings: &ShaderModelSettings) -> Result<()>;
    async fn get_virtual_desktop(&self) -> Result<Option<VirtualDesktopSettings>>;
    async fn set_virtual_desktop(&mut self, settings: &VirtualDesktopSettings) -> Result<()>;
    async fn get_app_settings(&self, app_name: &str) -> Result<Option<AppSettings>>;
    async fn set_app_settings(&mut self, app_name: &str, settings: &AppSettings) -> Result<()>;
    async fn remove_app_settings(&mut self, app_name: &str) -> Result<()>;
    async fn get_x11_driver_settings(&self) -> Result<Option<X11DriverSettings>>;
    async fn set_x11_driver_settings(&mut self, settings: &X11DriverSettings) -> Result<()>;
    async fn get_dpi_settings(&self) -> Result<Option<DpiSettings>>;
    async fn set_dpi_settings(&mut self, settings: &DpiSettings) -> Result<()>;
    async fn get_mac_driver_settings(&self) -> Result<Option<MacDriverSettings>>;
    async fn set_mac_driver_settings(&mut self, settings: &MacDriverSettings) -> Result<()>;
    async fn validate_registry(&self) -> Result<Vec<ValidationError>>;
    async fn get_all_keys(&self) -> Result<Vec<String>>;
}

#[async_trait]
pub trait RegistryCache: Send + Sync {
    async fn get_cached_registry(
        &self,
        prefix_path: &PathBuf,
    ) -> Result<Option<crate::WineRegistry>>;
    async fn cache_registry(
        &self,
        prefix_path: &PathBuf,
        registry: crate::WineRegistry,
    ) -> Result<()>;
    async fn invalidate_cache(&self, prefix_path: &PathBuf) -> Result<()>;
    async fn clear_all_cache(&self) -> Result<()>;
}
