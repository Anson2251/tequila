//! Registry editor module for Wine prefixes
//! 
//! This module provides functionality to edit Wine registry files using the Regashii library.
//! It implements a trait-based approach for registry operations with async support for
//! handling large registry files efficiently.

pub mod traits;
pub mod registry;
pub mod keys;
pub mod editor;
pub mod cache;

// Re-export main types for convenience
pub use traits::{RegEditor, RegistryCache};
pub use registry::WineRegistry;
pub use keys::*;
pub use editor::RegistryEditor;