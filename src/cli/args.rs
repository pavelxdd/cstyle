use super::CliError;
use crate::config;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum Command {
    Help,
    Version,
    Format {
        config: ConfigSelection,
        project_config: ProjectConfigSelection,
        option_args: Vec<String>,
        paths: Vec<PathBuf>,
        stdin_path: Option<PathBuf>,
        stdout_path: Option<PathBuf>,
        console: ConsoleOptions,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ConfigSelection {
    Auto,
    File(PathBuf),
    None,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ProjectConfigSelection {
    Auto,
    FileName(OsString),
    None,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ConsoleOptions {
    pub(super) backup_suffix: Option<String>,
    pub(super) recursive: bool,
    pub(super) dry_run: bool,
    pub(super) error_on_changes: bool,
    pub(super) accept_empty_list: bool,
    pub(super) preserve_date: bool,
    pub(super) quiet: bool,
    pub(super) formatted_only: bool,
    pub(super) verbose: bool,
    pub(super) errors_to_stdout: bool,
    pub(super) excludes: Vec<String>,
    pub(super) exclude_errors: ExcludeErrorMode,
}

impl Default for ConsoleOptions {
    fn default() -> Self {
        Self {
            backup_suffix: Some(".orig".to_string()),
            recursive: false,
            dry_run: false,
            error_on_changes: false,
            accept_empty_list: false,
            preserve_date: false,
            quiet: false,
            formatted_only: false,
            verbose: false,
            errors_to_stdout: false,
            excludes: Vec::new(),
            exclude_errors: ExcludeErrorMode::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ExcludeErrorMode {
    Error,
    Show,
    Ignore,
}

pub(super) fn requests_errors_to_stdout(args: &[OsString]) -> bool {
    args.iter().any(|arg| match arg.to_str() {
        Some("--errors-to-stdout") => true,
        Some(arg) if arg.starts_with('-') && !arg.starts_with("--") => {
            config::split_short_options(arg.strip_prefix('-').unwrap_or_default())
                .iter()
                .any(|option| option == "X")
        }
        _ => false,
    })
}

fn parse_options_file_arg(value: OsString) -> Result<ConfigSelection, CliError> {
    if value.is_empty() {
        return Err(CliError::new("missing value for --options", 2));
    }
    if value == OsStr::new("none") {
        Ok(ConfigSelection::None)
    } else {
        Ok(ConfigSelection::File(PathBuf::from(value)))
    }
}

#[cfg(unix)]
fn strip_os_prefix(value: &OsStr, prefix: &str) -> Option<OsString> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    value
        .as_bytes()
        .strip_prefix(prefix.as_bytes())
        .map(|value| OsString::from_vec(value.to_vec()))
}

#[cfg(windows)]
fn strip_os_prefix(value: &OsStr, prefix: &str) -> Option<OsString> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let value = value.encode_wide().collect::<Vec<_>>();
    let prefix = OsStr::new(prefix).encode_wide().collect::<Vec<_>>();
    value
        .strip_prefix(&prefix)
        .map(|value| OsString::from_wide(value))
}

#[cfg(not(any(unix, windows)))]
fn strip_os_prefix(value: &OsStr, prefix: &str) -> Option<OsString> {
    value.to_str()?.strip_prefix(prefix).map(OsString::from)
}

fn parse_project_file_arg(value: OsString) -> Result<ProjectConfigSelection, CliError> {
    if value.is_empty() {
        return Err(CliError::new("missing value for --project", 2));
    }
    if value == OsStr::new("none") {
        Ok(ProjectConfigSelection::None)
    } else {
        Ok(ProjectConfigSelection::FileName(value))
    }
}

fn parse_stdio_path_arg(option: &str, value: OsString) -> Result<PathBuf, CliError> {
    if value.is_empty() {
        Err(CliError::new(format!("missing value for {option}"), 2))
    } else {
        Ok(PathBuf::from(value))
    }
}

fn apply_suffix_arg(console: &mut ConsoleOptions, value: &str) {
    if value == "none" {
        console.backup_suffix = None;
    } else if !value.is_empty() && console.backup_suffix.is_some() {
        console.backup_suffix = Some(value.to_string());
    }
}

fn apply_console_arg(console: &mut ConsoleOptions, arg: &str) -> bool {
    if let Some(option) = arg.strip_prefix("--") {
        return apply_console_option(console, option);
    }
    if let Some(options) = arg.strip_prefix('-') {
        if apply_console_option(console, options) {
            return true;
        }
        let mut handled = false;
        for ch in options.chars() {
            let mut short = String::new();
            short.push(ch);
            if apply_console_option(console, &short) {
                handled = true;
            } else {
                return false;
            }
        }
        return handled;
    }
    apply_console_option(console, arg)
}

fn apply_console_option(console: &mut ConsoleOptions, option: &str) -> bool {
    match option {
        "n" | "suffix=none" => console.backup_suffix = None,
        "r" | "R" | "recursive" => console.recursive = true,
        "dry-run" => console.dry_run = true,
        "error-on-changes" => console.error_on_changes = true,
        "accept-empty-list" => console.accept_empty_list = true,
        "Z" | "preserve-date" => console.preserve_date = true,
        "q" | "quiet" => console.quiet = true,
        "Q" | "formatted" => console.formatted_only = true,
        "v" | "verbose" => console.verbose = true,
        "X" | "errors-to-stdout" => console.errors_to_stdout = true,
        "i" | "ignore-exclude-errors" => console.exclude_errors = ExcludeErrorMode::Show,
        "xi" | "ignore-exclude-errors-x" => console.exclude_errors = ExcludeErrorMode::Ignore,
        option => {
            if let Some(value) = option.strip_prefix("suffix=") {
                apply_suffix_arg(console, value);
            } else if let Some(value) = option.strip_prefix("exclude=") {
                if !value.is_empty() {
                    console.excludes.push(value.to_string());
                }
            } else {
                return false;
            }
        }
    }
    true
}

pub(super) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, CliError> {
    let mut config = ConfigSelection::Auto;
    let mut project_config = ProjectConfigSelection::Auto;
    let mut option_args = Vec::new();
    let mut paths = Vec::new();
    let mut stdin_path = None;
    let mut stdout_path = None;
    let mut console = ConsoleOptions::default();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        if let Some(value) = strip_os_prefix(&arg, "--options=") {
            config = parse_options_file_arg(value)?;
            continue;
        }
        if let Some(value) = strip_os_prefix(&arg, "--project=") {
            project_config = parse_project_file_arg(value)?;
            continue;
        }
        if let Some(value) = strip_os_prefix(&arg, "--stdin=") {
            stdin_path = Some(parse_stdio_path_arg("--stdin", value)?);
            continue;
        }
        if let Some(value) = strip_os_prefix(&arg, "--stdout=") {
            stdout_path = Some(parse_stdio_path_arg("--stdout", value)?);
            continue;
        }
        match arg.to_str() {
            Some("-h" | "-?" | "--help") => return Ok(Command::Help),
            Some("-V" | "--version") => return Ok(Command::Version),
            Some("--mode") => {
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("missing value for --mode", 2))?
                    .into_string()
                    .map_err(|_| CliError::new("invalid UTF-8 value for --mode", 2))?;
                option_args.push(format!("--mode={value}"));
            }
            Some("--project") => {
                project_config = ProjectConfigSelection::FileName(OsString::from(
                    config::ASTYLE_CONFIG_FILE_NAME,
                ));
            }
            Some(value) if value.starts_with('-') && !value.starts_with("--") => {
                let short_options = value
                    .strip_prefix('-')
                    .expect("checked short option prefix");
                if short_options.is_empty() {
                    option_args.push(value.to_string());
                    continue;
                }
                for option in config::split_short_options(short_options) {
                    let option = format!("-{option}");
                    if !apply_console_arg(&mut console, &option) {
                        option_args.push(option);
                    }
                }
            }
            Some(value) if value.starts_with('-') && apply_console_arg(&mut console, value) => {}
            Some(value) if value.starts_with('-') || value.starts_with("mode=") => {
                option_args.push(value.to_string());
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }

    Ok(Command::Format {
        config,
        project_config,
        option_args,
        paths,
        stdin_path,
        stdout_path,
        console,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_help_and_version_commands() {
        assert_eq!(parse(args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse(args(&["-h"])).unwrap(), Command::Help);
        assert_eq!(parse(args(&["-?"])).unwrap(), Command::Help);
        assert_eq!(parse(args(&["--version"])).unwrap(), Command::Version);
        assert_eq!(parse(args(&["-V"])).unwrap(), Command::Version);
    }

    #[cfg(unix)]
    #[test]
    fn parses_non_utf8_default_config_path() {
        use std::os::unix::ffi::OsStringExt;

        let value = b"config-\xff.rc".to_vec();
        let mut option = b"--options=".to_vec();
        option.extend_from_slice(&value);
        let command = parse([OsString::from_vec(option)]).expect("parse options path");
        let Command::Format { config, paths, .. } = command else {
            panic!("expected format command");
        };

        assert_eq!(
            config,
            ConfigSelection::File(PathBuf::from(OsString::from_vec(value)))
        );
        assert!(paths.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn parses_non_utf8_project_config_name() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let mut option = b"--project=project-".to_vec();
        option.push(0xff);
        option.extend_from_slice(b".rc");
        let command = parse([OsString::from_vec(option)]).expect("parse project name");
        let Command::Format {
            project_config,
            paths,
            ..
        } = command
        else {
            panic!("expected format command");
        };

        let ProjectConfigSelection::FileName(name) = project_config else {
            panic!("expected project config file name");
        };
        assert_eq!(name.as_os_str().as_bytes(), b"project-\xff.rc");
        assert!(paths.is_empty());
    }

    #[test]
    fn parses_options_paths_and_legacy_mode_form() {
        assert_eq!(
            parse(args(&[
                "--indent=spaces=2",
                "--mode",
                "c",
                "mode=c",
                "input.c",
            ]))
            .unwrap(),
            Command::Format {
                config: ConfigSelection::Auto,
                project_config: ProjectConfigSelection::Auto,
                option_args: vec![
                    "--indent=spaces=2".to_string(),
                    "--mode=c".to_string(),
                    "mode=c".to_string()
                ],
                paths: vec![PathBuf::from("input.c")],
                stdin_path: None,
                stdout_path: None,
                console: ConsoleOptions::default(),
            }
        );
    }

    #[test]
    fn plain_file_names_that_resemble_options_remain_paths() {
        let command =
            parse(args(&["quiet", "suffix=.bak", "name=value.c"])).expect("parse file names");
        let Command::Format {
            option_args,
            paths,
            console,
            ..
        } = command
        else {
            panic!("expected format command");
        };

        assert!(option_args.is_empty());
        assert_eq!(
            paths,
            [
                PathBuf::from("quiet"),
                PathBuf::from("suffix=.bak"),
                PathBuf::from("name=value.c")
            ]
        );
        assert_eq!(console, ConsoleOptions::default());
    }

    #[test]
    fn parses_legacy_console_options() {
        assert_eq!(
            parse(args(&[
                "--options=.astylerc",
                "-Q",
                "-n",
                "-r",
                "*.c",
                "*.h",
            ]))
            .unwrap(),
            Command::Format {
                config: ConfigSelection::File(PathBuf::from(".astylerc")),
                project_config: ProjectConfigSelection::Auto,
                option_args: Vec::new(),
                paths: vec![PathBuf::from("*.c"), PathBuf::from("*.h")],
                stdin_path: None,
                stdout_path: None,
                console: ConsoleOptions {
                    backup_suffix: None,
                    recursive: true,
                    formatted_only: true,
                    ..ConsoleOptions::default()
                },
            }
        );
    }

    #[test]
    fn parses_reporting_backup_exclude_and_error_console_options() {
        assert_eq!(
            parse(args(&[
                "--dry-run",
                "--error-on-changes",
                "--accept-empty-list",
                "--suffix=.bak",
                "--preserve-date",
                "--verbose",
                "--errors-to-stdout",
                "--exclude=skip.c",
                "--ignore-exclude-errors-x",
                "input.c",
            ]))
            .unwrap(),
            Command::Format {
                config: ConfigSelection::Auto,
                project_config: ProjectConfigSelection::Auto,
                option_args: Vec::new(),
                paths: vec![PathBuf::from("input.c")],
                stdin_path: None,
                stdout_path: None,
                console: ConsoleOptions {
                    backup_suffix: Some(".bak".to_string()),
                    recursive: false,
                    dry_run: true,
                    error_on_changes: true,
                    accept_empty_list: true,
                    preserve_date: true,
                    quiet: false,
                    formatted_only: false,
                    verbose: true,
                    errors_to_stdout: true,
                    excludes: vec!["skip.c".to_string()],
                    exclude_errors: ExcludeErrorMode::Ignore,
                },
            }
        );
    }

    #[test]
    fn parses_stdio_redirect_options() {
        assert_eq!(
            parse(args(&["--stdin=input.c", "--stdout=output.c"])).unwrap(),
            Command::Format {
                config: ConfigSelection::Auto,
                project_config: ProjectConfigSelection::Auto,
                option_args: Vec::new(),
                paths: Vec::new(),
                stdin_path: Some(PathBuf::from("input.c")),
                stdout_path: Some(PathBuf::from("output.c")),
                console: ConsoleOptions::default(),
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn parses_non_utf8_stdio_paths() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let input = OsString::from_vec(b"input-\xff.c".to_vec());
        let output = OsString::from_vec(b"output-\xfe.c".to_vec());
        let mut stdin_arg = b"--stdin=".to_vec();
        stdin_arg.extend_from_slice(input.as_os_str().as_bytes());
        let mut stdout_arg = b"--stdout=".to_vec();
        stdout_arg.extend_from_slice(output.as_os_str().as_bytes());

        let command = parse([
            OsString::from_vec(stdin_arg),
            OsString::from_vec(stdout_arg),
        ])
        .expect("parse stdio paths");
        let Command::Format {
            stdin_path,
            stdout_path,
            paths,
            ..
        } = command
        else {
            panic!("expected format command");
        };

        assert_eq!(stdin_path, Some(PathBuf::from(input)));
        assert_eq!(stdout_path, Some(PathBuf::from(output)));
        assert!(paths.is_empty());
    }

    #[test]
    fn parses_project_options() {
        assert_eq!(
            parse(args(&["--project", "*.c"])).unwrap(),
            Command::Format {
                config: ConfigSelection::Auto,
                project_config: ProjectConfigSelection::FileName(OsString::from(".astylerc")),
                option_args: Vec::new(),
                paths: vec![PathBuf::from("*.c")],
                stdin_path: None,
                stdout_path: None,
                console: ConsoleOptions::default(),
            }
        );
        assert_eq!(
            parse(args(&["--project=custom.rc", "*.c"])).unwrap(),
            Command::Format {
                config: ConfigSelection::Auto,
                project_config: ProjectConfigSelection::FileName(OsString::from("custom.rc")),
                option_args: Vec::new(),
                paths: vec![PathBuf::from("*.c")],
                stdin_path: None,
                stdout_path: None,
                console: ConsoleOptions::default(),
            }
        );
        assert_eq!(
            parse(args(&["--project=none", "*.c"])).unwrap(),
            Command::Format {
                config: ConfigSelection::Auto,
                project_config: ProjectConfigSelection::None,
                option_args: Vec::new(),
                paths: vec![PathBuf::from("*.c")],
                stdin_path: None,
                stdout_path: None,
                console: ConsoleOptions::default(),
            }
        );
    }

    #[test]
    fn rejects_mode_without_value() {
        let error = parse(args(&["--mode"])).expect_err("missing mode value must fail");

        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.to_string(), "missing value for --mode");
    }

    #[test]
    fn rejects_options_without_value() {
        let error = parse(args(&["--options="])).expect_err("missing options value must fail");

        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.to_string(), "missing value for --options");
    }

    #[test]
    fn rejects_project_without_value() {
        let error = parse(args(&["--project="])).expect_err("missing project value must fail");

        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.to_string(), "missing value for --project");
    }

    #[test]
    fn rejects_stdio_redirect_without_value() {
        let error = parse(args(&["--stdin="])).expect_err("missing stdin value must fail");
        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.to_string(), "missing value for --stdin");

        let error = parse(args(&["--stdout="])).expect_err("missing stdout value must fail");
        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.to_string(), "missing value for --stdout");
    }

    #[test]
    fn no_backup_option_stays_set_after_later_suffix_options() {
        let mut console = ConsoleOptions::default();

        assert!(apply_console_arg(&mut console, "suffix=none"));
        assert!(apply_console_arg(&mut console, "suffix=.bak"));

        assert_eq!(console.backup_suffix, None);
    }
}
