use base::error::{PrefixError, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Compute the SHA-256 hex digest of a file.
pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|e| {
        PrefixError::RegistryError(format!("Failed to open file for hashing: {}", e))
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(|e| {
            PrefixError::RegistryError(format!("Failed to read file for hashing: {}", e))
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Compute the SHA-256 hex digests of `user.reg` and `system.reg` inside a prefix.
///
/// Returns `(user_reg_hash, system_reg_hash)`.
/// If a file does not exist its hash is an empty string.
pub fn hash_registry_files(prefix_path: &Path) -> Result<(String, String)> {
    let user_reg = prefix_path.join("user.reg");
    let system_reg = prefix_path.join("system.reg");

    let user_hash = if user_reg.exists() {
        hash_file(&user_reg)?
    } else {
        String::new()
    };

    let system_hash = if system_reg.exists() {
        hash_file(&system_reg)?
    } else {
        String::new()
    };

    Ok((user_hash, system_hash))
}
