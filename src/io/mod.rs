mod encoding;
mod files;
mod stream;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FileFormatOptions {
    pub(crate) backup_suffix: Option<String>,
    pub(crate) dry_run: bool,
    pub(crate) preserve_date: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct FormatPathResult {
    pub(crate) changed: bool,
}

pub(crate) use files::format_path_with_options;
pub(crate) use stream::format_reader_to_writer;

#[cfg(test)]
mod tests {
    #![allow(clippy::field_reassign_with_default)]

    use super::*;
    use crate::config::{FormatOptions, LineEnding};
    use std::fs::{self, File};
    use std::io;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn format_bytes(input: &[u8], options: &FormatOptions) -> io::Result<Vec<u8>> {
        let mut output = Vec::new();
        format_reader_to_writer(input, &mut output, options)?;
        Ok(output)
    }

    fn format_path(path: &Path, options: &FormatOptions) -> io::Result<()> {
        format_path_with_options(
            path,
            options,
            &FileFormatOptions {
                backup_suffix: None,
                dry_run: false,
                preserve_date: false,
            },
        )
        .map(|_| ())
    }

    fn utf16le_with_bom(text: &str) -> Vec<u8> {
        let mut output = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            output.extend_from_slice(&unit.to_le_bytes());
        }
        output
    }

    fn utf16be_with_bom(text: &str) -> Vec<u8> {
        let mut output = vec![0xFE, 0xFF];
        for unit in text.encode_utf16() {
            output.extend_from_slice(&unit.to_be_bytes());
        }
        output
    }

    fn decode_utf16le_with_bom(bytes: &[u8]) -> String {
        let body = bytes.strip_prefix(&[0xFF, 0xFE]).expect("UTF-16LE BOM");
        let units = body
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).expect("UTF-16LE output")
    }

    fn decode_utf16be_with_bom(bytes: &[u8]) -> String {
        let body = bytes.strip_prefix(&[0xFE, 0xFF]).expect("UTF-16BE BOM");
        let units = body
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        String::from_utf16(&units).expect("UTF-16BE output")
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cstyle-{stamp}-{name}"))
    }

    #[test]
    fn preserves_utf8_bom_when_formatting_reader() {
        let mut input = vec![0xEF, 0xBB, 0xBF];
        input.extend_from_slice(b"int main(){return 0;}\n");

        let output = format_bytes(&input, &FormatOptions::default()).expect("format UTF-8 BOM");

        assert!(output.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert_eq!(
            std::str::from_utf8(&output[3..]).expect("UTF-8 output"),
            "int main() {\n    return 0;\n}\n"
        );
    }

    #[test]
    fn formats_eight_bit_input_and_preserves_high_bytes() {
        let input = b"int x=1;// caf\xe9\nvoid f(){int y=2;}\n";

        let output = format_bytes(input, &FormatOptions::default()).expect("format 8-bit input");

        assert_eq!(output, b"int x=1;// caf\xe9\nvoid f() {\n    int y=2;\n}\n");
    }

    #[test]
    fn preserves_high_bytes_inside_string_literal() {
        let input = b"const char* s=\"\xe9\xe8\";int a=3;\n";

        let output = format_bytes(input, &FormatOptions::default()).expect("format 8-bit string");

        assert_eq!(output, b"const char* s=\"\xe9\xe8\";\nint a=3;\n");
    }

    #[test]
    fn preserves_eight_bit_identifier_separator() {
        let input = b"int \xe9;\n";

        let output =
            format_bytes(input, &FormatOptions::default()).expect("format 8-bit identifier");

        assert_eq!(output, input);
    }

    #[test]
    fn preserves_utf16le_encoding_when_formatting_reader() {
        let input = utf16le_with_bom("int main(){return 0;}\n");

        let output = format_bytes(&input, &FormatOptions::default()).expect("format UTF-16LE");

        assert_eq!(
            decode_utf16le_with_bom(&output),
            "int main() {\n    return 0;\n}\n"
        );
    }

    #[test]
    fn rejects_malformed_bom_encoded_input() {
        for input in [
            &[0xEF, 0xBB, 0xBF, 0xFF][..],
            &[0xFF, 0xFE, 0x41][..],
            &[0xFE, 0xFF, 0xD8, 0x00][..],
        ] {
            let error = format_bytes(input, &FormatOptions::default())
                .expect_err("malformed encoded input must fail");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }

    #[test]
    fn rejects_utf32_input() {
        let error = format_bytes(&[0xFF, 0xFE, 0x00, 0x00], &FormatOptions::default())
            .expect_err("UTF-32 must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn default_reader_formatting_preserves_observed_line_endings() {
        let crlf = format_bytes(b"int main(){return 0;}\r\n", &FormatOptions::default())
            .expect("format CRLF input");
        assert_eq!(
            std::str::from_utf8(&crlf).expect("UTF-8 CRLF output"),
            "int main() {\r\n    return 0;\r\n}\r\n"
        );

        let cr = format_bytes(b"int main(){return 0;}\r", &FormatOptions::default())
            .expect("format CR input");
        assert_eq!(
            std::str::from_utf8(&cr).expect("UTF-8 CR output"),
            "int main() {\r    return 0;\r}\r"
        );
    }

    #[test]
    fn mixed_reader_line_endings_use_stable_dominant_ending() {
        let output = format_bytes(b"int a;\r\nint b;\n", &FormatOptions::default())
            .expect("format mixed input");

        assert_eq!(
            std::str::from_utf8(&output).expect("UTF-8 mixed output"),
            "int a;\r\nint b;\r\n"
        );
    }

    #[test]
    fn configured_lf_overrides_observed_crlf() {
        let mut options = FormatOptions::default();
        options.line_ending = LineEnding::Lf;

        let output = format_bytes(b"int main(){return 0;}\r\n", &options).expect("format LF");

        assert_eq!(
            std::str::from_utf8(&output).expect("UTF-8 output"),
            "int main() {\n    return 0;\n}\n"
        );
    }

    #[test]
    fn configured_line_endings_are_written_by_reader_formatting() {
        let mut options = FormatOptions::default();
        options.line_ending = LineEnding::Crlf;

        let output = format_bytes(b"int main(){return 0;}\n", &options).expect("format CRLF");

        assert_eq!(
            std::str::from_utf8(&output).expect("UTF-8 output"),
            "int main() {\r\n    return 0;\r\n}\r\n"
        );
    }

    #[test]
    fn reader_formatting_preserves_missing_final_line_break() {
        let output = format_bytes(b"int main(){return 0;}", &FormatOptions::default())
            .expect("format without final newline");

        assert_eq!(
            std::str::from_utf8(&output).expect("UTF-8 output"),
            "int main() {\n    return 0;\n}"
        );
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

        let result = format_path_with_options(
            &path,
            &FormatOptions::default(),
            &FileFormatOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect("format with backup");

        assert!(result.changed);
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
        let result = format_path_with_options(
            &path,
            &FormatOptions::default(),
            &FileFormatOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: true,
                preserve_date: false,
            },
        )
        .expect("dry run");
        assert!(result.changed);
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
        format_path_with_options(
            &path,
            &FormatOptions::default(),
            &FileFormatOptions {
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

        format_path_with_options(
            &path,
            &FormatOptions::default(),
            &FileFormatOptions {
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

        format_path_with_options(
            &path,
            &FormatOptions::default(),
            &FileFormatOptions {
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

        let result = format_path_with_options(
            &path,
            &FormatOptions::default(),
            &FileFormatOptions {
                backup_suffix: Some(".orig".to_string()),
                dry_run: false,
                preserve_date: false,
            },
        )
        .expect("format with hard-linked backup");

        assert!(result.changed);
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

        let error = format_path_with_options(
            &path,
            &FormatOptions::default(),
            &FileFormatOptions {
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
    fn utf8_bom_and_utf16_preserve_observed_line_endings() {
        let mut utf8_bom = vec![0xEF, 0xBB, 0xBF];
        utf8_bom.extend_from_slice(b"int main(){return 0;}\r\n");
        let utf8_output = format_bytes(&utf8_bom, &FormatOptions::default()).expect("UTF-8 BOM");
        assert_eq!(
            std::str::from_utf8(&utf8_output[3..]).expect("UTF-8 BOM output"),
            "int main() {\r\n    return 0;\r\n}\r\n"
        );

        let utf16_output = format_bytes(
            &utf16le_with_bom("int main(){return 0;}\r\n"),
            &FormatOptions::default(),
        )
        .expect("UTF-16LE");
        assert_eq!(
            decode_utf16le_with_bom(&utf16_output),
            "int main() {\r\n    return 0;\r\n}\r\n"
        );
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

        format_path_with_options(
            &path,
            &FormatOptions::default(),
            &FileFormatOptions {
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
    fn format_path_reports_changed_readonly_write_errors() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("readonly-changed.c");
        fs::write(&path, "int main(){return 0;}\n").expect("write input");
        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o444);
        fs::set_permissions(&path, permissions).expect("set readonly");

        let error = format_path(&path, &FormatOptions::default()).expect_err("readonly write");

        let mut permissions = fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(&path, permissions).expect("restore writable");
        fs::remove_file(path).expect("remove temp file");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
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
