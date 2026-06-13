use super::{FileFormatOptions, FormatPathResult, encoding::DecodedSource};
use crate::config::FormatOptions;
use std::ffi::OsString;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(crate) fn format_path_with_options(
    path: &Path,
    options: &FormatOptions,
    file_options: &FileFormatOptions,
) -> io::Result<FormatPathResult> {
    let input_bytes = fs::read(path)?;
    let input = DecodedSource::from_bytes(&input_bytes)?;
    let output_bytes = input.format(options);
    if output_bytes == input_bytes {
        return Ok(FormatPathResult { changed: false });
    }
    if file_options.dry_run {
        return Ok(FormatPathResult { changed: true });
    }
    let metadata = fs::metadata(path)?;
    let modified = file_options
        .preserve_date
        .then(|| metadata.modified())
        .transpose()?;
    if let Some(suffix) = &file_options.backup_suffix {
        let backup = backup_path(path, suffix);
        if backup == path {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "backup path is the input path",
            ));
        }
        replace_file_with_backup(
            path,
            &backup,
            &output_bytes,
            metadata.permissions(),
            modified,
        )?;
    } else {
        fs::write(path, output_bytes)?;
        if let Some(modified) = modified {
            File::open(path)?.set_modified(modified)?;
        }
    }
    Ok(FormatPathResult { changed: true })
}

fn backup_path(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

fn replace_file_with_backup(
    path: &Path,
    backup: &Path,
    output: &[u8],
    permissions: fs::Permissions,
    modified: Option<std::time::SystemTime>,
) -> io::Result<()> {
    match fs::symlink_metadata(backup) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("backup path is a symbolic link: {}", backup.display()),
            ));
        }
        Ok(_) => fs::remove_file(backup)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(path, backup)?;

    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(error) => return Err(restore_moved_input(path, backup, error, false)),
    };
    let write_result = file
        .write_all(output)
        .and_then(|()| file.set_permissions(permissions))
        .and_then(|()| modified.map_or(Ok(()), |time| file.set_modified(time)));
    drop(file);
    match write_result {
        Ok(()) => Ok(()),
        Err(error) => Err(restore_moved_input(path, backup, error, true)),
    }
}

fn restore_moved_input(
    path: &Path,
    backup: &Path,
    error: io::Error,
    remove_output: bool,
) -> io::Error {
    if remove_output
        && let Err(cleanup_error) = fs::remove_file(path)
        && cleanup_error.kind() != io::ErrorKind::NotFound
    {
        return io::Error::new(
            error.kind(),
            format!(
                "{error}; failed to remove incomplete output {}: {cleanup_error}",
                path.display()
            ),
        );
    }
    match fs::rename(backup, path) {
        Ok(()) => error,
        Err(restore_error) => io::Error::new(
            error.kind(),
            format!(
                "{error}; original input remains at {} because restoring {} failed: {restore_error}",
                backup.display(),
                path.display()
            ),
        ),
    }
}
