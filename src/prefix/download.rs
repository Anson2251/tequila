use sha2::{Sha256, Digest};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::prefix::error::{Result, PrefixError};
use crate::prefix::homebrew::CaskInfo;
use crate::prefix::runtime::Channel;

/// Directory for runtime downloads under the data dir.
pub fn runtimes_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tequila")
        .join("runtimes")
}

/// Callback type for download progress: (bytes_downloaded, total_bytes).
pub type ProgressFn = Box<dyn Fn(u64, u64) + Send>;

/// Download a file from `url` to `dest`, calling `progress` with byte counts.
pub async fn download_file(
    url: &str,
    dest: &Path,
    progress: &ProgressFn,
) -> Result<()> {
    let mut response = reqwest::get(url)
        .await
        .map_err(|e| PrefixError::Process(format!("Download failed: {}", e)))?;

    let total = response.content_length().unwrap_or(0);

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(dest)?;
    let mut downloaded: u64 = 0;

    loop {
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
    Ok(())
}

/// Verify that `path` has the expected SHA256 hex digest.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<()> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let actual = hex::encode(hasher.finalize());

    if actual != expected_hex {
        return Err(PrefixError::Validation(format!(
            "SHA256 mismatch: expected {}, got {}",
            expected_hex, actual
        )));
    }
    Ok(())
}

/// Extract a tar archive (tar.xz or tar.gz) into `dest_dir` using system tar.
pub fn extract_tar(archive: &Path, dest_dir: &Path) -> Result<()> {
    fs::create_dir_all(dest_dir)?;

    let status = Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest_dir)
        .status()
        .map_err(|e| PrefixError::Process(format!("Failed to run tar: {}", e)))?;

    if !status.success() {
        return Err(PrefixError::Process("tar extraction failed".to_string()));
    }
    Ok(())
}

/// Extract a `.tar.zst` archive.
/// Uses the `zstd` crate for decompression (no system zstd binary needed), then pipes to `tar`.
pub fn extract_tar_zst(archive: &Path, dest_dir: &Path) -> Result<()> {
    fs::create_dir_all(dest_dir)?;

    let data = fs::read(archive)?;
    let decompressed = zstd::decode_all(&data[..])
        .map_err(|e| PrefixError::Process(format!("zstd decompression failed: {}", e)))?;

    let mut child = Command::new("tar")
        .arg("-xf")
        .arg("-")
        .arg("-C")
        .arg(dest_dir)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| PrefixError::Process(format!("Failed to run tar: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(&decompressed)?;
    }

    let status = child
        .wait()
        .map_err(|e| PrefixError::Process(format!("tar wait failed: {}", e)))?;

    if !status.success() {
        return Err(PrefixError::Process(
            "tar extraction failed after zstd decompression".to_string(),
        ));
    }
    Ok(())
}

/// Search `dir` recursively for a `bin/wine` executable.
/// Handles nested directories (e.g. tarballs that extract into a subdirectory).
pub fn find_wine_binary(dir: &Path) -> Result<PathBuf> {
    for entry in walkdir::WalkDir::new(dir).max_depth(5).into_iter().flatten() {
        if entry.file_type().is_file() && entry.file_name() == "wine" {
            let parent = entry.path().parent().unwrap();
            if parent.file_name().map(|n| n == "bin").unwrap_or(false) {
                return Ok(entry.path().to_path_buf());
            }
        }
    }
    Err(PrefixError::NotFound(
        "Could not find bin/wine in extracted archive".to_string(),
    ))
}

/// Determine the bundle_dir from a wine binary path.
/// bundle_dir is the parent of the `bin/` directory that contains wine.
pub fn bundle_dir_from_wine_bin(wine_bin: &Path) -> PathBuf {
    wine_bin
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| wine_bin.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| wine_bin.to_path_buf()))
}

/// Acquire a file lock for a runtime id to prevent concurrent operations.
/// Returns a guard that removes the lock on drop.
pub struct LockGuard {
    lock_path: PathBuf,
}

impl LockGuard {
    pub fn acquire(runtimes_dir: &Path, id: &str) -> Result<Self> {
        let lock_path = runtimes_dir.join(format!(".lock-{}", id));
        if lock_path.exists() {
            return Err(PrefixError::AlreadyExists(format!(
                "Runtime '{}' is already being downloaded or modified",
                id
            )));
        }
        fs::write(&lock_path, &std::process::id().to_string())?;
        Ok(LockGuard { lock_path })
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

/// Full download flow for a macOS channel runtime:
/// fetch cask → download → verify → extract → rename to final dir.
pub async fn download_channel_runtime(
    channel: &Channel,
    progress: &ProgressFn,
) -> Result<PathBuf> {
    let cask = crate::prefix::homebrew::fetch_cask(channel.cask_name())
        .await
        .map_err(|e| PrefixError::Process(e))?;

    let runtimes = runtimes_dir();
    let runtime_id = channel.runtime_id().to_string();
    let tmp_dir = runtimes.join(format!(".tmp-{}", runtime_id));
    let final_dir = runtimes.join(&runtime_id);

    let _lock = LockGuard::acquire(&runtimes, &runtime_id)?;

    // Clean stale tmp
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;

    let archive_path = tmp_dir.join("wine.tar.xz");

    // Download
    download_file(&cask.url, &archive_path, progress).await?;

    // Verify
    verify_sha256(&archive_path, &cask.sha256)?;

    // Extract
    extract_tar(&archive_path, &tmp_dir)?;

    // Find wine binary
    let wine_bin = find_wine_binary(&tmp_dir)?;
    let _bundle_dir = bundle_dir_from_wine_bin(&wine_bin);

    // Remove archive before rename
    let _ = fs::remove_file(&archive_path);

    // Atomic rename
    if final_dir.exists() {
        fs::remove_dir_all(&final_dir)?;
    }
    fs::rename(&tmp_dir, &final_dir)?;

    Ok(final_dir)
}

/// Download and extract GStreamer runtime (macOS only).
pub async fn download_gstreamer(
    data_dir: &Path,
    progress: &ProgressFn,
) -> Result<PathBuf> {
    let gst_cask = crate::prefix::homebrew::fetch_cask("gstreamer-runtime")
        .await
        .map_err(|e| PrefixError::Process(e))?;

    let gst_dir = data_dir.join("runtimes").join("gstreamer");
    let tmp_dir = data_dir.join("runtimes").join(".tmp-gstreamer");

    // Clean stale tmp
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;

    let pkg_path = tmp_dir.join("gstreamer.pkg");

    // Download pkg
    download_file(&gst_cask.url, &pkg_path, progress).await?;

    // Verify
    verify_sha256(&pkg_path, &gst_cask.sha256)?;

    // Extract using the bundled script
    let script = include_str!("../../scripts/extract-gstreamer-pkg.sh");
    let script_path = tmp_dir.join("extract.sh");
    fs::write(&script_path, script)?;

    let status = Command::new("bash")
        .arg(&script_path)
        .arg("--force")
        .arg(&pkg_path)
        .arg(&tmp_dir)
        .status()
        .map_err(|e| PrefixError::Process(format!("Failed to run extract script: {}", e)))?;

    if !status.success() {
        return Err(PrefixError::Process(
            "GStreamer extraction failed".to_string(),
        ));
    }

    // Atomic rename
    if gst_dir.exists() {
        fs::remove_dir_all(&gst_dir)?;
    }
    fs::rename(&tmp_dir, &gst_dir)?;

    Ok(gst_dir)
}

/// Remove stale `.tmp-*` directories from the runtimes directory.
pub fn cleanup_temp_runtimes(runtimes_dir: &Path) {
    if !runtimes_dir.is_dir() {
        return;
    }
    if let Ok(entries) = fs::read_dir(runtimes_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(".tmp-") && entry.path().is_dir() {
                let _ = fs::remove_dir_all(entry.path());
                eprintln!("Cleaned up stale temp dir: {}", name_str);
            }
            // Also clean stale lock files
            if name_str.starts_with(".lock-") && entry.path().is_file() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}
