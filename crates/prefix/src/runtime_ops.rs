use crate::Manager;
use base::config::PrefixConfig;
use base::error::{PrefixError, Result};
use runtime::{Channel, Runtime};
use std::path::PathBuf;

impl Manager {
    pub fn save_runtime_state(&self) {
        let settings: store::Settings = self.runtime_manager.clone().into();
        if let Err(e) = settings.save() {
            log::error!("[runtime] failed to save runtime settings: {}", e);
        }
    }

    pub async fn download_channel_runtime(
        &mut self,
        channel: Channel,
        progress: runtime::download::ProgressFn,
    ) -> Result<Runtime> {
        let runtimes = runtime::download::runtimes_dir();
        runtime::download::cleanup_temp_runtimes(&runtimes);
        let bundle_dir = runtime::download::download_channel_runtime(&channel, &progress).await?;
        let cask = runtime::homebrew::fetch_cask(channel.cask_name())
            .await
            .map_err(|e| PrefixError::Process(e))?;
        let runtime = self
            .runtime_manager
            .register_channel(channel, cask.version, bundle_dir)
            .clone();
        self.save_runtime_state();
        Ok(runtime)
    }

    pub fn import_runtime(
        &mut self,
        source_path: &PathBuf,
        label: &str,
    ) -> std::result::Result<Runtime, String> {
        let runtimes = runtime::download::runtimes_dir();
        let runtime = self
            .runtime_manager
            .import_runtime(source_path, label, &runtimes)?;
        self.save_runtime_state();
        Ok(runtime)
    }

    pub fn remove_runtime(&mut self, id: &str) {
        self.runtime_manager.remove(id);
        self.save_runtime_state();
    }

    pub fn set_default_runtime(&mut self, id: &str) {
        self.runtime_manager.set_default(id);
        self.save_runtime_state();
    }

    pub(crate) fn runtime_for_prefix(&self, config: &PrefixConfig) -> Option<&Runtime> {
        self.runtime_manager.resolve(config.wine_version.as_deref())
    }
}
