use crate::Manager;
use base::config::{PrefixConfig, RegisteredExecutable};
use base::error::Result;
use base::traits::PrefixInfo;
use log::debug;
use std::path::{Path, PathBuf};

impl Manager {
    pub fn scan_for_applications(
        &self,
        prefix_path: &PathBuf,
    ) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        executables.extend(self.scanner.scan_prefix(prefix_path)?);
        executables.extend(self.scanner.scan_for_desktop_files(prefix_path)?);
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        Ok(executables)
    }

    pub async fn scan_for_applications_async(
        &self,
        prefix_path: &PathBuf,
    ) -> Result<Vec<RegisteredExecutable>> {
        let mut executables = Vec::new();
        executables.extend(self.scanner.scan_prefix_async(prefix_path).await?);
        executables.extend(
            self.scanner
                .scan_for_desktop_files_async(prefix_path)
                .await?,
        );
        executables.sort_by(|a, b| a.name.cmp(&b.name));
        executables.dedup_by(|a, b| a.name == b.name && a.executable_path == b.executable_path);
        Ok(executables)
    }

    pub fn update_config(&self, prefix_path: &PathBuf, config: &PrefixConfig) -> Result<()> {
        config.validate()?;
        let mut updated_config = config.clone();
        updated_config.update_last_modified();
        updated_config.save_to_file(prefix_path)?;
        Ok(())
    }

    pub fn add_executable_to_prefix(
        &self,
        prefix_path: &PathBuf,
        executable: RegisteredExecutable,
    ) -> Result<()> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, name, &None)?;
        config.add_executable(executable);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn remove_executable_from_prefix(&self, prefix_path: &PathBuf, index: usize) -> Result<()> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let mut config = self.load_or_create_config(prefix_path, name, &None)?;
        config.remove_executable(index);
        self.update_config(prefix_path, &config)?;
        Ok(())
    }

    pub fn enrich_executables(&self, prefix_path: &Path, config: &mut PrefixConfig) -> bool {
        let ic = self.scanner.icon_cache();
        let mut changed = false;
        for exe in &mut config.registered_executables {
            if let Some(resolved) = resolve_or_extract_icon(exe, prefix_path, ic) {
                if exe.icon_path.as_ref() != Some(&resolved) {
                    exe.icon_path = Some(resolved);
                    changed = true;
                }
            } else if exe.icon_path.is_some() {
                // Previously persisted icon no longer available and extraction
                // failed — drop the stale path so the UI can show its fallback.
                exe.icon_path = None;
                changed = true;
            }
            if exe.file_description.is_none() {
                let meta = scan::extract_metadata_for_exe(&exe.executable_path);
                if meta.file_version.is_some() || meta.file_description.is_some() {
                    exe.file_version = meta.file_version;
                    exe.product_version = meta.product_version;
                    exe.company_name = meta.company_name;
                    exe.file_description = meta.file_description;
                    exe.product_name = meta.product_name;
                    exe.imported_modules = meta.imported_modules;
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn get_prefix_info(&self, prefix_path: &PathBuf) -> Result<PrefixInfo> {
        let name = prefix_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let config = self.load_or_create_config(prefix_path, name, &None)?;
        let size = self.calculate_prefix_size(prefix_path)?;
        Ok(PrefixInfo {
            name: config.name.clone(),
            path: prefix_path.clone(),
            size,
            executable_count: config.get_executable_count(),
            wine_version: config.wine_version.clone(),
            architecture: config.architecture.clone(),
            creation_date: config.creation_date,
            last_modified: config.last_modified,
        })
    }

    fn calculate_prefix_size(&self, prefix_path: &PathBuf) -> Result<u64> {
        let total_size = walkdir::WalkDir::new(prefix_path)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_file())
            .filter_map(|entry| entry.metadata().ok())
            .map(|metadata| metadata.len())
            .sum();
        Ok(total_size)
    }
}

/// Resolve the icon for a registered executable.
///
/// 1. Honour the user-provided `icon_path`:
///    * absolute → use directly,
///    * relative → join with `prefix_path`.
///    Returned only when the resulting file actually exists on disk.
/// 2. Otherwise fall back to extracting an icon from the executable itself
///    (using the shared `IconCache`).
pub fn resolve_or_extract_icon(
    exe: &RegisteredExecutable,
    prefix_path: &Path,
    icon_cache: &scan::IconCache,
) -> Option<PathBuf> {
    if let Some(resolved) = exe.resolve_icon_path(prefix_path) {
        debug!(
            "[apps] Using configured icon for '{}': {}",
            exe.name,
            resolved.display()
        );
        return Some(resolved);
    }
    if exe.icon_path.is_some() {
        debug!(
            "[apps] Configured icon for '{}' is missing, attempting extraction",
            exe.name
        );
    }
    scan::extract_icon_for_exe(&exe.executable_path, icon_cache)
}
