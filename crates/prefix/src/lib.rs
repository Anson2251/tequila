mod app_ops;
mod launch_ops;
mod manager;
mod prefix_ops;
mod process_tracker;
mod runtime_ops;
mod wine_processes;

pub use manager::Manager;
pub use process_tracker::ProcessTracker;
pub use wine_processes::{WineProcesses, apply_runtime_env};

// Re-exports from sub-crates for UI convenience
pub use base::config;
pub use base::{
    self, GraphicsBackend, GraphicsConfig, PrefixConfig, PrefixError, PrefixInfo,
    RegisteredExecutable, WinePrefix,
};
pub use registry;
pub use registry::keys;
pub use registry::{InMemoryRegistryCache, RegEditor, RegistryCache, RegistryEditor, WineRegistry};
pub use runtime;
pub use runtime::download;
pub use runtime::{Channel, Runtime, RuntimeManager, RuntimeSource};
pub use scan::{ApplicationScanner, IconCache};
pub use store::{PrefixStore, Settings};
