//! GitHub Releases API client.
//!
//! Provides [`GitHubClient`] — a builder-style client that holds authentication
//! once and can fetch releases for any public repository. Used by [`kron4ek`](crate::kron4ek),
//! [`anson2251`](crate::anson2251), and [`graphics`](crate::graphics).

use base::error::{PrefixError, Result};
use serde::Deserialize;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

// ── Progress callback ──────────────────────────────────────────────────

/// Download progress callback: `(downloaded_bytes, total_bytes)`.
///
/// This is intentionally kept simple so that [`GitHubClient`] does not
/// depend on higher-level install-phase concepts.
pub type DownloadProgress = Box<dyn Fn(u64, u64) + Send>;

// ── Response types ─────────────────────────────────────────────────────

/// A release fetched from the GitHub Releases API.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    /// Tag name, e.g. `"11.9"` or `"v26.2.0"`.
    pub tag_name: String,
    /// Whether this is a pre-release (GitHub pre-release flag).
    #[serde(default)]
    pub prerelease: bool,
    /// Assets attached to this release.
    pub assets: Vec<GitHubAsset>,
}

/// A single asset in a GitHub release.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubAsset {
    /// Asset filename, e.g. `"wine-11.9-amd64.tar.xz"`.
    pub name: String,
    /// Direct download URL for the asset.
    pub browser_download_url: String,
    /// SHA-256 digest string, e.g. `"sha256:a296d3f6…"`.
    /// Only present when the uploader attached a digest to the asset.
    #[serde(default)]
    pub digest: Option<String>,
}

// ── Client ─────────────────────────────────────────────────────────────

/// A GitHub API client that holds authentication and can fetch release info.
///
/// Build once with an optional API key, then reuse across multiple
/// `fetch_*` calls without passing the key each time.
#[derive(Debug, Clone)]
pub struct GitHubClient {
    api_key: Option<String>,
}

impl GitHubClient {
    /// Create a new client.
    ///
    /// `api_key` — optional GitHub Personal Access Token to raise the
    /// rate limit from ~60 to 5000 req/h.
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }

    /// Fetch the **latest** release for a GitHub repository.
    pub async fn fetch_latest_release(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<GitHubRelease> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            owner, repo,
        );
        let response = self.github_api_get(&url).await?;
        parse_response(repo, response).await
    }

    /// Fetch **all** releases for a GitHub repository (paginated, newest first).
    ///
    /// `per_page` defaults to 30 when `None`.
    pub async fn fetch_all_releases(
        &self,
        owner: &str,
        repo: &str,
        per_page: Option<u32>,
    ) -> Result<Vec<GitHubRelease>> {
        let pp = per_page.unwrap_or(30);
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases?per_page={}",
            owner, repo, pp,
        );
        let response = self.github_api_get(&url).await?;
        parse_response(repo, response).await
    }

    /// Low-level GET helper.  Each call builds a fresh `reqwest` client
    /// so we don't keep a long-lived HTTP connection, but the builder
    /// overhead is negligible compared to a network round-trip.
    async fn github_api_get(&self, url: &str) -> Result<reqwest::Response> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|e| PrefixError::Process(format!("Failed to build HTTP client: {}", e)))?;

        let mut req = client.get(url);
        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        req.send().await.map_err(|e| {
            PrefixError::Process(format!(
                "Network error: {}. \
                 Please check your internet connection or VPN/proxy settings.",
                e,
            ))
        })
    }

    /// Download an asset file to `dest_dir` and return the local path.
    ///
    /// 1. **Download** — streams the asset to a file inside `dest_dir`,
    ///    reporting progress via [`DownloadProgress`].  The filename is
    ///    taken from [`GitHubAsset::name`].
    /// 2. **SHA-256 verification** — if [`GitHubAsset::digest`] contains
    ///    a valid `"sha256:…"` hex string (64 hex chars), the file is
    ///    verified after the download completes.  **If no valid digest is
    ///    present, the download proceeds without verification.**
    ///
    /// The caller is responsible for cleaning up `dest_dir` after use.
    pub async fn download_asset(
        &self,
        asset: &GitHubAsset,
        dest_dir: &Path,
        progress: &DownloadProgress,
        cancel: &AtomicBool,
    ) -> Result<std::path::PathBuf> {
        use crate::download::verify_sha256;

        std::fs::create_dir_all(dest_dir)?;
        let archive_path = dest_dir.join(&asset.name);

        // Download
        progress(0, 0);
        let mut response = self.github_api_get(&asset.browser_download_url).await?;
        let total = response.content_length().unwrap_or(0);
        let mut file = std::fs::File::create(&archive_path)?;
        let mut downloaded: u64 = 0;
        loop {
            if cancel.load(Ordering::Relaxed) {
                drop(file);
                let _ = std::fs::remove_file(&archive_path);
                return Err(PrefixError::Process("Download cancelled".into()));
            }
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    file.write_all(&chunk)?;
                    downloaded += chunk.len() as u64;
                    progress(downloaded, total);
                }
                Ok(None) => break,
                Err(e) => {
                    return Err(PrefixError::Process(format!("Download error: {}", e)));
                }
            }
        }
        file.flush()?;

        // Verify (if a valid SHA-256 digest is available)
        if let Some(hash) = asset
            .digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
            .and_then(|h| {
                let h = h.trim();
                if h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()) {
                    Some(h)
                } else {
                    None
                }
            })
        {
            verify_sha256(&archive_path, hash)?;
        }

        Ok(archive_path)
    }
}

// ── Internal helpers ───────────────────────────────────────────────────

/// User-agent sent with every GitHub API request.
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Check the HTTP status; on success deserialize JSON, on error consume
/// the body for a descriptive message.
async fn parse_response<T>(repo: &str, response: reqwest::Response) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    if status.is_success() {
        return response.json::<T>().await.map_err(|e| {
            PrefixError::Process(format!(
                "Failed to parse {} release data: {}. \
                 If this persists, the release format may have changed.",
                repo, e,
            ))
        });
    }

    let body_text = response.text().await.unwrap_or_default();
    let msg = serde_json::from_str::<serde_json::Value>(&body_text)
        .ok()
        .and_then(|v| v.get("message")?.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| format!("HTTP {}", status));

    Err(PrefixError::Process(format!(
        "Failed to fetch {} release information: {}\n\n\
         If you are using a VPN or proxy, try switching to a different node \
         or disabling it temporarily, as shared IPs are often rate-limited \
         by GitHub.",
        repo, msg,
    )))
}
