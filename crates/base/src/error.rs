use std::fmt;
use std::io;

#[derive(Debug)]
pub enum PrefixError {
    Io(io::Error),
    Serialization(serde_json::Error),
    Validation(String),
    Process(String),
    NotFound(String),
    AlreadyExists(String),
    InvalidPath(String),
    Wine(String),
    RegistryError(String),
    ValidationError(String),
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
            PrefixError::RegistryError(msg) => write!(f, "Registry error: {}", msg),
            PrefixError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
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
