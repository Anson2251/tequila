pub mod config;
pub mod manager;
pub mod scanner;
pub mod error;
pub mod traits;
pub mod wine_processes;
pub mod regeditor;

#[allow(unused)]
pub use config::{PrefixConfig, RegisteredExecutable};
pub use manager::Manager;
pub use scanner::ApplicationScanner;
pub use error::{PrefixError, Result};
pub use traits::{ConfigOperations, Scanner, ExecutableManager, PrefixManager, WinePrefix, PrefixInfo};
pub use wine_processes::WineProcesses;
pub use regeditor::{RegEditor, WineRegistry, RegistryEditor, RegistryCache};