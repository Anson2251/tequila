use crate::Manager;
use base::config::PrefixConfig;
use base::error::Result;
use std::path::Path;

impl Manager {
    /// Open a prefix and scan it for installed applications.
    pub fn scan_for_applications(
        &self,
        prefix_path: &Path,
    ) -> Result<Vec<base::RegisteredExecutable>> {
        let prefix = self.open_prefix(prefix_path)?;
        prefix.scan_applications()
    }

    /// Update and persist a prefix's configuration.
    pub fn update_config(&self, prefix_path: &Path, config: &PrefixConfig) -> Result<()> {
        config.validate()?;
        let mut updated_config = config.clone();
        updated_config.update_last_modified();
        updated_config.save_to_file(prefix_path)?;
        Ok(())
    }
}
