pub use error::ConfigError;
pub(crate) use file_options::{BackupSuffix, ConfigFileOptions};
pub use files::{
    apply_file, apply_project_file, find_project_file, load_from_current_dir, load_from_dir,
    load_from_file,
};
pub(crate) use files::{apply_project_config_file, load_config_file, load_optional_config_file};
pub use options::{
    BraceStyle, FormatOptions, IndentStyle, LineBetweenMembers, LineEnding, MinConditionalIndent,
    Mode, ObjCColonPad, PointerAlign, ReferenceAlign, StylePreset,
};
pub use parser::apply_command_line_args;
pub(crate) use parser::split_short_options;

mod error;
mod file_options;
mod files;
mod options;
mod parser;

pub const CONFIG_FILE_NAME: &str = ".cstylerc";
pub const ASTYLE_CONFIG_FILE_NAME: &str = ".astylerc";
