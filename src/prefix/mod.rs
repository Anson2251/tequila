pub mod config;
pub mod manager;
pub mod scanner;
pub mod error;
pub mod traits;
pub mod wine_processes;
pub mod regeditor;
pub mod icon_cache;
pub mod icon_extract;

#[allow(unused)]
pub use config::{PrefixConfig, RegisteredExecutable};
pub use manager::Manager;
pub use scanner::ApplicationScanner;
pub use error::{PrefixError, Result};
pub use traits::{ConfigOperations, Scanner, ExecutableManager, PrefixManager, WinePrefix, PrefixInfo};
pub use wine_processes::WineProcesses;
pub use regeditor::{RegEditor, WineRegistry, RegistryEditor, RegistryCache};
pub use icon_cache::IconCache;
pub use icon_extract::extract_icon;
pub use scanner::{extract_icon_for_exe, extract_metadata_for_exe};