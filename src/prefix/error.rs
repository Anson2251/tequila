use std::fmt;
use std::io;

/// Error types for prefix operations
///
/// This enum represents all possible errors that can occur during
/// Wine prefix management operations.
#[derive(Debug)]
pub enum PrefixError {
    /// I/O operation failed
    Io(io::Error),
    /// JSON serialization/deserialization failed
    Serialization(serde_json::Error),
    /// Data validation failed
    Validation(String),
    /// External process execution failed
    Process(String),
    /// Resource not found
    NotFound(String),
    /// Resource already exists
    AlreadyExists(String),
    /// Invalid file path
    InvalidPath(String),
    /// Wine-related operation failed
    Wine(String),
}

impl fmt::Display for PrefixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrefixError::Io(err) => write!(f, "IO error: {}", err),
            PrefixError::Serialization(err) => write!(f, "Serialization error: {}", err),
            PrefixError::Validation(msg) => write!(f, "Validation error: {}", msg),
            PrefixError::Process(msg) => write!(f, "Process error: {}", msg),
            PrefixError::NotFound(msg) => write!(f, "Not found: {}", msg),
            PrefixError::AlreadyExists(msg) => write!(f, "Already exists: {}", msg),
            PrefixError::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            PrefixError::Wine(msg) => write!(f, "Wine error: {}", msg),
        }
    }
}

impl std::error::Error for PrefixError {}

impl From<io::Error> for PrefixError {
    fn from(err: io::Error) -> Self {
        PrefixError::Io(err)
    }
}

impl From<serde_json::Error> for PrefixError {
    fn from(err: serde_json::Error) -> Self {
        PrefixError::Serialization(err)
    }
}

impl From<String> for PrefixError {
    fn from(err: String) -> Self {
        PrefixError::Validation(err)
    }
}

impl From<&str> for PrefixError {
    fn from(err: &str) -> Self {
        PrefixError::Validation(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PrefixError>;