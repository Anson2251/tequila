pub mod error;
pub mod config;
pub mod traits;
pub mod graphics;

pub use error::{PrefixError, Result};
pub use config::{PrefixConfig, RegisteredExecutable, RegisteredExecutableBuilder};
pub use traits::{ConfigOperations, ExecutableManager, Scanner, WinePrefix, PrefixInfo};
pub use graphics::{GraphicsBackend, GraphicsConfig};
