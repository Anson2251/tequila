//! Registry key structures for Wine configuration
//! 
//! This module defines structured representations of commonly used Wine registry keys
//! based on the useful-wine-reg-keys.md documentation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Windows version settings for Wine
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WindowsVersion {
    Win10,
    Win81,
    Win8,
    Win7,
    Win2008,
    Vista,
    Win2003,
    WinXP,
    Win2K,
    NT40,
    WinME,
    Win98,
    Win95,
    Win31,
}

impl WindowsVersion {
    /// Convert to string representation for registry
    pub fn to_string(&self) -> &'static str {
        match self {
            WindowsVersion::Win10 => "win10",
            WindowsVersion::Win81 => "win81",
            WindowsVersion::Win8 => "win8",
            WindowsVersion::Win7 => "win7",
            WindowsVersion::Win2008 => "win2008",
            WindowsVersion::Vista => "vista",
            WindowsVersion::Win2003 => "win2003",
            WindowsVersion::WinXP => "winxp",
            WindowsVersion::Win2K => "win2k",
            WindowsVersion::NT40 => "nt40",
            WindowsVersion::WinME => "winme",
            WindowsVersion::Win98 => "win98",
            WindowsVersion::Win95 => "win95",
            WindowsVersion::Win31 => "win31",
        }
    }

    /// Parse from string
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "win10" => Some(WindowsVersion::Win10),
            "win81" => Some(WindowsVersion::Win81),
            "win8" => Some(WindowsVersion::Win8),
            "win7" => Some(WindowsVersion::Win7),
            "win2008" => Some(WindowsVersion::Win2008),
            "vista" => Some(WindowsVersion::Vista),
            "win2003" => Some(WindowsVersion::Win2003),
            "winxp" => Some(WindowsVersion::WinXP),
            "win2k" => Some(WindowsVersion::Win2K),
            "nt40" => Some(WindowsVersion::NT40),
            "winme" => Some(WindowsVersion::WinME),
            "win98" => Some(WindowsVersion::Win98),
            "win95" => Some(WindowsVersion::Win95),
            "win31" => Some(WindowsVersion::Win31),
            _ => None,
        }
    }
}

/// Direct3D renderer options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum D3DRenderer {
    GDI,
    OpenGL,
    Vulkan,
}

impl D3DRenderer {
    pub fn to_string(&self) -> &'static str {
        match self {
            D3DRenderer::GDI => "gdi",
            D3DRenderer::OpenGL => "gl",
            D3DRenderer::Vulkan => "vulkan",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "gdi" | "no3d" => Some(D3DRenderer::GDI),
            "gl" => Some(D3DRenderer::OpenGL),
            "vulkan" => Some(D3DRenderer::Vulkan),
            _ => None,
        }
    }
}

/// Offscreen rendering mode options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OffscreenRenderingMode {
    Backbuffer,
    FBO,
}

impl OffscreenRenderingMode {
    pub fn to_string(&self) -> &'static str {
        match self {
            OffscreenRenderingMode::Backbuffer => "backbuffer",
            OffscreenRenderingMode::FBO => "fbo",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "backbuffer" => Some(OffscreenRenderingMode::Backbuffer),
            "fbo" => Some(OffscreenRenderingMode::FBO),
            _ => None,
        }
    }
}

/// Mouse warp override options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MouseWarpOverride {
    Enable,
    Disable,
    Force,
}

impl MouseWarpOverride {
    pub fn to_string(&self) -> &'static str {
        match self {
            MouseWarpOverride::Enable => "enable",
            MouseWarpOverride::Disable => "disable",
            MouseWarpOverride::Force => "force",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "enable" => Some(MouseWarpOverride::Enable),
            "disable" => Some(MouseWarpOverride::Disable),
            "force" => Some(MouseWarpOverride::Force),
            _ => None,
        }
    }
}

/// Desktop settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopSettings {
    pub desktop: Option<String>,
    pub desktops: HashMap<String, DesktopSize>,
    pub show_systray: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopSize {
    pub width: u32,
    pub height: u32,
}

impl DesktopSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn to_string(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('x').collect();
        if parts.len() == 2 {
            if let (Ok(width), Ok(height)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                return Some(DesktopSize { width, height });
            }
        }
        None
    }
}

/// Font replacement settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontReplacement {
    pub original: String,
    pub replacement: String,
}

impl FontReplacement {
    pub fn new(original: String, replacement: String) -> Self {
        Self { original, replacement }
    }
}

/// DLL override settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DllOverride {
    pub dll: String,
    pub setting: DllOverrideSetting,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DllOverrideSetting {
    Native,
    Builtin,
    NativeBuiltin,
    BuiltinNative,
    Disabled,
}

impl DllOverrideSetting {
    pub fn to_string(&self) -> &'static str {
        match self {
            DllOverrideSetting::Native => "native",
            DllOverrideSetting::Builtin => "builtin",
            DllOverrideSetting::NativeBuiltin => "native,builtin",
            DllOverrideSetting::BuiltinNative => "builtin,native",
            DllOverrideSetting::Disabled => "",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "native" => Some(DllOverrideSetting::Native),
            "builtin" => Some(DllOverrideSetting::Builtin),
            "native,builtin" => Some(DllOverrideSetting::NativeBuiltin),
            "builtin,native" => Some(DllOverrideSetting::BuiltinNative),
            "" => Some(DllOverrideSetting::Disabled),
            _ => None,
        }
    }
}

/// Shader model settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShaderModelSettings {
    pub max_shader_model_vs: Option<u32>,
    pub max_shader_model_ps: Option<u32>,
    pub max_shader_model_gs: Option<u32>,
    pub max_shader_model_hs: Option<u32>,
    pub max_shader_model_ds: Option<u32>,
    pub max_shader_model_cs: Option<u32>,
}

impl ShaderModelSettings {
    pub fn new() -> Self {
        Self {
            max_shader_model_vs: None,
            max_shader_model_ps: None,
            max_shader_model_gs: None,
            max_shader_model_hs: None,
            max_shader_model_ds: None,
            max_shader_model_cs: None,
        }
    }
}

/// Virtual desktop settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VirtualDesktopSettings {
    pub enabled: bool,
    pub width: u32,
    pub height: u32,
}

impl VirtualDesktopSettings {
    pub fn new(enabled: bool, width: u32, height: u32) -> Self {
        Self { enabled, width, height }
    }
}

/// Application-specific settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppSettings {
    pub name: String,
    pub dll_overrides: Vec<DllOverride>,
    pub d3d_renderer: Option<D3DRenderer>,
    pub offscreen_rendering_mode: Option<OffscreenRenderingMode>,
    pub desktop_settings: Option<DesktopSettings>,
    pub custom_settings: HashMap<String, String>,
}

impl AppSettings {
    pub fn new(name: String) -> Self {
        Self {
            name,
            dll_overrides: Vec::new(),
            d3d_renderer: None,
            offscreen_rendering_mode: None,
            desktop_settings: None,
            custom_settings: HashMap::new(),
        }
    }
}

/// Validation error for registry entries
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    pub key_path: String,
    pub value_name: Option<String>,
    pub error_message: String,
}

impl ValidationError {
    pub fn new(key_path: String, value_name: Option<String>, error_message: String) -> Self {
        Self {
            key_path,
            value_name,
            error_message,
        }
    }
}

/// Audio driver settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AudioDriver {
    Pulse,
    ALSA,
    OSS,
    CoreAudio,
    Disabled,
}

impl AudioDriver {
    pub fn to_string(&self) -> &'static str {
        match self {
            AudioDriver::Pulse => "pulse",
            AudioDriver::ALSA => "alsa",
            AudioDriver::OSS => "oss",
            AudioDriver::CoreAudio => "coreaudio",
            AudioDriver::Disabled => "",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "pulse" => Some(AudioDriver::Pulse),
            "alsa" => Some(AudioDriver::ALSA),
            "oss" => Some(AudioDriver::OSS),
            "coreaudio" => Some(AudioDriver::CoreAudio),
            "" => Some(AudioDriver::Disabled),
            _ => None,
        }
    }
}

/// Graphics driver settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GraphicsDriver {
    X11,
    Mac,
    Null,
}

impl GraphicsDriver {
    pub fn to_string(&self) -> &'static str {
        match self {
            GraphicsDriver::X11 => "x11",
            GraphicsDriver::Mac => "mac",
            GraphicsDriver::Null => "null",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "x11" => Some(GraphicsDriver::X11),
            "mac" => Some(GraphicsDriver::Mac),
            "null" => Some(GraphicsDriver::Null),
            _ => None,
        }
    }
}

/// X11 Driver settings for Wine
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct X11DriverSettings {
    pub decorated: Option<bool>,
    pub client_side_graphics: Option<bool>,
    pub client_side_with_render: Option<bool>,
    pub client_side_antialias_with_render: Option<bool>,
    pub client_side_antialias_with_core: Option<bool>,
    pub grab_fullscreen: Option<bool>,
    pub grab_pointer: Option<bool>,
    pub managed: Option<bool>,
    pub use_xrandr: Option<bool>,
    pub use_xvid_mode: Option<bool>,
}

impl X11DriverSettings {
    pub fn new() -> Self {
        Self {
            decorated: None,
            client_side_graphics: None,
            client_side_with_render: None,
            client_side_antialias_with_render: None,
            client_side_antialias_with_core: None,
            grab_fullscreen: None,
            grab_pointer: None,
            managed: None,
            use_xrandr: None,
            use_xvid_mode: None,
        }
    }
}

/// DPI settings for Windows display
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DpiSettings {
    pub log_pixels: Option<u32>,
}

impl DpiSettings {
    pub fn new() -> Self {
        Self { log_pixels: None }
    }

    pub fn new_with_dpi(dpi: u32) -> Self {
        Self { log_pixels: Some(dpi) }
    }
}

/// Mac Driver settings for Wine on macOS
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacDriverSettings {
    pub allow_vertical_sync: Option<bool>,
    pub capture_displays_for_fullscreen: Option<bool>,
    pub use_precise_scrolling: Option<bool>,
    pub retina_mode: Option<bool>,
    pub windows_float_when_inactive: Option<WindowsFloatWhenInactive>,
}

impl MacDriverSettings {
    pub fn new() -> Self {
        Self {
            allow_vertical_sync: None,
            capture_displays_for_fullscreen: None,
            use_precise_scrolling: None,
            retina_mode: None,
            windows_float_when_inactive: None,
        }
    }
}

/// Windows float when inactive options for Mac Driver
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WindowsFloatWhenInactive {
    None,
    All,
    NonFullscreen,
}

impl WindowsFloatWhenInactive {
    pub fn to_string(&self) -> &'static str {
        match self {
            WindowsFloatWhenInactive::None => "none",
            WindowsFloatWhenInactive::All => "all",
            WindowsFloatWhenInactive::NonFullscreen => "nonfullscreen",
        }
    }

    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "none" => Some(WindowsFloatWhenInactive::None),
            "all" => Some(WindowsFloatWhenInactive::All),
            "nonfullscreen" => Some(WindowsFloatWhenInactive::NonFullscreen),
            _ => None,
        }
    }
}