use crate::api;
use crate::config::FormatOptions;
use std::ffi::OsString;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct InPlaceOptions {
    pub(super) backup_suffix: Option<String>,
    pub(super) dry_run: bool,
    pub(super) preserve_date: bool,
}

/// Formats `path` in place and returns whether its content changed.
pub(super) fn format_file_in_place(
    path: &Path,
    options: &FormatOptions,
    in_place: &InPlaceOptions,
) -> io::Result<bool> {
    let input = fs::read(path)?;
    let output = api::format_bytes(&input, options)?;
    if output == input {
        return Ok(false);
    }
    if in_place.dry_run {
        return Ok(true);
    }
    let metadata = fs::metadata(path)?;
    let modified = in_place
        .preserve_date
        .then(|| metadata.modified())
        .transpose()?;
    if let Some(suffix) = &in_place.backup_suffix {
        let backup = backup_path(path, suffix);
        if backup == path {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "backup path is the input path",
            ));
        }
        replace_file_with_backup(path, &backup, &output, metadata.permissions(), modified)?;
    } else {
        fs::write(path, output)?;
        if let Some(modified) = modified {
            File::open(path)?.set_modified(modified)?;
        }
    }
    Ok(true)
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

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]

    use super::*;
    use crate::config::LineEnding;
    use crate::test_support::temp_path;
    use std::fs::File;
    use std::time::UNIX_EPOCH;

    fn format_path(path: &Path, options: &FormatOptions) -> io::Result<()> {
        format_file_in_place(
            path,
            options,
            &InPlaceOptions {
                backup_suffix: None,
                dry_run: false,
                preserve_date: false,
            },
        )
        .map(|_| ())
    }

    fn utf16be_with_bom(text: &str) -> Vec<u8> {
        let mut output = vec![0xFE, 0xFF];
        for unit in text.encode_utf16() {
            output.extend_from_slice(&unit.to_be_bytes());
        }
        output
    }

    fn decode_utf16be_with_bom(bytes: &[u8]) -> String {
        let body = bytes.strip_prefix(&[0xFE, 0xFF]).expect("UTF-16BE BOM");
        let units = body
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).expect("UTF-16BE output")
    }

    #[test]
    fn format_path_writes_changed_files_without_backup() {
        let path = temp_path("changed.c");
        fs::write(&path, "int main(){return 0;}\n").expect("write input");

        format_path(&path, &FormatOptions::default()).expect("format path");

        assert_eq!(
            fs::read_to_string(&path).expect("read output"),
            "int main() {\n    return 0;\n}\n"
        );
        assert!(!path.with_extension("c.orig").exists());
        fs::remove_file(path).expect("remove temp file");
    }

    #[test]
    fn format_path_with_options_creates_backups_dry_runs_and_preserves_date() {
        let path = temp_path("backup.c");
        fs::write(&path, "int main(){return 0;}\n").expect("write input");
        let backup = PathBuf::from(format!("{}.orig", path.display()));

        let changed = format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect("format with backup");

        assert!(changed);
        assert_eq!(
            fs::read_to_string(&path).expect("read output"),
            "int main() {\n    return 0;\n}\n"
        );
        assert_eq!(
            fs::read_to_string(&backup).expect("read backup"),
            "int main(){return 0;}\n"
        );
        fs::remove_file(&path).expect("remove output");
        fs::remove_file(backup).expect("remove backup");

        fs::write(&path, "int main(){return 0;}\n").expect("write dry-run input");
        let changed = format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: true,
                preserve_date: false,
            },
        )
        .expect("dry run");
        assert!(changed);
        assert_eq!(
            fs::read_to_string(&path).expect("read dry-run output"),
            "int main(){return 0;}\n"
        );
        assert!(!PathBuf::from(format!("{}.orig", path.display())).exists());
        fs::remove_file(&path).expect("remove dry-run input");

        fs::write(&path, "int main(){return 0;}\n").expect("write preserve-date input");
        let modified = UNIX_EPOCH + std::time::Duration::from_secs(1_000_000);
        File::open(&path)
            .expect("open preserve-date input")
            .set_modified(modified)
            .expect("set modified");
        format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: None,
                dry_run: false,
                preserve_date: true,
            },
        )
        .expect("format preserve date");
        assert_eq!(
            fs::metadata(&path)
                .expect("preserve-date metadata")
                .modified()
                .expect("preserve-date modified"),
            modified
        );
        fs::remove_file(path).expect("remove preserve-date input");
    }

    #[test]
    fn format_path_with_backup_does_not_modify_other_hard_links() {
        let path = temp_path("source-hard-link.c");
        let other = temp_path("source-hard-link-other.c");
        let backup = PathBuf::from(format!("{}.orig", path.display()));
        let input = "int main(){return 0;}\n";
        fs::write(&path, input).expect("write input");
        fs::hard_link(&path, &other).expect("create source hard link");

        format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect("format hard-linked source");

        assert_eq!(
            fs::read_to_string(&path).expect("read output"),
            "int main() {\n    return 0;\n}\n"
        );
        assert_eq!(fs::read_to_string(&other).expect("read other link"), input);
        assert_eq!(fs::read_to_string(&backup).expect("read backup"), input);
        fs::remove_file(path).expect("remove output");
        fs::remove_file(other).expect("remove other link");
        fs::remove_file(backup).expect("remove backup");
    }

    #[cfg(unix)]
    #[test]
    fn format_path_with_backup_replaces_source_symlink_without_modifying_target() {
        let path = temp_path("source-symlink.c");
        let target = temp_path("source-symlink-target.c");
        let backup = PathBuf::from(format!("{}.orig", path.display()));
        let input = "int main(){return 0;}\n";
        fs::write(&target, input).expect("write target");
        std::os::unix::fs::symlink(&target, &path).expect("create source symlink");

        format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect("format source symlink");

        assert!(!path.is_symlink());
        assert_eq!(
            fs::read_to_string(&path).expect("read output"),
            "int main() {\n    return 0;\n}\n"
        );
        assert_eq!(fs::read_to_string(&target).expect("read target"), input);
        assert_eq!(fs::read_to_string(&backup).expect("read backup"), input);
        fs::remove_file(path).expect("remove output");
        fs::remove_file(target).expect("remove target");
        fs::remove_file(backup).expect("remove backup");
    }

    #[test]
    fn format_path_replaces_backup_hard_link_with_original_contents() {
        let path = temp_path("backup-hard-link.c");
        let backup = PathBuf::from(format!("{}.orig", path.display()));
        let input = "int main(){return 0;}\n";
        fs::write(&path, input).expect("write input");
        fs::hard_link(&path, &backup).expect("create backup hard link");

        let changed = format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect("format with hard-linked backup");

        assert!(changed);
        assert_eq!(
            fs::read_to_string(&path).expect("read output"),
            "int main() {\n    return 0;\n}\n"
        );
        assert_eq!(fs::read_to_string(&backup).expect("read backup"), input);
        fs::remove_file(path).expect("remove output");
        fs::remove_file(backup).expect("remove backup");
    }

    #[cfg(unix)]
    #[test]
    fn format_path_rejects_backup_symlink_without_writing_target() {
        let path = temp_path("backup-symlink.c");
        let target = temp_path("backup-symlink-target.txt");
        let backup = PathBuf::from(format!("{}.orig", path.display()));
        fs::write(&path, "int main(){return 0;}\n").expect("write input");
        fs::write(&target, "keep me\n").expect("write backup target");
        std::os::unix::fs::symlink(&target, &backup).expect("create backup symlink");

        let error = format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect_err("backup symlink must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            fs::read_to_string(&path).expect("read input"),
            "int main(){return 0;}\n"
        );
        assert_eq!(
            fs::read_to_string(&target).expect("read backup target"),
            "keep me\n"
        );
        fs::remove_file(backup).expect("remove backup symlink");
        fs::remove_file(path).expect("remove input");
        fs::remove_file(target).expect("remove backup target");
    }

    #[test]
    fn format_path_preserves_observed_crlf_by_default() {
        let path = temp_path("crlf.c");
        fs::write(&path, "int main(){return 0;}\r\n").expect("write CRLF input");

        format_path(&path, &FormatOptions::default()).expect("format path");

        assert_eq!(
            fs::read(&path).expect("read output"),
            b"int main() {\r\n    return 0;\r\n}\r\n"
        );
        fs::remove_file(path).expect("remove temp file");
    }

    #[test]
    fn format_path_applies_line_ending_only_changes() {
        let path = temp_path("lineend.c");
        fs::write(&path, "int main()\n{\n    return 0;\n}\n").expect("write input");
        let mut options = FormatOptions::default();
        options.line_ending = LineEnding::Crlf;

        format_path(&path, &options).expect("format path");

        assert_eq!(
            fs::read(&path).expect("read output"),
            b"int main()\r\n{\r\n    return 0;\r\n}\r\n"
        );
        fs::remove_file(path).expect("remove temp file");
    }

    #[cfg(unix)]
    #[test]
    fn format_path_with_backup_preserves_readonly_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("readonly-backup.c");
        let backup = PathBuf::from(format!("{}.orig", path.display()));
        fs::write(&path, "int main(){return 0;}\n").expect("write input");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o444);
        fs::set_permissions(&path, permissions).expect("set readonly");

        format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect("format readonly path with backup");

        assert_eq!(
            fs::metadata(&path)
                .expect("output metadata")
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
        assert_eq!(
            fs::metadata(&backup)
                .expect("backup metadata")
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
        fs::remove_file(path).expect("remove output");
        fs::remove_file(backup).expect("remove backup");
    }

    #[cfg(unix)]
    #[test]
    fn format_path_skips_unchanged_readonly_files() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("readonly.c");
        fs::write(&path, "int main()\n{\n    return 0;\n}\n").expect("write input");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o444);
        fs::set_permissions(&path, permissions).expect("set readonly");

        format_path(&path, &FormatOptions::default()).expect("format readonly unchanged path");

        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&path, permissions).expect("restore writable");
        fs::remove_file(path).expect("remove temp file");
    }

    #[cfg(unix)]
    #[test]
    fn format_path_reports_backup_write_errors() {
        // A backup under the input file, as under a directory, fails to
        // write for any user and leaves the input as it was.
        let path = temp_path("unwritable-backup.c");
        let input = "int main(){return 0;}\n";
        fs::write(&path, input).expect("write input");

        let error = format_file_in_place(
            &path,
            &FormatOptions::default(),
            &InPlaceOptions {
                backup_suffix: Some("/orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect_err("backup write");

        let unchanged = fs::read_to_string(&path).expect("read input");
        fs::remove_file(path).expect("remove temp file");
        assert_eq!(unchanged, input);
        assert_ne!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn format_path_reports_read_errors() {
        let path = temp_path("missing-dir").join("missing.c");

        let error = format_path(&path, &FormatOptions::default()).expect_err("missing path");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn format_path_preserves_original_utf16be_encoding() {
        let path = temp_path("utf16be.c");
        fs::write(&path, utf16be_with_bom("int main(){return 0;}\n")).expect("write input");

        format_path(&path, &FormatOptions::default()).expect("format path");
        let output = fs::read(&path).expect("read output");

        assert_eq!(
            decode_utf16be_with_bom(&output),
            "int main() {\n    return 0;\n}\n"
        );
        fs::remove_file(path).expect("remove temp file");
    }
}
