use crate::WineRegistry;
use crate::keys::*;
use crate::traits::{RegEditor, RegistryCache};
use async_trait::async_trait;
use base::error::{PrefixError, Result};
use regashii::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub struct RegistryEditor {
    pub registry: WineRegistry,
    cache: Arc<dyn RegistryCache>,
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
    pub fn new(cache: Arc<dyn RegistryCache>) -> Self {
        Self {
            registry: WineRegistry::new(),
            cache,
            prefix_path: None,
        }
    }

    pub async fn with_prefix(cache: Arc<dyn RegistryCache>, prefix_path: &PathBuf) -> Result<Self> {
        if let Some(cached_registry) = cache.get_cached_registry(prefix_path).await? {
            return Ok(Self {
                registry: cached_registry,
                cache,
                prefix_path: Some(prefix_path.clone()),
            });
        }
        let registry = WineRegistry::load_from_prefix(prefix_path).await?;
        cache.cache_registry(prefix_path, registry.clone()).await?;
        Ok(Self {
            registry,
            cache,
            prefix_path: Some(prefix_path.clone()),
        })
    }

    fn get_registry_path(prefix_path: &PathBuf) -> Result<PathBuf> {
        let reg_path = prefix_path
            .join("user.reg")
            .canonicalize()
            .map_err(|e| PrefixError::InvalidPath(format!("Invalid prefix path: {}", e)))?;
        Ok(reg_path)
    }

    async fn get_string_value(&self, key_path: &str, value_name: &str) -> Result<Option<String>> {
        if let Some(value) = self.registry.get_value(key_path, value_name).await? {
            match value {
                Value::Sz(s) => Ok(Some(s)),
                Value::ExpandSz(s) => Ok(Some(s)),
                _ => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    async fn set_string_value(
        &mut self,
        key_path: &str,
        value_name: &str,
        value: &str,
    ) -> Result<()> {
        let reg_value = Value::Sz(value.to_string());
        self.registry
            .set_value(key_path, value_name, reg_value)
            .await
    }

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

    async fn set_dword_value(
        &mut self,
        key_path: &str,
        value_name: &str,
        value: u32,
    ) -> Result<()> {
        let reg_value = Value::Dword(value);
        self.registry
            .set_value(key_path, value_name, reg_value)
            .await
    }

    fn validate_key_path(key_path: &str) -> Result<()> {
        if key_path.is_empty() {
            return Err(PrefixError::ValidationError(
                "Key path cannot be empty".to_string(),
            ));
        }
        if !key_path.starts_with("Software") && !key_path.starts_with("Control Panel") {
            return Err(PrefixError::ValidationError(format!(
                "Invalid key path format: {}. Expected to start with 'Software' or 'Control Panel'",
                key_path
            )));
        }
        Ok(())
    }

    fn validate_value_name(value_name: &str) -> Result<()> {
        if value_name.is_empty() {
            return Err(PrefixError::ValidationError(
                "Value name cannot be empty".to_string(),
            ));
        }
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

fn is_mac_option_true(v: &str) -> bool {
    matches!(v, "Y" | "y" | "T" | "t" | "1")
}

#[async_trait]
impl RegEditor for RegistryEditor {
    async fn load_registry(&mut self, prefix_path: &PathBuf) -> Result<()> {
        let registry_path = Self::get_registry_path(prefix_path)?;
        self.registry = WineRegistry::load_from_file(&registry_path).await?;
        self.prefix_path = Some(prefix_path.clone());
        self.cache
            .cache_registry(prefix_path, self.registry.clone())
            .await?;
        Ok(())
    }

    async fn save_registry(&self, prefix_path: &PathBuf) -> Result<()> {
        let registry_path = Self::get_registry_path(prefix_path)?;
        self.registry.save_to_file(&registry_path).await?;
        self.cache.invalidate_cache(prefix_path).await?;
        Ok(())
    }

    async fn get_windows_version(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine", "Version").await
    }

    async fn set_windows_version(&mut self, version: &str) -> Result<()> {
        let key_path = "Software\\Wine";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("Version")?;
        if let Some(parsed_version) = WindowsVersion::from_string(version) {
            self.set_string_value(key_path, "Version", parsed_version.to_string())
                .await
        } else {
            Err(PrefixError::ValidationError(format!(
                "Invalid Windows version: {}",
                version
            )))
        }
    }

    async fn get_d3d_renderer(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\Direct3D", "renderer")
            .await
    }

    async fn set_d3d_renderer(&mut self, renderer: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("renderer")?;
        if let Some(parsed_renderer) = D3DRenderer::from_string(renderer) {
            self.set_string_value(key_path, "renderer", parsed_renderer.to_string())
                .await
        } else {
            Err(PrefixError::ValidationError(format!(
                "Invalid Direct3D renderer: {}",
                renderer
            )))
        }
    }

    async fn get_d3d_csmt(&self) -> Result<Option<u32>> {
        self.get_dword_value("Software\\Wine\\Direct3D", "csmt")
            .await
    }

    async fn set_d3d_csmt(&mut self, enabled: bool) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("csmt")?;
        let value = if enabled { 1 } else { 0 };
        self.set_dword_value(key_path, "csmt", value).await
    }

    async fn get_offscreen_rendering_mode(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\Direct3D", "OffscreenRenderingMode")
            .await
    }

    async fn set_offscreen_rendering_mode(&mut self, mode: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("OffscreenRenderingMode")?;
        if let Some(parsed_mode) = OffscreenRenderingMode::from_string(mode) {
            self.set_string_value(key_path, "OffscreenRenderingMode", parsed_mode.to_string())
                .await
        } else {
            Err(PrefixError::ValidationError(format!(
                "Invalid offscreen rendering mode: {}",
                mode
            )))
        }
    }

    async fn get_mouse_warp_override(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\DirectInput", "MouseWarpOverride")
            .await
    }

    async fn set_mouse_warp_override(&mut self, mode: &str) -> Result<()> {
        let key_path = "Software\\Wine\\DirectInput";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("MouseWarpOverride")?;
        if let Some(parsed_mode) = MouseWarpOverride::from_string(mode) {
            self.set_string_value(key_path, "MouseWarpOverride", parsed_mode.to_string())
                .await
        } else {
            Err(PrefixError::ValidationError(format!(
                "Invalid mouse warp override: {}",
                mode
            )))
        }
    }

    async fn get_audio_driver(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\Drivers\\Audio", "")
            .await
    }

    async fn set_audio_driver(&mut self, driver: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Drivers\\Audio";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("")?;
        if let Some(parsed_driver) = AudioDriver::from_string(driver) {
            self.set_string_value(key_path, "", parsed_driver.to_string())
                .await
        } else {
            Err(PrefixError::ValidationError(format!(
                "Invalid audio driver: {}",
                driver
            )))
        }
    }

    async fn get_graphics_driver(&self) -> Result<Option<String>> {
        self.get_string_value("Software\\Wine\\Drivers\\Graphics", "")
            .await
    }

    async fn set_graphics_driver(&mut self, driver: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Drivers\\Graphics";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("")?;
        if let Some(parsed_driver) = GraphicsDriver::from_string(driver) {
            self.set_string_value(key_path, "", parsed_driver.to_string())
                .await
        } else {
            Err(PrefixError::ValidationError(format!(
                "Invalid graphics driver: {}",
                driver
            )))
        }
    }

    async fn get_desktop_settings(&self) -> Result<Option<DesktopSettings>> {
        let key_path = "Software\\Wine\\Explorer";
        let desktop = self.get_string_value(key_path, "Desktop").await?;
        let show_systray = self
            .get_dword_value(key_path, "ShowSystray")
            .await?
            .unwrap_or(1)
            != 0;
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

    async fn set_desktop_settings(&mut self, settings: &DesktopSettings) -> Result<()> {
        let key_path = "Software\\Wine\\Explorer";
        Self::validate_key_path(key_path)?;
        if let Some(desktop) = &settings.desktop {
            self.set_string_value(key_path, "Desktop", desktop).await?;
        }
        let systray_value = if settings.show_systray { 1 } else { 0 };
        self.set_dword_value(key_path, "ShowSystray", systray_value)
            .await?;
        let desktops_path = format!("{}\\Desktops", key_path);
        for (name, size) in &settings.desktops {
            Self::validate_value_name(name)?;
            self.set_string_value(&desktops_path, name, &size.to_string())
                .await?;
        }
        Ok(())
    }

    async fn get_font_replacements(&self) -> Result<Vec<FontReplacement>> {
        let key_path = "Software\\Wine\\Fonts\\Replacements";
        let mut replacements = Vec::new();
        let values = self.registry.get_key_values(key_path).await?;
        for (original, value) in values {
            if let Value::Sz(replacement) = value {
                replacements.push(FontReplacement::new(original, replacement));
            }
        }
        Ok(replacements)
    }

    async fn add_font_replacement(&mut self, original: &str, replacement: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Fonts\\Replacements";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name(original)?;
        self.set_string_value(key_path, original, replacement).await
    }

    async fn remove_font_replacement(&mut self, original: &str) -> Result<()> {
        let key_path = "Software\\Wine\\Fonts\\Replacements";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name(original)?;
        self.registry.delete_value(key_path, original).await
    }

    async fn get_dll_overrides(&self) -> Result<Vec<DllOverride>> {
        let key_path = "Software\\Wine\\DllOverrides";
        let mut overrides = Vec::new();
        let values = self.registry.get_key_values(key_path).await?;
        for (dll, value) in values {
            if let Value::Sz(setting_str) = value {
                if let Some(setting) = DllOverrideSetting::from_string(&setting_str) {
                    overrides.push(DllOverride { dll, setting });
                }
            }
        }
        Ok(overrides)
    }

    async fn add_dll_override(&mut self, dll: &str, setting: DllOverrideSetting) -> Result<()> {
        let key_path = "Software\\Wine\\DllOverrides";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name(dll)?;
        self.set_string_value(key_path, dll, setting.to_string())
            .await
    }

    async fn remove_dll_override(&mut self, dll: &str) -> Result<()> {
        let key_path = "Software\\Wine\\DllOverrides";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name(dll)?;
        self.registry.delete_value(key_path, dll).await
    }

    async fn get_video_memory_size(&self) -> Result<Option<u32>> {
        self.get_dword_value("Software\\Wine\\Direct3D", "VideoMemorySize")
            .await
    }

    async fn set_video_memory_size(&mut self, size_mb: u32) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        Self::validate_value_name("VideoMemorySize")?;
        if size_mb == 0 || size_mb > 16384 {
            return Err(PrefixError::ValidationError(
                "Video memory size must be between 1 and 16384 MB".to_string(),
            ));
        }
        self.set_dword_value(key_path, "VideoMemorySize", size_mb)
            .await
    }

    async fn get_shader_model_settings(&self) -> Result<Option<ShaderModelSettings>> {
        let key_path = "Software\\Wine\\Direct3D";
        let max_shader_model_vs = self.get_dword_value(key_path, "MaxShaderModelVS").await?;
        let max_shader_model_ps = self.get_dword_value(key_path, "MaxShaderModelPS").await?;
        let max_shader_model_gs = self.get_dword_value(key_path, "MaxShaderModelGS").await?;
        let max_shader_model_hs = self.get_dword_value(key_path, "MaxShaderModelHS").await?;
        let max_shader_model_ds = self.get_dword_value(key_path, "MaxShaderModelDS").await?;
        let max_shader_model_cs = self.get_dword_value(key_path, "MaxShaderModelCS").await?;
        if max_shader_model_vs.is_none()
            && max_shader_model_ps.is_none()
            && max_shader_model_gs.is_none()
            && max_shader_model_hs.is_none()
            && max_shader_model_ds.is_none()
            && max_shader_model_cs.is_none()
        {
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

    async fn set_shader_model_settings(&mut self, settings: &ShaderModelSettings) -> Result<()> {
        let key_path = "Software\\Wine\\Direct3D";
        Self::validate_key_path(key_path)?;
        if let Some(vs) = settings.max_shader_model_vs {
            Self::validate_value_name("MaxShaderModelVS")?;
            self.set_dword_value(key_path, "MaxShaderModelVS", vs)
                .await?;
        }
        if let Some(ps) = settings.max_shader_model_ps {
            Self::validate_value_name("MaxShaderModelPS")?;
            self.set_dword_value(key_path, "MaxShaderModelPS", ps)
                .await?;
        }
        if let Some(gs) = settings.max_shader_model_gs {
            Self::validate_value_name("MaxShaderModelGS")?;
            self.set_dword_value(key_path, "MaxShaderModelGS", gs)
                .await?;
        }
        if let Some(hs) = settings.max_shader_model_hs {
            Self::validate_value_name("MaxShaderModelHS")?;
            self.set_dword_value(key_path, "MaxShaderModelHS", hs)
                .await?;
        }
        if let Some(ds) = settings.max_shader_model_ds {
            Self::validate_value_name("MaxShaderModelDS")?;
            self.set_dword_value(key_path, "MaxShaderModelDS", ds)
                .await?;
        }
        if let Some(cs) = settings.max_shader_model_cs {
            Self::validate_value_name("MaxShaderModelCS")?;
            self.set_dword_value(key_path, "MaxShaderModelCS", cs)
                .await?;
        }
        Ok(())
    }

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

    async fn set_virtual_desktop(&mut self, settings: &VirtualDesktopSettings) -> Result<()> {
        if settings.enabled {
            let desktop_settings = DesktopSettings {
                desktop: Some("Default".to_string()),
                desktops: {
                    let mut desktops = HashMap::new();
                    desktops.insert(
                        "Default".to_string(),
                        DesktopSize::new(settings.width, settings.height),
                    );
                    desktops
                },
                show_systray: false,
            };
            self.set_desktop_settings(&desktop_settings).await
        } else {
            let key_path = "Software\\Wine\\Explorer";
            Self::validate_key_path(key_path)?;
            self.registry.delete_value(key_path, "Desktop").await?;
            let desktops_path = format!("{}\\Desktops", key_path);
            self.registry
                .delete_value(&desktops_path, "Default")
                .await?;
            Ok(())
        }
    }

    async fn get_app_settings(&self, app_name: &str) -> Result<Option<AppSettings>> {
        let key_path = format!("Software\\Wine\\AppDefaults\\{}", app_name);
        if !self.registry.key_exists(&key_path).await? {
            return Ok(None);
        }
        let mut app_settings = AppSettings::new(app_name.to_string());
        let dll_overrides_path = format!("{}\\DllOverrides", key_path);
        let values = self.registry.get_key_values(&dll_overrides_path).await?;
        for (dll, value) in values {
            if let Value::Sz(setting_str) = value {
                if let Some(setting) = DllOverrideSetting::from_string(&setting_str) {
                    app_settings
                        .dll_overrides
                        .push(DllOverride { dll, setting });
                }
            }
        }
        if let Some(renderer_str) = self
            .get_string_value(&format!("{}\\Direct3D", key_path), "renderer")
            .await?
        {
            if let Some(renderer) = D3DRenderer::from_string(&renderer_str) {
                app_settings.d3d_renderer = Some(renderer);
            }
        }
        if let Some(mode_str) = self
            .get_string_value(&format!("{}\\Direct3D", key_path), "OffscreenRenderingMode")
            .await?
        {
            if let Some(mode) = OffscreenRenderingMode::from_string(&mode_str) {
                app_settings.offscreen_rendering_mode = Some(mode);
            }
        }
        Ok(Some(app_settings))
    }

    async fn set_app_settings(&mut self, app_name: &str, settings: &AppSettings) -> Result<()> {
        let base_key_path = format!("Software\\Wine\\AppDefaults\\{}", app_name);
        Self::validate_key_path(&base_key_path)?;
        if !settings.dll_overrides.is_empty() {
            let dll_overrides_path = format!("{}\\DllOverrides", base_key_path);
            for dll_override in &settings.dll_overrides {
                Self::validate_value_name(&dll_override.dll)?;
                self.set_string_value(
                    &dll_overrides_path,
                    &dll_override.dll,
                    dll_override.setting.to_string(),
                )
                .await?;
            }
        }
        if let Some(renderer) = &settings.d3d_renderer {
            let d3d_path = format!("{}\\Direct3D", base_key_path);
            Self::validate_value_name("renderer")?;
            self.set_string_value(&d3d_path, "renderer", renderer.to_string())
                .await?;
        }
        if let Some(mode) = &settings.offscreen_rendering_mode {
            let d3d_path = format!("{}\\Direct3D", base_key_path);
            Self::validate_value_name("OffscreenRenderingMode")?;
            self.set_string_value(&d3d_path, "OffscreenRenderingMode", mode.to_string())
                .await?;
        }
        for (key, value) in &settings.custom_settings {
            Self::validate_value_name(key)?;
            self.set_string_value(&base_key_path, key, value).await?;
        }
        Ok(())
    }

    async fn remove_app_settings(&mut self, app_name: &str) -> Result<()> {
        let key_path = format!("Software\\Wine\\AppDefaults\\{}", app_name);
        Self::validate_key_path(&key_path)?;
        self.registry.delete_key(&key_path).await
    }

    async fn get_x11_driver_settings(&self) -> Result<Option<X11DriverSettings>> {
        let key_path = "Software\\Wine\\X11 Driver";
        let decorated = self.get_string_value(key_path, "Decorated").await?;
        let client_side_graphics = self
            .get_string_value(key_path, "ClientSideGraphics")
            .await?;
        let client_side_with_render = self
            .get_string_value(key_path, "ClientSideWithRender")
            .await?;
        let client_side_antialias_with_render = self
            .get_string_value(key_path, "ClientSideAntiAliasWithRender")
            .await?;
        let client_side_antialias_with_core = self
            .get_string_value(key_path, "ClientSideAntiAliasWithCore")
            .await?;
        let grab_fullscreen = self.get_string_value(key_path, "GrabFullscreen").await?;
        let grab_pointer = self.get_string_value(key_path, "GrabPointer").await?;
        let managed = self.get_string_value(key_path, "Managed").await?;
        let use_xrandr = self.get_string_value(key_path, "UseXRandR").await?;
        let use_xvid_mode = self.get_string_value(key_path, "UseXVidMode").await?;
        if decorated.is_none()
            && client_side_graphics.is_none()
            && client_side_with_render.is_none()
            && client_side_antialias_with_render.is_none()
            && client_side_antialias_with_core.is_none()
            && grab_fullscreen.is_none()
            && grab_pointer.is_none()
            && managed.is_none()
            && use_xrandr.is_none()
            && use_xvid_mode.is_none()
        {
            return Ok(None);
        }
        let mut settings = X11DriverSettings::new();
        if let Some(v) = decorated {
            settings.decorated = Some(v != "N");
        }
        if let Some(v) = client_side_graphics {
            settings.client_side_graphics = Some(v != "N");
        }
        if let Some(v) = client_side_with_render {
            settings.client_side_with_render = Some(v != "N");
        }
        if let Some(v) = client_side_antialias_with_render {
            settings.client_side_antialias_with_render = Some(v != "N");
        }
        if let Some(v) = client_side_antialias_with_core {
            settings.client_side_antialias_with_core = Some(v != "N");
        }
        if let Some(v) = grab_fullscreen {
            settings.grab_fullscreen = Some(v == "Y");
        }
        if let Some(v) = grab_pointer {
            settings.grab_pointer = Some(v != "N");
        }
        if let Some(v) = managed {
            settings.managed = Some(v != "N");
        }
        if let Some(v) = use_xrandr {
            settings.use_xrandr = Some(v != "N");
        }
        if let Some(v) = use_xvid_mode {
            settings.use_xvid_mode = Some(v == "Y");
        }
        Ok(Some(settings))
    }

    async fn set_x11_driver_settings(&mut self, settings: &X11DriverSettings) -> Result<()> {
        let key_path = "Software\\Wine\\X11 Driver";
        Self::validate_key_path(key_path)?;
        if let Some(v) = settings.decorated {
            Self::validate_value_name("Decorated")?;
            self.set_string_value(key_path, "Decorated", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.client_side_graphics {
            Self::validate_value_name("ClientSideGraphics")?;
            self.set_string_value(key_path, "ClientSideGraphics", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.client_side_with_render {
            Self::validate_value_name("ClientSideWithRender")?;
            self.set_string_value(key_path, "ClientSideWithRender", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.client_side_antialias_with_render {
            Self::validate_value_name("ClientSideAntiAliasWithRender")?;
            self.set_string_value(
                key_path,
                "ClientSideAntiAliasWithRender",
                if v { "Y" } else { "N" },
            )
            .await?;
        }
        if let Some(v) = settings.client_side_antialias_with_core {
            Self::validate_value_name("ClientSideAntiAliasWithCore")?;
            self.set_string_value(
                key_path,
                "ClientSideAntiAliasWithCore",
                if v { "Y" } else { "N" },
            )
            .await?;
        }
        if let Some(v) = settings.grab_fullscreen {
            Self::validate_value_name("GrabFullscreen")?;
            self.set_string_value(key_path, "GrabFullscreen", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.grab_pointer {
            Self::validate_value_name("GrabPointer")?;
            self.set_string_value(key_path, "GrabPointer", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.managed {
            Self::validate_value_name("Managed")?;
            self.set_string_value(key_path, "Managed", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.use_xrandr {
            Self::validate_value_name("UseXRandR")?;
            self.set_string_value(key_path, "UseXRandR", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.use_xvid_mode {
            Self::validate_value_name("UseXVidMode")?;
            self.set_string_value(key_path, "UseXVidMode", if v { "Y" } else { "N" })
                .await?;
        }
        Ok(())
    }

    async fn get_dpi_settings(&self) -> Result<Option<DpiSettings>> {
        let key_path = "Control Panel\\Desktop";
        let log_pixels = self.get_dword_value(key_path, "LogPixels").await?;
        if log_pixels.is_none() {
            return Ok(None);
        }
        Ok(Some(DpiSettings { log_pixels }))
    }

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

    async fn get_mac_driver_settings(&self) -> Result<Option<MacDriverSettings>> {
        let key_path = "Software\\Wine\\Mac Driver";
        let allow_vertical_sync = self.get_string_value(key_path, "AllowVerticalSync").await?;
        let capture_displays_for_fullscreen = self
            .get_string_value(key_path, "CaptureDisplaysForFullscreen")
            .await?;
        let use_precise_scrolling = self
            .get_string_value(key_path, "UsePreciseScrolling")
            .await?;
        let retina_mode = self.get_string_value(key_path, "RetinaMode").await?;
        let windows_float_when_inactive = self
            .get_string_value(key_path, "WindowsFloatWhenInactive")
            .await?;
        let left_option_is_alt = self.get_string_value(key_path, "LeftOptionIsAlt").await?;
        let right_option_is_alt = self.get_string_value(key_path, "RightOptionIsAlt").await?;
        let left_command_is_ctrl = self.get_string_value(key_path, "LeftCommandIsCtrl").await?;
        let right_command_is_ctrl = self
            .get_string_value(key_path, "RightCommandIsCtrl")
            .await?;
        if allow_vertical_sync.is_none()
            && capture_displays_for_fullscreen.is_none()
            && use_precise_scrolling.is_none()
            && retina_mode.is_none()
            && windows_float_when_inactive.is_none()
            && left_option_is_alt.is_none()
            && right_option_is_alt.is_none()
            && left_command_is_ctrl.is_none()
            && right_command_is_ctrl.is_none()
        {
            return Ok(None);
        }
        let mut settings = MacDriverSettings::new();
        if let Some(v) = allow_vertical_sync {
            settings.allow_vertical_sync = Some(is_mac_option_true(&v));
        }
        if let Some(v) = capture_displays_for_fullscreen {
            settings.capture_displays_for_fullscreen = Some(is_mac_option_true(&v));
        }
        if let Some(v) = use_precise_scrolling {
            settings.use_precise_scrolling = Some(is_mac_option_true(&v));
        }
        if let Some(v) = retina_mode {
            settings.retina_mode = Some(is_mac_option_true(&v));
        }
        if let Some(v) = left_option_is_alt {
            settings.left_option_is_alt = Some(is_mac_option_true(&v));
        }
        if let Some(v) = right_option_is_alt {
            settings.right_option_is_alt = Some(is_mac_option_true(&v));
        }
        if let Some(v) = left_command_is_ctrl {
            settings.left_command_is_ctrl = Some(is_mac_option_true(&v));
        }
        if let Some(v) = right_command_is_ctrl {
            settings.right_command_is_ctrl = Some(is_mac_option_true(&v));
        }
        if let Some(float_str) = windows_float_when_inactive {
            if let Some(float_mode) = WindowsFloatWhenInactive::from_string(&float_str) {
                settings.windows_float_when_inactive = Some(float_mode);
            }
        }
        Ok(Some(settings))
    }

    async fn set_mac_driver_settings(&mut self, settings: &MacDriverSettings) -> Result<()> {
        let key_path = "Software\\Wine\\Mac Driver";
        Self::validate_key_path(key_path)?;
        if let Some(v) = settings.allow_vertical_sync {
            Self::validate_value_name("AllowVerticalSync")?;
            self.set_string_value(key_path, "AllowVerticalSync", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.capture_displays_for_fullscreen {
            Self::validate_value_name("CaptureDisplaysForFullscreen")?;
            self.set_string_value(
                key_path,
                "CaptureDisplaysForFullscreen",
                if v { "Y" } else { "N" },
            )
            .await?;
        }
        if let Some(v) = settings.use_precise_scrolling {
            Self::validate_value_name("UsePreciseScrolling")?;
            self.set_string_value(key_path, "UsePreciseScrolling", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.retina_mode {
            Self::validate_value_name("RetinaMode")?;
            self.set_string_value(key_path, "RetinaMode", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(float_mode) = &settings.windows_float_when_inactive {
            Self::validate_value_name("WindowsFloatWhenInactive")?;
            self.set_string_value(key_path, "WindowsFloatWhenInactive", float_mode.to_string())
                .await?;
        }
        if let Some(v) = settings.left_option_is_alt {
            Self::validate_value_name("LeftOptionIsAlt")?;
            self.set_string_value(key_path, "LeftOptionIsAlt", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.right_option_is_alt {
            Self::validate_value_name("RightOptionIsAlt")?;
            self.set_string_value(key_path, "RightOptionIsAlt", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.left_command_is_ctrl {
            Self::validate_value_name("LeftCommandIsCtrl")?;
            self.set_string_value(key_path, "LeftCommandIsCtrl", if v { "Y" } else { "N" })
                .await?;
        }
        if let Some(v) = settings.right_command_is_ctrl {
            Self::validate_value_name("RightCommandIsCtrl")?;
            self.set_string_value(key_path, "RightCommandIsCtrl", if v { "Y" } else { "N" })
                .await?;
        }
        Ok(())
    }

    async fn validate_registry(&self) -> Result<Vec<ValidationError>> {
        let mut errors = Vec::new();
        let all_keys = self.registry.find_keys("").await?;
        for key_path in all_keys {
            if let Err(e) = Self::validate_key_path(&key_path) {
                errors.push(ValidationError::new(key_path.clone(), None, e.to_string()));
                continue;
            }
            if let Ok(values) = self.registry.get_key_values(&key_path).await {
                for (value_name, _) in &values {
                    if let Err(e) = Self::validate_value_name(value_name) {
                        errors.push(ValidationError::new(
                            key_path.clone(),
                            Some(value_name.clone()),
                            e.to_string(),
                        ));
                    }
                }
            }
        }
        Ok(errors)
    }

    async fn get_all_keys(&self) -> Result<Vec<String>> {
        self.registry.find_keys("").await
    }
}
