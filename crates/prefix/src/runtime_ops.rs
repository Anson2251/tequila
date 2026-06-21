use crate::Manager;
use base::config::PrefixConfig;
use runtime::Runtime;
use std::path::PathBuf;

impl Manager {
    pub fn save_runtime_state(&self) {
        let settings: store::Settings = self.clone_runtime().into();
        if let Err(e) = settings.save() {
            log::error!("[runtime] failed to save runtime settings: {}", e);
        }
    }

    pub fn import_runtime(
        &self,
        source_path: &PathBuf,
        label: &str,
    ) -> std::result::Result<Runtime, String> {
        let runtimes = runtime::download::runtimes_dir();
        let runtime = self
            .write_runtime()
            .import_runtime(source_path, label, &runtimes)?;
        self.save_runtime_state();
        Ok(runtime)
    }

    pub fn remove_runtime(&self, id: &str) {
        self.write_runtime().remove(id);
        self.save_runtime_state();
    }

    pub fn set_default_runtime(&self, id: &str) {
        self.write_runtime().set_default(id);
        self.save_runtime_state();
    }

    pub(crate) fn runtime_for_prefix(&self, config: &PrefixConfig) -> Option<Runtime> {
        self.read_runtime()
            .resolve(config.wine_version.as_deref())
            .cloned()
    }
}
