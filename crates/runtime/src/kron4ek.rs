//! Fetch Wine builds from [Kron4ek/Wine-Builds](https://github.com/Kron4ek/Wine-Builds)
//! using the GitHub Releases API.
//!
//! Unlike the Homebrew-based approach (which only knows about the 3 latest channels
//! on macOS), this module lists **all** available Wine versions published by Kron4ek
//! and picks the correct architecture asset for the current system.

use base::error::{PrefixError, Result};

/// A single Wine build published by Kron4ek.
#[derive(Debug, Clone)]
pub struct WineBuild {
    /// Version identifier, e.g. `"11.9"` or `"11.9-staging"`.
    pub version: String,
    /// Whether this is a Staging build.
    pub is_staging: bool,
    /// Direct download URL for the archive.
    pub archive_url: String,
    /// Archive filename.
    pub archive_name: String,
}

/// Return the arch string used in Kron4ek asset filenames for the current system.
///
/// | `uname -m`      | Kron4ek suffix | Supported? |
/// |-----------------|----------------|------------|
/// | `x86_64`        | `amd64`        | ✅         |
/// | `aarch64`       | *(none)*       | ❌         |
/// | `i686` / `x86`  | `x86`          | ⚠️ (32-bit only) |
pub fn system_arch_suffix() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("amd64"),
        "x86" => Some("x86"),
        _ => None,
    }
}

/// Fetch **all** available Wine builds for the current system architecture.
///
/// Calls the GitHub Releases API (`per_page=100`) and parses every release
/// that carries usable assets. Both vanilla and Staging builds are returned,
/// sorted with vanilla first and newest versions on top.
///
/// * `api_key` — optional GitHub Personal Access Token to avoid rate-limiting.
pub async fn fetch_all_builds(client: &crate::github::GitHubClient) -> Result<Vec<WineBuild>> {
    let arch_suffix = system_arch_suffix().ok_or_else(|| {
        PrefixError::Process(format!(
            "Unsupported CPU architecture '{}'. Kron4ek/Wine-Builds only provides \
             amd64 and x86 builds.",
            std::env::consts::ARCH
        ))
    })?;

    let releases = client.fetch_all_releases("Kron4ek", "Wine-Builds", Some(100)).await?;

    let mut builds = Vec::new();

    for release in releases {
        let ver = &release.tag_name;

        // Skip non-version tags (e.g. proton-* tags)
        if !ver.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            continue;
        }

        // ── Vanilla build ──────────────────────────────────────────
        let vanilla_name = format!("wine-{}-{}.tar.xz", ver, arch_suffix);
        if let Some(asset) = release.assets.iter().find(|a| a.name == vanilla_name) {
            builds.push(WineBuild {
                version: ver.clone(),
                is_staging: false,
                archive_url: asset.browser_download_url.clone(),
                archive_name: asset.name.clone(),
            });
        }

        // ── Staging build ──────────────────────────────────────────
        let staging_name = format!("wine-{}-staging-{}.tar.xz", ver, arch_suffix);
        if let Some(asset) = release.assets.iter().find(|a| a.name == staging_name) {
            builds.push(WineBuild {
                version: format!("{}-staging", ver),
                is_staging: true,
                archive_url: asset.browser_download_url.clone(),
                archive_name: asset.name.clone(),
            });
        }
    }

    // Sort by base version descending, vanilla before staging for each version.
    // Result: 11.9, 11.9-staging, 11.8, 11.8-staging, …
    builds.sort_by(|a, b| {
        let a_base = a.version.trim_end_matches("-staging");
        let b_base = b.version.trim_end_matches("-staging");
        match compare_versions_desc(a_base, b_base) {
            std::cmp::Ordering::Equal => {
                // Same base version: vanilla before staging
                (a.is_staging as u8).cmp(&(b.is_staging as u8))
            }
            other => other,
        }
    });

    Ok(builds)
}

/// Compare dotted version strings in descending order (newest first).
fn compare_versions_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<&str> = a.split('.').collect();
    let b_parts: Vec<&str> = b.split('.').collect();
    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        let a_num = a_part.parse::<u32>().unwrap_or(0);
        let b_num = b_part.parse::<u32>().unwrap_or(0);
        if a_num != b_num {
            return b_num.cmp(&a_num);
        }
    }
    b_parts.len().cmp(&a_parts.len())
}
