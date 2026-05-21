use serde::{Deserialize, Serialize};

/// Information about a Homebrew cask, fetched from the Cask API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaskInfo {
    pub url: String,
    pub version: String,
    pub sha256: String,
}

/// Fetch cask metadata from the Homebrew Cask API.
///
/// Fetches `https://formulae.brew.sh/api/cask/{cask-name}.json` and extracts
/// the url, version, and sha256 needed for download and update checks.
pub async fn fetch_cask(cask_name: &str) -> Result<CaskInfo, String> {
    let url = format!("https://formulae.brew.sh/api/cask/{}.json", cask_name);
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to fetch cask info for {}: {}", cask_name, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Cask API returned status {} for {}",
            response.status(),
            cask_name
        ));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse cask JSON for {}: {}", cask_name, e))?;

    let url = json["url"]
        .as_str()
        .ok_or_else(|| format!("Missing 'url' in cask JSON for {}", cask_name))?
        .to_string();
    let version = json["version"]
        .as_str()
        .ok_or_else(|| format!("Missing 'version' in cask JSON for {}", cask_name))?
        .to_string();
    let sha256 = json["sha256"]
        .as_str()
        .ok_or_else(|| format!("Missing 'sha256' in cask JSON for {}", cask_name))?
        .to_string();

    Ok(CaskInfo { url, version, sha256 })
}

/// Check whether a newer version is available for a managed channel runtime.
/// Returns Some(CaskInfo) if an update is available, None if the installed
/// version matches the latest.
pub async fn check_update(
    cask_name: &str,
    installed_version: &str,
) -> Result<Option<CaskInfo>, String> {
    let info = fetch_cask(cask_name).await?;
    if info.version != installed_version {
        Ok(Some(info))
    } else {
        Ok(None)
    }
}
