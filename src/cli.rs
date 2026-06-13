use crate::config;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};

mod args;
mod files;
mod help;
mod option_sources;
mod streams;
mod targets;

use args::Command;

const PROGRAM_NAME: &str = env!("CARGO_PKG_NAME");
const DISPLAY_NAME: &str = "CStyle";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub struct CliError {
    message: String,
    exit_code: u8,
    errors_to_stdout: bool,
}

impl CliError {
    fn new(message: impl Into<String>, exit_code: u8) -> Self {
        Self {
            message: message.into(),
            exit_code,
            errors_to_stdout: false,
        }
    }

    fn with_stdout(mut self, errors_to_stdout: bool) -> Self {
        self.errors_to_stdout = errors_to_stdout;
        self
    }

    fn stdout(error: io::Error) -> Self {
        Self::new(format!("failed to write stdout: {error}"), 1)
    }

    pub fn exit_code(&self) -> u8 {
        self.exit_code
    }

    pub fn errors_to_stdout(&self) -> bool {
        self.errors_to_stdout
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

pub fn run_from_env() -> Result<(), CliError> {
    run(env::args_os().skip(1))
}

fn run(args: impl IntoIterator<Item = OsString>) -> Result<(), CliError> {
    let args = args.into_iter().collect::<Vec<_>>();
    let errors_to_stdout = args::requests_errors_to_stdout(&args);
    match args::parse(args).map_err(|error| error.with_stdout(errors_to_stdout))? {
        Command::Help => help::print(DISPLAY_NAME),
        Command::Version => {
            writeln!(io::stdout().lock(), "{} {}", PROGRAM_NAME, VERSION).map_err(CliError::stdout)
        }
        Command::Format {
            config,
            project_config,
            option_args,
            paths,
            stdin_path,
            stdout_path,
            console,
        } => {
            let command_line_errors_to_stdout = console.errors_to_stdout;
            let mut options = option_sources::load_selected_config(&config, &env::var_os)
                .map_err(|error| CliError::new(format!("config error: {error}"), 2))
                .map_err(|error| error.with_stdout(command_line_errors_to_stdout))?;
            option_sources::apply_selected_project_config(
                &mut options,
                &project_config,
                &paths,
                stdin_path.as_deref(),
                &env::var_os,
            )
            .map_err(|error| CliError::new(format!("config error: {error}"), 2))
            .map_err(|error| error.with_stdout(command_line_errors_to_stdout))?;
            config::apply_command_line_args(&mut options, &option_args)
                .map_err(|error| {
                    CliError::new(format!("option error: {error}. Try 'cstyle --help'."), 2)
                })
                .map_err(|error| error.with_stdout(console.errors_to_stdout))?;
            if paths.is_empty() {
                streams::format(stdin_path.as_deref(), stdout_path.as_deref(), &options)
            } else {
                files::format(
                    &paths,
                    &options,
                    console.recursive,
                    &console,
                    PROGRAM_NAME,
                    VERSION,
                )
            }
            .map_err(|error| error.with_stdout(console.errors_to_stdout))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cstyle-cli-{stamp}-{name}"))
    }

    #[test]
    fn rejects_bare_short_option_marker() {
        let error = run([OsString::from("--options=none"), OsString::from("-")])
            .expect_err("bare short option marker must fail");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("unknown option '-'"), "{error}");
    }

    #[test]
    fn parse_errors_honor_errors_to_stdout() {
        for flag in ["--errors-to-stdout", "-X"] {
            let error =
                run(args(&[flag, "--options="])).expect_err("empty options value must fail");

            assert_eq!(error.exit_code(), 2);
            assert!(error.errors_to_stdout());
        }
    }

    #[test]
    fn config_formatter_options_and_console_controls_are_composed() {
        let config_path = temp_path("formatter-options.rc");
        let source_path = temp_path("formatter-options.c");
        fs::write(&config_path, "pad-oper\n").expect("write formatter options");
        fs::write(&source_path, "int value=1+2;\n").expect("write source");
        let mut config_arg = OsString::from("--options=");
        config_arg.push(&config_path);

        let error = run([
            config_arg,
            OsString::from("--dry-run"),
            OsString::from("--error-on-changes"),
            source_path.as_os_str().to_os_string(),
        ])
        .expect_err("configured formatter change must be reported by dry-run");

        assert_eq!(error.exit_code(), 1);
        assert_eq!(
            fs::read_to_string(&source_path).expect("read source"),
            "int value=1+2;\n"
        );
        fs::remove_file(config_path).expect("remove options");
        fs::remove_file(source_path).expect("remove source");
    }

    #[test]
    fn mixed_console_and_formatter_short_options_share_one_bundle() {
        let path = temp_path("mixed-short-options.c");
        let backup = path.with_file_name(format!(
            "{}{}",
            path.file_name().expect("file name").to_string_lossy(),
            ".orig"
        ));
        fs::write(&path, "int f(){return 1+2;}\n").expect("write input");

        let result = run([
            OsString::from("--options=none"),
            OsString::from("-nA1p"),
            path.as_os_str().to_os_string(),
        ]);
        let output = fs::read_to_string(&path).expect("read output");
        let backup_exists = backup.is_file();
        fs::remove_file(&path).expect("remove input");
        if backup_exists {
            fs::remove_file(backup).expect("remove backup");
        }

        result.expect("mixed short bundle must format");
        assert_eq!(output, "int f()\n{\n    return 1 + 2;\n}\n");
        assert!(!backup_exists);
    }
}
