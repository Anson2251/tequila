use serde::{Deserialize, Serialize};

/// Graphics translation backends installed for a runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GraphicsBackend {
    Dxmt { version: String },
    D3DMetal { version: String },
    DxvkVkd3d { dxvk_version: String, vkd3d_version: String },
}

/// Per-prefix graphics configuration, stored in tequila-config.json.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphicsConfig {
    pub backend: String,
    pub version: String,
}
