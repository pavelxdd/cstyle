use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    match cstyle::cli::run_from_env() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if error.errors_to_stdout() {
                let _ = writeln!(io::stdout().lock(), "cstyle: {error}");
            } else {
                let _ = writeln!(io::stderr().lock(), "cstyle: {error}");
            }
            ExitCode::from(error.exit_code())
        }
    }
}
