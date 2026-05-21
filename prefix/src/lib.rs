mod manager;
mod prefix_ops;
mod runtime_ops;
mod app_ops;
mod launch_ops;
mod wine_processes;
mod process_tracker;

pub use manager::Manager;
pub use wine_processes::{WineProcesses, apply_runtime_env};
pub use process_tracker::ProcessTracker;

// Re-exports from sub-crates for UI convenience
pub use base::{self, PrefixConfig, RegisteredExecutable, PrefixError, WinePrefix, PrefixInfo, GraphicsBackend, GraphicsConfig};
pub use base::config;
pub use registry::{RegEditor, RegistryEditor, WineRegistry, InMemoryRegistryCache, RegistryCache};
pub use registry;
pub use registry::keys;
pub use runtime::{Runtime, RuntimeSource, Channel, RuntimeManager};
pub use runtime;
pub use runtime::download;
pub use scan::{ApplicationScanner, IconCache};
pub use store::{PrefixStore, Settings};
