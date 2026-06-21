//! Fetch Wine builds from
//! [Anson2251/crossover-foss-build](https://github.com/Anson2251/crossover-foss-build)
//! using the GitHub Releases API.
//!
//! These builds are pre-packaged Wine runtimes based on CodeWeavers' CrossOver
//! with optional DXMT integration, distributed as `.tar.zst` archives.

use base::error::{PrefixError, Result};

/// A release from Anson2251/crossover-foss-build that the UI can display as a download row.
///
/// The caller can use [`crate::github::GitHubClient::download_asset`] with
/// [`Self::asset`] to download + verify the archive.
#[derive(Debug, Clone)]
pub struct Anson2251Release {
    /// Crossover version (e.g. "26.2.0")
    pub version: String,
    /// DXMT version (e.g. "0.80")
    pub dxmt_version: String,
    /// The matching asset from GitHub (carries the download URL and optional digest).
    pub asset: crate::github::GitHubAsset,
}

/// Fetch the latest release from Anson2251/crossover-foss-build on GitHub.
///
/// Looks for an asset matching `with-dxmt-*-osx64.tar.zst` and extracts
/// version info from the asset name.  SHA-256 verification is handled by
/// [`crate::github::GitHubClient::download_asset`] at download time.
pub async fn fetch_latest_release(client: &crate::github::GitHubClient) -> Result<Anson2251Release> {
    let release = client
        .fetch_latest_release("Anson2251", "crossover-foss-build")
        .await?;

    // Find the with-dxmt macOS asset
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name.contains("with-dxmt") && a.name.ends_with("-osx64.tar.zst"))
        .ok_or_else(|| {
            PrefixError::NotFound(
                "No crossover-foss build with DXMT found in the latest release. \
                 Make sure the repository has a release with a \
                 'with-dxmt-*-osx64.tar.zst' asset."
                    .to_string(),
            )
        })?;

    // Parse version & dxmt_version from asset name
    let (version, dxmt_version) = parse_asset_name(&asset.name).ok_or_else(|| {
        PrefixError::Process(format!(
            "Could not parse version from asset name: {}",
            asset.name
        ))
    })?;

    Ok(Anson2251Release {
        version,
        dxmt_version,
        asset,
    })
}

/// Parse version and dxmt_version from an Anson2251 crossover-foss asset filename.
///
/// # Example
///
/// ```ignore
/// "crossover-foss-26.2.0-with-dxmt-0.80-osx64.tar.zst"
///   -> Some(("26.2.0", "0.80"))
/// ```
pub(crate) fn parse_asset_name(name: &str) -> Option<(String, String)> {
    let stem = name.strip_suffix("-osx64.tar.zst")?;
    // stem is "crossover-foss-{version}-with-dxmt-{dxmt}"
    let mut parts = stem.splitn(3, '-');
    let _crossover = parts.next()?;
    let _foss = parts.next()?;
    let rest = parts.next()?; // "{version}-with-dxmt-{dxmt}"

    let mut split = rest.splitn(2, "-with-dxmt-");
    let crossover_ver = split.next()?.to_string();
    let dxmt_ver = split.next()?.to_string();

    Some((crossover_ver, dxmt_ver))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_asset_name() {
        let (ver, dxmt) =
            parse_asset_name("crossover-foss-26.2.0-with-dxmt-0.80-osx64.tar.zst")
                .expect("should parse");
        assert_eq!(ver, "26.2.0");
        assert_eq!(dxmt, "0.80");
    }
}
