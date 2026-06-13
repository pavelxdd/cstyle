use std::fmt;
use std::io;
use std::path::Path;

pub const CONFIG_FILE_NAME: &str = ".cstylerc";
pub const ASTYLE_CONFIG_FILE_NAME: &str = ".astylerc";

mod options;

pub use options::{
    BraceStyle, FormatOptions, IndentStyle, LineBetweenMembers, LineEnding, MinConditionalIndent,
    Mode, ObjCColonPad, PointerAlign, ReferenceAlign, StylePreset,
};

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

    fn line(path: &Path, line_number: usize, message: impl fmt::Display) -> Self {
        Self::new(format!("{}:{line_number}: {message}", path.display()))
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

mod files;

pub(crate) use files::load_optional_file;
pub use files::{
    apply_file, apply_project_file, find_project_file, load_from_current_dir, load_from_dir,
    load_from_file,
};

mod parser;

pub use parser::apply_command_line_args;
pub(crate) use parser::split_short_options;
