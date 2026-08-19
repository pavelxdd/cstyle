use super::args::{ConsoleOptions, ExcludeErrorMode};
use super::{CliError, targets};
use crate::config::FormatOptions;
use crate::io as cstyle_io;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(super) fn format(
    paths: &[PathBuf],
    options: &FormatOptions,
    recursive: bool,
    console: &ConsoleOptions,
    program_name: &str,
    version: &str,
) -> Result<(), CliError> {
    let mut matched_excludes = vec![false; console.excludes.len()];
    let expanded = targets::expand_target_paths_with_excludes(
        paths,
        recursive,
        console.accept_empty_list,
        console.backup_suffix.as_deref(),
        &console.excludes,
        &mut matched_excludes,
    )?;
    handle_unmatched_excludes(console, matched_excludes)?;
    let mut formatted = 0usize;
    let mut unchanged = 0usize;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    if console.verbose && !console.quiet {
        writeln!(stdout, "{program_name} {version}").map_err(CliError::stdout)?;
    }
    for path in expanded {
        targets::validate_target_path(&path)?;
        let changed = format_file(&path, options, console)?;
        if changed {
            formatted += 1;
        } else {
            unchanged += 1;
        }
        print_file_status(&mut stdout, &path, changed, console).map_err(CliError::stdout)?;
    }
    if console.verbose && !console.quiet {
        writeln!(stdout, " {formatted} formatted   {unchanged} unchanged")
            .map_err(CliError::stdout)?;
    }
    if console.dry_run && console.error_on_changes && formatted > 0 {
        return Err(CliError::new("dry-run found files that would change", 1));
    }
    Ok(())
}

fn handle_unmatched_excludes(console: &ConsoleOptions, matched: Vec<bool>) -> Result<(), CliError> {
    let unmatched = console
        .excludes
        .iter()
        .zip(matched)
        .filter_map(|(pattern, matched)| (!matched).then_some(pattern))
        .collect::<Vec<_>>();
    if unmatched.is_empty() {
        return Ok(());
    }
    match console.exclude_errors {
        ExcludeErrorMode::Error => Err(CliError::new(
            format!("unmatched exclude {}", unmatched[0]),
            1,
        )),
        ExcludeErrorMode::Show => {
            if !console.quiet {
                let stdout = io::stdout();
                let mut stdout = stdout.lock();
                for pattern in unmatched {
                    writeln!(stdout, "Unmatched exclude {pattern}").map_err(CliError::stdout)?;
                }
            }
            Ok(())
        }
        ExcludeErrorMode::Ignore => Ok(()),
    }
}

fn print_file_status(
    writer: &mut impl Write,
    path: &Path,
    changed: bool,
    console: &ConsoleOptions,
) -> io::Result<()> {
    if console.quiet || (console.formatted_only && !changed) {
        return Ok(());
    }
    let status = if changed { "Formatted" } else { "Unchanged" };
    writeln!(writer, "{status}  {}", path.display())
}

fn format_file(
    path: &Path,
    options: &FormatOptions,
    console: &ConsoleOptions,
) -> Result<bool, CliError> {
    cstyle_io::format_path_with_options(
        path,
        options,
        &cstyle_io::FileFormatOptions {
            backup_suffix: console.backup_suffix.clone(),
            dry_run: console.dry_run,
            preserve_date: console.preserve_date,
        },
    )
    .map(|result| result.changed)
    .map_err(|error| CliError::new(format!("failed to format {}: {error}", path.display()), 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cstyle-cli-{stamp}-{name}"))
    }

    fn format_paths(
        paths: &[PathBuf],
        recursive: bool,
        console: &ConsoleOptions,
    ) -> Result<(), CliError> {
        format(
            paths,
            &FormatOptions::default(),
            recursive,
            console,
            "cstyle",
            "test-version",
        )
    }

    #[test]
    fn unmatched_excludes_follow_console_policy() {
        let mut console = ConsoleOptions {
            excludes: vec!["missing.c".to_string()],
            ..ConsoleOptions::default()
        };

        let error = handle_unmatched_excludes(&console, vec![false])
            .expect_err("unmatched exclude must fail");
        assert_eq!(error.to_string(), "unmatched exclude missing.c");

        console.exclude_errors = ExcludeErrorMode::Ignore;
        handle_unmatched_excludes(&console, vec![false]).expect("ignored unmatched exclude");
    }

    #[test]
    fn dry_run_error_on_changes_returns_exit_one_when_file_would_change() {
        let path = temp_path("error-on-changes-changed.c");
        fs::write(&path, "int main(){return 0;}\n").expect("write changed input");
        let original = fs::read_to_string(&path).expect("read changed input");
        let console = ConsoleOptions {
            dry_run: true,
            error_on_changes: true,
            ..ConsoleOptions::default()
        };

        let error = format_paths(std::slice::from_ref(&path), false, &console)
            .expect_err("dry-run changes must fail");

        assert_eq!(error.exit_code(), 1);
        assert_eq!(
            fs::read_to_string(&path).expect("read dry-run input"),
            original
        );
        fs::remove_file(path).expect("remove changed input");
    }

    #[test]
    fn dry_run_error_on_changes_succeeds_when_file_is_unchanged() {
        let path = temp_path("error-on-changes-unchanged.c");
        fs::write(&path, "int value;\n").expect("write unchanged input");
        let console = ConsoleOptions {
            dry_run: true,
            error_on_changes: true,
            ..ConsoleOptions::default()
        };

        format_paths(std::slice::from_ref(&path), false, &console)
            .expect("unchanged dry-run succeeds");

        assert_eq!(
            fs::read_to_string(&path).expect("read unchanged input"),
            "int value;\n"
        );
        fs::remove_file(path).expect("remove unchanged input");
    }

    #[test]
    fn error_on_changes_without_dry_run_keeps_normal_write_behavior() {
        let path = temp_path("error-on-changes-write.c");
        fs::write(&path, "int main(){return 0;}\n").expect("write input");
        let console = ConsoleOptions {
            error_on_changes: true,
            ..ConsoleOptions::default()
        };

        format_paths(std::slice::from_ref(&path), false, &console).expect("normal write succeeds");

        assert_ne!(
            fs::read_to_string(&path).expect("read formatted input"),
            "int main(){return 0;}\n"
        );
        fs::remove_file(&path).expect("remove formatted input");
        fs::remove_file(path.with_file_name(format!(
            "{}{}",
            path.file_name().unwrap().to_string_lossy(),
            ".orig"
        )))
        .expect("remove backup");
    }

    #[test]
    fn missing_wildcard_targets_fail_without_accept_empty_list() {
        let dir = temp_path("empty-wildcard-fail");
        fs::create_dir_all(&dir).expect("create empty dir");

        let error = format_paths(&[dir.join("*.c")], false, &ConsoleOptions::default())
            .expect_err("empty wildcard must fail");

        assert_eq!(error.exit_code(), 1);
        assert!(error.to_string().contains("no file to process"), "{error}");
        fs::remove_dir_all(dir).expect("remove empty dir");
    }

    #[test]
    fn accept_empty_list_allows_missing_wildcard_targets() {
        let dir = temp_path("empty-wildcard-ok");
        fs::create_dir_all(&dir).expect("create empty dir");
        let console = ConsoleOptions {
            accept_empty_list: true,
            ..ConsoleOptions::default()
        };

        format_paths(&[dir.join("*.c")], false, &console)
            .expect("accepted empty wildcard succeeds");

        fs::remove_dir_all(dir).expect("remove empty dir");
    }

    #[test]
    fn accept_empty_list_does_not_hide_missing_direct_targets() {
        let path = temp_path("missing-direct.c");
        let console = ConsoleOptions {
            accept_empty_list: true,
            ..ConsoleOptions::default()
        };

        let error = format_paths(std::slice::from_ref(&path), false, &console)
            .expect_err("missing direct target must fail");

        assert_eq!(error.exit_code(), 1);
        assert!(
            error.to_string().contains("input file not found"),
            "{error}"
        );
    }

    #[test]
    fn accept_empty_list_does_not_hide_missing_comma_direct_targets() {
        let dir = temp_path("missing-comma-direct");
        fs::create_dir_all(&dir).expect("create empty dir");
        let console = ConsoleOptions {
            accept_empty_list: true,
            ..ConsoleOptions::default()
        };

        let error = format_paths(&[dir.join("a.c,b.c")], false, &console)
            .expect_err("missing comma target must fail");

        assert_eq!(error.exit_code(), 1);
        assert!(error.to_string().contains("no file to process"), "{error}");
        fs::remove_dir_all(dir).expect("remove empty dir");
    }

    #[test]
    fn wildcard_expansion_skips_active_backup_suffix_files() {
        let dir = temp_path("skip-backups");
        fs::create_dir_all(&dir).expect("create dir");
        let source = dir.join("source.c");
        let backup = dir.join("skip.c.bak");
        fs::write(&source, "int source(){return 0;}\n").expect("write source");
        fs::write(&backup, "int backup(){return 0;}\n").expect("write backup");
        let console = ConsoleOptions {
            backup_suffix: Some(".bak".to_string()),
            ..ConsoleOptions::default()
        };

        format_paths(&[dir.join("*.c*")], false, &console).expect("format wildcard");

        assert_ne!(
            fs::read_to_string(&source).expect("read source"),
            "int source(){return 0;}\n"
        );
        assert_eq!(
            fs::read_to_string(&backup).expect("read backup"),
            "int backup(){return 0;}\n"
        );
        fs::remove_dir_all(dir).expect("remove dir");
    }

    #[test]
    fn direct_targets_with_active_backup_suffix_still_format() {
        let dir = temp_path("direct-backup-target");
        fs::create_dir_all(&dir).expect("create dir");
        let backup = dir.join("direct.c.bak");
        fs::write(&backup, "int backup(){return 0;}\n").expect("write backup");
        let console = ConsoleOptions {
            backup_suffix: Some(".bak".to_string()),
            ..ConsoleOptions::default()
        };

        format_paths(std::slice::from_ref(&backup), false, &console)
            .expect("format direct backup target");

        assert_ne!(
            fs::read_to_string(&backup).expect("read backup"),
            "int backup(){return 0;}\n"
        );
        assert!(backup.with_file_name("direct.c.bak.bak").is_file());
        fs::remove_dir_all(dir).expect("remove dir");
    }

    #[test]
    fn recursive_exclude_matches_an_empty_directory() {
        let root = temp_path("empty-excluded-dir");
        let skip = root.join("skip");
        let keep = root.join("keep");
        fs::create_dir_all(&skip).expect("create excluded dir");
        fs::create_dir_all(&keep).expect("create included dir");
        fs::write(keep.join("source.c"), "int value;\n").expect("write source");
        let console = ConsoleOptions {
            dry_run: true,
            excludes: vec!["skip".to_string()],
            ..ConsoleOptions::default()
        };

        let result = format_paths(&[root.join("*.c")], true, &console);

        fs::remove_dir_all(root).expect("remove target dirs");
        result.expect("empty excluded directory must count as matched");
    }

    #[test]
    fn exclude_matching_only_files_outside_the_target_mask_is_unmatched() {
        let root = temp_path("exclude-outside-mask");
        fs::create_dir_all(&root).expect("create target dir");
        fs::write(root.join("source.c"), "int value;\n").expect("write source");
        fs::write(root.join("notes.txt"), "notes\n").expect("write non-target");

        for exclude in ["notes.txt", "*.txt"] {
            let console = ConsoleOptions {
                dry_run: true,
                excludes: vec![exclude.to_string()],
                ..ConsoleOptions::default()
            };
            let error = format_paths(&[root.join("*.c")], false, &console)
                .expect_err("non-target exclude must be unmatched");
            assert!(error.to_string().contains("unmatched exclude"), "{error}");
        }

        fs::remove_dir_all(root).expect("remove target dir");
    }

    #[test]
    fn exclude_does_not_hide_an_explicit_file_target() {
        let path = temp_path("explicit-exclude.c");
        fs::write(&path, "int value;\n").expect("write source");
        let console = ConsoleOptions {
            dry_run: true,
            excludes: vec![
                path.file_name()
                    .expect("file name")
                    .to_string_lossy()
                    .into_owned(),
            ],
            ..ConsoleOptions::default()
        };

        let result = format_paths(std::slice::from_ref(&path), false, &console);

        fs::remove_file(path).expect("remove source");
        let error = result.expect_err("exclude must be unmatched for an explicit target");
        assert!(error.to_string().contains("unmatched exclude"), "{error}");
    }

    #[test]
    fn wildcard_with_every_matching_file_excluded_reports_no_file() {
        let root = temp_path("all-files-excluded");
        fs::create_dir_all(&root).expect("create target dir");
        fs::write(root.join("skip.c"), "int value;\n").expect("write excluded source");
        let console = ConsoleOptions {
            dry_run: true,
            excludes: vec!["skip.c".to_string()],
            ..ConsoleOptions::default()
        };

        let result = format_paths(&[root.join("*.c")], false, &console);

        fs::remove_dir_all(root).expect("remove target dir");
        let error = result.expect_err("fully excluded wildcard must fail");
        assert!(error.to_string().contains("no file to process"), "{error}");
    }
}
