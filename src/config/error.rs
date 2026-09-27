use std::path::Path;
use std::{fmt, io};

#[derive(Debug)]
pub struct ConfigError {
    message: String,
}

impl ConfigError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn io(path: &Path, error: io::Error) -> Self {
        Self::new(format!("failed to read {}: {error}", path.display()))
    }

    pub(crate) fn line(path: &Path, line_number: usize, message: impl fmt::Display) -> Self {
        Self::new(format!("{}:{line_number}: {message}", path.display()))
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}
