use super::CliError;
use crate::config::FormatOptions;
use crate::io as cstyle_io;
use std::fs::{self, File};
use std::io;
use std::path::Path;

pub(super) fn format(
    stdin_path: Option<&Path>,
    stdout_path: Option<&Path>,
    options: &FormatOptions,
) -> Result<(), CliError> {
    if let (Some(input), Some(output)) = (stdin_path, stdout_path) {
        reject_same_stdio_paths(input, output)?;
    }
    match (stdin_path, stdout_path) {
        (Some(input), Some(output)) => {
            let reader = File::open(input).map_err(|error| {
                CliError::new(format!("failed to open stdin {input:?}: {error}"), 1)
            })?;
            format_stream_to_path(reader, output, options)
        }
        (Some(input), None) => {
            let reader = File::open(input).map_err(|error| {
                CliError::new(format!("failed to open stdin {input:?}: {error}"), 1)
            })?;
            cstyle_io::format_reader_to_writer(reader, io::stdout(), options)
                .map_err(|error| CliError::new(format!("failed to format stdin: {error}"), 1))
        }
        (None, Some(output)) => format_stream_to_path(io::stdin(), output, options),
        (None, None) => cstyle_io::format_reader_to_writer(io::stdin(), io::stdout(), options)
            .map_err(|error| CliError::new(format!("failed to format stdin: {error}"), 1)),
    }
}

fn format_stream_to_path(
    reader: impl io::Read,
    output_path: &Path,
    options: &FormatOptions,
) -> Result<(), CliError> {
    let mut output = Vec::new();
    cstyle_io::format_reader_to_writer(reader, &mut output, options)
        .map_err(|error| CliError::new(format!("failed to format stdin: {error}"), 1))?;
    fs::write(output_path, output).map_err(|error| {
        CliError::new(
            format!("failed to write stdout {output_path:?}: {error}"),
            1,
        )
    })
}

fn reject_same_stdio_paths(input: &Path, output: &Path) -> Result<(), CliError> {
    let same_file = stdio_paths_are_same_file(input, output).map_err(|error| {
        CliError::new(
            format!("failed to inspect stdin/stdout paths before formatting: {error}"),
            1,
        )
    })?;
    if same_file {
        return Err(CliError::new(
            "stdin and stdout paths refer to the same file",
            2,
        ));
    }
    Ok(())
}

fn stdio_paths_are_same_file(input: &Path, output: &Path) -> io::Result<bool> {
    let input_metadata = match fs::metadata(input) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let output_metadata = match fs::metadata(output) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    same_file_metadata(input, output, &input_metadata, &output_metadata)
}

#[cfg(unix)]
fn same_file_metadata(
    _input: &Path,
    _output: &Path,
    input_metadata: &fs::Metadata,
    output_metadata: &fs::Metadata,
) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    Ok(input_metadata.dev() == output_metadata.dev()
        && input_metadata.ino() == output_metadata.ino())
}

#[cfg(windows)]
fn same_file_metadata(
    input: &Path,
    output: &Path,
    input_metadata: &fs::Metadata,
    output_metadata: &fs::Metadata,
) -> io::Result<bool> {
    use std::os::windows::fs::MetadataExt;

    match (
        input_metadata.volume_serial_number(),
        input_metadata.file_index(),
        output_metadata.volume_serial_number(),
        output_metadata.file_index(),
    ) {
        (Some(input_volume), Some(input_index), Some(output_volume), Some(output_index)) => {
            Ok(input_volume == output_volume && input_index == output_index)
        }
        _ => Ok(fs::canonicalize(input)? == fs::canonicalize(output)?),
    }
}

#[cfg(not(any(unix, windows)))]
fn same_file_metadata(
    input: &Path,
    output: &Path,
    _input_metadata: &fs::Metadata,
    _output_metadata: &fs::Metadata,
) -> io::Result<bool> {
    Ok(fs::canonicalize(input)? == fs::canonicalize(output)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cstyle-cli-{stamp}-{name}"))
    }

    #[test]
    fn rejects_same_stdio_input_and_output_path_without_truncating() {
        let path = temp_path("same-stdio.c");
        fs::write(&path, "int main(){return 0;}\n").expect("write stdio input");

        let error = format(Some(&path), Some(&path), &FormatOptions::default())
            .expect_err("same stdio path must fail");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("same file"));
        assert_eq!(
            fs::read_to_string(&path).expect("read stdio input"),
            "int main(){return 0;}\n"
        );
        fs::remove_file(path).expect("remove stdio input");
    }

    #[test]
    fn stream_paths_format_output_without_mutating_input() {
        let input = temp_path("stream-input.c");
        let output = temp_path("stream-output.c");
        fs::write(&input, "int main(){return 0;}\n").expect("write input");
        fs::write(&output, "old output\n").expect("write output");

        format(Some(&input), Some(&output), &FormatOptions::default())
            .expect("format stream paths");

        assert_eq!(
            fs::read_to_string(&input).expect("read input"),
            "int main(){return 0;}\n"
        );
        assert_eq!(
            fs::read_to_string(&output).expect("read output"),
            "int main() {\n    return 0;\n}\n"
        );
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");
    }

    #[test]
    fn stdout_file_is_unchanged_when_stream_input_is_invalid() {
        let input = temp_path("invalid-stream-input.c");
        let output = temp_path("invalid-stream-output.c");
        fs::write(&input, [0x00, 0x00, 0xfe, 0xff, 0x00, 0x00, 0x00, 0x41])
            .expect("write invalid input");
        fs::write(&output, "keep output\n").expect("write existing output");

        let result = format(Some(&input), Some(&output), &FormatOptions::default());
        let output_text = fs::read_to_string(&output).expect("read existing output");
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");

        let error = result.expect_err("invalid stream input must fail");
        assert_eq!(error.exit_code(), 1);
        assert_eq!(output_text, "keep output\n");
    }

    #[test]
    fn rejects_hard_link_stdio_alias_without_truncating() {
        let input = temp_path("hard-link-stdin.c");
        let output = temp_path("hard-link-stdout.c");
        fs::write(&input, "int main(){return 0;}\n").expect("write stdio input");
        fs::hard_link(&input, &output).expect("create hard link");

        let error = format(Some(&input), Some(&output), &FormatOptions::default())
            .expect_err("hard-linked stdio paths must fail");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("same file"));
        assert_eq!(
            fs::read_to_string(&input).expect("read stdio input"),
            "int main(){return 0;}\n"
        );
        fs::remove_file(input).expect("remove input");
        fs::remove_file(output).expect("remove output");
    }
}
