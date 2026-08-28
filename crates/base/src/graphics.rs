use serde::{Deserialize, Serialize};

/// Graphics translation backends installed for a runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GraphicsBackend {
    Dxmt {
        version: String,
    },
    D3DMetal {
        version: String,
    },
    DxvkVkd3d {
        dxvk_version: String,
        vkd3d_version: String,
    },
}

impl GraphicsBackend {
    /// Human-readable short name (used in UI, WINEDLLPATH dir naming, etc.).
    pub fn label(&self) -> &'static str {
        match self {
            GraphicsBackend::Dxmt { .. } => "dxmt",
            GraphicsBackend::D3DMetal { .. } => "d3dmetal",
            GraphicsBackend::DxvkVkd3d { .. } => "dxvk-vkd3d",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            GraphicsBackend::Dxmt { .. } => "DXMT",
            GraphicsBackend::D3DMetal { .. } => "D3DMetal",
            GraphicsBackend::DxvkVkd3d { .. } => "DXVK+VKD3D",
        }
    }

    /// Version string used in `GraphicsConfig.version`.
    pub fn version_string(&self) -> String {
        match self {
            GraphicsBackend::Dxmt { version } => version.clone(),
            GraphicsBackend::D3DMetal { version } => version.clone(),
            GraphicsBackend::DxvkVkd3d {
                dxvk_version,
                vkd3d_version,
            } => {
                format!("dxvk-{}+vkd3d-{}", dxvk_version, vkd3d_version)
            }
        }
    }

    /// DLL override entries: `(dll_name, setting_string)`.
    /// Setting is always `"native,builtin"`.
    pub fn override_entries(&self) -> Vec<(&str, &str)> {
        match self {
            GraphicsBackend::Dxmt { .. } => vec![
                // We don't patch Wine's bundle — all DXMT DLLs go into prefix system32
                // as native overrides, regardless of how DXMT was built.
                ("winemetal", "native,builtin"),
                ("d3d11", "native,builtin"),
                ("dxgi", "native,builtin"),
                ("d3d10core", "native,builtin"),
            ],
            GraphicsBackend::D3DMetal { .. } => vec![
                ("d3d11", "native,builtin"),
                ("d3d12", "native,builtin"),
                ("dxgi", "native,builtin"),
            ],
            GraphicsBackend::DxvkVkd3d { .. } => vec![
                ("d3d8", "native,builtin"),
                ("d3d9", "native,builtin"),
                ("d3d10core", "native,builtin"),
                ("d3d11", "native,builtin"),
                ("dxgi", "native,builtin"),
                ("d3d12", "native,builtin"),
                ("d3d12core", "native,builtin"),
            ],
        }
    }

    /// DLL names only (without settings) — used for symlink cleanup and registry removal.
    pub fn override_dlls(&self) -> Vec<&str> {
        self.override_entries()
            .into_iter()
            .map(|(dll, _)| dll)
            .collect()
    }

    /// Returns the `WINEDLLOVERRIDES` string: `"dll1,dll2,...=native,builtin"`.
    pub fn override_env_string(&self) -> String {
        let dlls: Vec<&str> = self
            .override_entries()
            .iter()
            .map(|(dll, _)| *dll)
            .collect();
        format!("{}=native,builtin", dlls.join(","))
    }

    /// Whether the backend supports 32-bit prefixes.
    pub fn supports_arch(&self, arch: &str) -> bool {
        match self {
            GraphicsBackend::Dxmt { .. } | GraphicsBackend::D3DMetal { .. } => arch == "win64",
            GraphicsBackend::DxvkVkd3d { .. } => true,
        }
    }
}

/// Per-prefix graphics configuration, stored in tequila-config.json.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphicsConfig {
    pub backend: String,
    pub version: String,
}

impl GraphicsConfig {
    /// Reconstruct a `GraphicsBackend` from the stored config (best-effort).
    pub fn to_backend(&self) -> Option<GraphicsBackend> {
        match self.backend.as_str() {
            "dxmt" => Some(GraphicsBackend::Dxmt {
                version: self.version.clone(),
            }),
            "d3dmetal" => Some(GraphicsBackend::D3DMetal {
                version: self.version.clone(),
            }),
            "dxvk-vkd3d" => {
                // version stored as "dxvk-{v}+vkd3d-{v}"
                if let Some((dxvk, vkd3d)) = self.version.split_once('+') {
                    let dxvk = dxvk.strip_prefix("dxvk-").unwrap_or(dxvk).to_string();
                    let vkd3d = vkd3d.strip_prefix("vkd3d-").unwrap_or(vkd3d).to_string();
                    Some(GraphicsBackend::DxvkVkd3d {
                        dxvk_version: dxvk,
                        vkd3d_version: vkd3d,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Returns the display name (e.g. "DXMT", "D3DMetal").
    pub fn display_name(&self) -> &str {
        match self.backend.as_str() {
            "dxmt" => "DXMT",
            "d3dmetal" => "D3DMetal",
            "dxvk-vkd3d" => "DXVK+VKD3D",
            _ => &self.backend,
        }
    }

    /// DLL override keys for deactivation.
    ///
    /// NOTE: Keep in sync with `GraphicsBackend::override_entries()`.
    /// If you add a new DLL override to the enum, add it here too.
    pub fn override_dlls(&self) -> Vec<&str> {
        match self.backend.as_str() {
            "dxmt" => vec!["winemetal", "d3d11", "dxgi", "d3d10core"],
            "d3dmetal" => vec!["d3d11", "d3d12", "dxgi"],
            "dxvk-vkd3d" => vec![
                "d3d8",
                "d3d9",
                "d3d10core",
                "d3d11",
                "dxgi",
                "d3d12",
                "d3d12core",
            ],
            _ => vec![],
        }
    }

    /// Returns the `WINEDLLOVERRIDES` string for this config.
    pub fn override_env_string(&self) -> String {
        let dlls = self.override_dlls();
        format!("{}=native,builtin", dlls.join(","))
    }

    /// Whether the backend identifier is valid.
    pub fn is_valid(&self) -> bool {
        matches!(self.backend.as_str(), "dxmt" | "d3dmetal" | "dxvk-vkd3d")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_labels() {
        let dxmt = GraphicsBackend::Dxmt {
            version: "0.80".to_string(),
        };
        let d3dmetal = GraphicsBackend::D3DMetal {
            version: "1.0".to_string(),
        };
        let dxvk = GraphicsBackend::DxvkVkd3d {
            dxvk_version: "2.4".to_string(),
            vkd3d_version: "1.14".to_string(),
        };
        assert_eq!(dxmt.label(), "dxmt");
        assert_eq!(d3dmetal.label(), "d3dmetal");
        assert_eq!(dxvk.label(), "dxvk-vkd3d");
        assert_eq!(dxmt.display_name(), "DXMT");
        assert_eq!(d3dmetal.display_name(), "D3DMetal");
        assert_eq!(dxvk.display_name(), "DXVK+VKD3D");
    }

    #[test]
    fn version_strings() {
        let dxmt = GraphicsBackend::Dxmt {
            version: "0.80".to_string(),
        };
        let dxvk = GraphicsBackend::DxvkVkd3d {
            dxvk_version: "2.4".to_string(),
            vkd3d_version: "1.14".to_string(),
        };
        assert_eq!(dxmt.version_string(), "0.80");
        assert_eq!(dxvk.version_string(), "dxvk-2.4+vkd3d-1.14");
    }

    #[test]
    fn dxmt_overrides() {
        let dxmt = GraphicsBackend::Dxmt {
            version: "0.80".to_string(),
        };
        let entries = dxmt.override_entries();
        assert_eq!(
            entries,
            vec![
                ("winemetal", "native,builtin"),
                ("d3d11", "native,builtin"),
                ("dxgi", "native,builtin"),
                ("d3d10core", "native,builtin"),
            ]
        );
        assert_eq!(
            dxmt.override_env_string(),
            "winemetal,d3d11,dxgi,d3d10core=native,builtin"
        );
        assert_eq!(
            dxmt.override_dlls(),
            entries.iter().map(|(d, _)| *d).collect::<Vec<_>>()
        );
    }

    #[test]
    fn d3dmetal_overrides() {
        let backend = GraphicsBackend::D3DMetal {
            version: "1.0".to_string(),
        };
        assert_eq!(backend.override_dlls(), vec!["d3d11", "d3d12", "dxgi"]);
    }

    #[test]
    fn dxvk_overrides() {
        let backend = GraphicsBackend::DxvkVkd3d {
            dxvk_version: "2.4".to_string(),
            vkd3d_version: "1.14".to_string(),
        };
        assert_eq!(
            backend.override_dlls(),
            vec![
                "d3d8",
                "d3d9",
                "d3d10core",
                "d3d11",
                "dxgi",
                "d3d12",
                "d3d12core"
            ]
        );
    }

    #[test]
    fn arch_support() {
        let dxmt = GraphicsBackend::Dxmt {
            version: "0.80".to_string(),
        };
        let dxvk = GraphicsBackend::DxvkVkd3d {
            dxvk_version: "2.4".to_string(),
            vkd3d_version: "1.14".to_string(),
        };
        assert!(dxmt.supports_arch("win64"));
        assert!(!dxmt.supports_arch("win32"));
        assert!(dxvk.supports_arch("win32"));
        assert!(dxvk.supports_arch("win64"));
    }

    #[test]
    fn config_to_backend_roundtrip() {
        let dxmt = GraphicsConfig {
            backend: "dxmt".to_string(),
            version: "0.80".to_string(),
        };
        assert_eq!(
            dxmt.to_backend(),
            Some(GraphicsBackend::Dxmt {
                version: "0.80".to_string()
            })
        );

        let d3dmetal = GraphicsConfig {
            backend: "d3dmetal".to_string(),
            version: "1.0".to_string(),
        };
        assert_eq!(
            d3dmetal.to_backend(),
            Some(GraphicsBackend::D3DMetal {
                version: "1.0".to_string()
            })
        );
    }

    #[test]
    fn config_dxvk_version_parsing() {
        let config = GraphicsConfig {
            backend: "dxvk-vkd3d".to_string(),
            version: "dxvk-2.4+vkd3d-1.14".to_string(),
        };
        assert_eq!(
            config.to_backend(),
            Some(GraphicsBackend::DxvkVkd3d {
                dxvk_version: "2.4".to_string(),
                vkd3d_version: "1.14".to_string(),
            })
        );

        let malformed = GraphicsConfig {
            backend: "dxvk-vkd3d".to_string(),
            version: "2.4".to_string(),
        };
        assert!(malformed.to_backend().is_none());
    }

    #[test]
    fn config_unknown_backend() {
        let config = GraphicsConfig {
            backend: "carbide".to_string(),
            version: "1.0".to_string(),
        };
        assert!(config.to_backend().is_none());
        assert!(!config.is_valid());
        assert!(config.override_dlls().is_empty());
        assert_eq!(config.display_name(), "carbide");
    }

    #[test]
    fn config_display_names_and_validity() {
        assert!(
            GraphicsConfig {
                backend: "dxmt".to_string(),
                version: "1".to_string(),
            }
            .is_valid()
        );
        let config = GraphicsConfig {
            backend: "dxvk-vkd3d".to_string(),
            version: "dxvk-2.4+vkd3d-1.14".to_string(),
        };
        assert_eq!(config.display_name(), "DXVK+VKD3D");
        assert_eq!(
            config.override_env_string(),
            "d3d8,d3d9,d3d10core,d3d11,dxgi,d3d12,d3d12core=native,builtin"
        );
    }
}
