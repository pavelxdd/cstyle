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

    pub(crate) fn at(location: OptionLocation<'_>, message: impl fmt::Display) -> Self {
        Self::new(format!(
            "{}:{}: {message}",
            location.path.display(),
            location.line_number
        ))
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

/// The config file line or command-line argument an option came from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OptionLocation<'a> {
    pub(crate) path: &'a Path,
    pub(crate) line_number: usize,
}
