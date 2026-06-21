use base::error::{PrefixError, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn runtimes_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tequila")
        .join("runtimes")
}

pub type ProgressFn = Box<dyn Fn(u64, u64) + Send>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstallPhase {
    Download,
    Verify,
    Extract,
}

pub type PhaseProgressFn = Box<dyn Fn(u64, u64, InstallPhase) + Send>;

pub async fn download_file(url: &str, dest: &Path, progress: &ProgressFn) -> Result<()> {
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
            Err(e) => return Err(PrefixError::Process(format!("Download error: {}", e))),
        }
    }
    file.flush()?;
    Ok(())
}

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

pub fn find_wine_binary(dir: &Path) -> Result<PathBuf> {
    for entry in walkdir::WalkDir::new(dir)
        .max_depth(6)
        .into_iter()
        .flatten()
    {
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

/// Kron4ek archives often contain a single top-level directory (e.g.
/// `wine-11.8-amd64/`).  After extracting into `dest_dir`, call this to
/// "un-nest" that directory so the rest of the pipeline sees a flat layout.
///
/// If `dest_dir` contains exactly one subdirectory and no loose files,
/// it returns the path of that subdirectory.  Otherwise it returns `dest_dir`
/// as-is (no nesting to flatten).
pub fn find_content_dir(dest_dir: &Path) -> Result<PathBuf> {
    let mut entries = Vec::new();
    if let Ok(read) = fs::read_dir(dest_dir) {
        for entry in read.flatten() {
            entries.push(entry);
        }
    }

    // If there's exactly one entry and it's a directory, use it as the content root
    if entries.len() == 1 {
        if let Ok(ftype) = entries[0].file_type() {
            if ftype.is_dir() {
                return Ok(entries[0].path());
            }
        }
    }

    // Otherwise assume flat extraction (like Homebrew casks)
    Ok(dest_dir.to_path_buf())
}

pub fn bundle_dir_from_wine_bin(wine_bin: &Path) -> PathBuf {
    wine_bin
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| {
            wine_bin
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| wine_bin.to_path_buf())
        })
}

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
            }
            if name_str.starts_with(".lock-") && entry.path().is_file() {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
}

/// Download and install a Kron4ek Wine build with phase progress.
///
/// Reports InstallPhase::Download and Extract progress.
pub async fn install_kron4ek_build(
    version: &str,
    archive_url: &str,
    archive_name: &str,
    progress: &PhaseProgressFn,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    let runtimes = runtimes_dir();
    fs::create_dir_all(&runtimes)?;
    let runtime_id = format!("wine-{}", version);
    cleanup_temp_runtimes(&runtimes);
    let tmp_dir = runtimes.join(format!(".tmp-{}", runtime_id));
    let final_dir = runtimes.join(&runtime_id);
    let _lock = LockGuard::acquire(&runtimes, &runtime_id)?;
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;
    let archive_path = tmp_dir.join(archive_name);

    // Download
    let mut response = reqwest::get(archive_url)
        .await
        .map_err(|e| PrefixError::Process(format!("Download failed: {}", e)))?;
    let total = response.content_length().unwrap_or(0);
    let mut file = fs::File::create(&archive_path)?;
    let mut downloaded: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err(PrefixError::Process("Download cancelled".into()));
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                file.write_all(&chunk)?;
                downloaded += chunk.len() as u64;
                progress(downloaded, total, InstallPhase::Download);
            }
            Ok(None) => break,
            Err(e) => return Err(PrefixError::Process(format!("Download error: {}", e))),
        }
    }
    file.flush()?;

    // Extract
    progress(0, 1, InstallPhase::Extract);
    extract_tar(&archive_path, &tmp_dir)?;
    // Remove archive so find_content_dir only sees extracted content
    let _ = fs::remove_file(&archive_path);

    // Resolve content root & find wine binary
    let content_dir = find_content_dir(&tmp_dir)?;
    let _ = find_wine_binary(&content_dir)?;

    if final_dir.exists() {
        fs::remove_dir_all(&final_dir)?;
    }
    if content_dir != tmp_dir {
        fs::rename(&content_dir, &final_dir)?;
        let _ = fs::remove_dir_all(&tmp_dir);
    } else {
        fs::rename(&tmp_dir, &final_dir)?;
    }
    progress(1, 1, InstallPhase::Extract);

    Ok(final_dir)
}

/// Download and install a crossover-foss build with phase progress.
///
/// Reports InstallPhase::Download, Verify (if sha256 is non-empty), and Extract progress.
pub async fn install_crossover_build(
    version: &str,
    archive_url: &str,
    sha256: &str,
    progress: &PhaseProgressFn,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    let runtimes = runtimes_dir();
    fs::create_dir_all(&runtimes)?;
    let runtime_id = format!("crossover-{}", version);
    cleanup_temp_runtimes(&runtimes);
    let tmp_dir = runtimes.join(format!(".tmp-{}", runtime_id));
    let final_dir = runtimes.join(&runtime_id);
    let _lock = LockGuard::acquire(&runtimes, &runtime_id)?;
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;

    let archive_name = archive_url.rsplit('/').next().unwrap_or("archive.tar.zst");
    let archive_path = tmp_dir.join(archive_name);

    // Download
    progress(0, 0, InstallPhase::Download);
    let mut response = reqwest::get(archive_url)
        .await
        .map_err(|e| PrefixError::Process(format!("Download failed: {}", e)))?;
    let total = response.content_length().unwrap_or(0);
    let mut file = fs::File::create(&archive_path)?;
    let mut downloaded: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = fs::remove_dir_all(&tmp_dir);
            return Err(PrefixError::Process("Download cancelled".into()));
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                file.write_all(&chunk)?;
                downloaded += chunk.len() as u64;
                progress(downloaded, total, InstallPhase::Download);
            }
            Ok(None) => break,
            Err(e) => return Err(PrefixError::Process(format!("Download error: {}", e))),
        }
    }
    file.flush()?;

    // Verify (optional — only if a non-empty sha256 was provided)
    if !sha256.is_empty() {
        progress(0, 1, InstallPhase::Verify);
        verify_sha256(&archive_path, sha256)?;
        progress(1, 1, InstallPhase::Verify);
    }

    // Extract (zst-compressed tar)
    progress(0, 1, InstallPhase::Extract);
    extract_tar_zst(&archive_path, &tmp_dir)?;
    let _ = fs::remove_file(&archive_path);

    // Resolve content root & find wine binary
    let content_dir = find_content_dir(&tmp_dir)?;
    let _ = find_wine_binary(&content_dir)?;

    if final_dir.exists() {
        fs::remove_dir_all(&final_dir)?;
    }
    if content_dir != tmp_dir {
        fs::rename(&content_dir, &final_dir)?;
        let _ = fs::remove_dir_all(&tmp_dir);
    } else {
        fs::rename(&tmp_dir, &final_dir)?;
    }
    progress(1, 1, InstallPhase::Extract);

    Ok(final_dir)
}

/// Extract a pre-downloaded crossover-foss archive and install it as a runtime.
///
/// This function assumes the archive has already been downloaded and its
/// SHA-256 verified (e.g. via [`crate::github::GitHubClient::download_asset`]).
pub fn extract_crossover_build(version: &str, archive_path: &Path) -> Result<PathBuf> {
    let runtimes = runtimes_dir();
    fs::create_dir_all(&runtimes)?;
    let runtime_id = format!("anson2251-{}", version);
    cleanup_temp_runtimes(&runtimes);
    let tmp_dir = runtimes.join(format!(".tmp-{}", runtime_id));
    let final_dir = runtimes.join(&runtime_id);
    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
    }
    fs::create_dir_all(&tmp_dir)?;

    // Extract (zst-compressed tar)
    extract_tar_zst(archive_path, &tmp_dir)?;

    // Resolve content root & find wine binary
    let content_dir = find_content_dir(&tmp_dir)?;
    let _ = find_wine_binary(&content_dir)?;

    if final_dir.exists() {
        fs::remove_dir_all(&final_dir)?;
    }
    if content_dir != tmp_dir {
        fs::rename(&content_dir, &final_dir)?;
        let _ = fs::remove_dir_all(&tmp_dir);
    } else {
        fs::rename(&tmp_dir, &final_dir)?;
    }

    Ok(final_dir)
}
