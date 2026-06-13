use super::CliError;
use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn mark_matching_excludes(path: &Path, excludes: &[String], matched: &mut [bool]) -> bool {
    let mut excluded = false;
    for (index, pattern) in excludes.iter().enumerate() {
        if exclude_matches(pattern, path) {
            matched[index] = true;
            excluded = true;
        }
    }
    excluded
}

fn exclude_matches(pattern: &str, path: &Path) -> bool {
    let path_text = path.to_string_lossy();
    let path_text = normalize_path_separators(&path_text);
    let pattern = normalize_path_separators(pattern);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if has_wildcard(&pattern) {
        wildcard_match(&pattern, file_name) || wildcard_match(&pattern, &path_text)
    } else {
        path_segments_contain(&path_text, &pattern)
    }
}

fn normalize_path_separators(value: &str) -> Cow<'_, str> {
    if std::path::MAIN_SEPARATOR == '/' {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(value.replace(std::path::MAIN_SEPARATOR, "/"))
    }
}

/// Matches `pattern` against `path` aligned to directory separators: the pattern
/// must start at the path start or after a separator and end at the path end or
/// before a separator, so a directory or file name is matched whole, never a
/// partial segment (`lib` does not match `mylib`).
fn path_segments_contain(path: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    let separator = std::path::MAIN_SEPARATOR;
    path.match_indices(pattern).any(|(start, matched)| {
        let end = start + matched.len();
        let boundary_before =
            start == 0 || path[..start].ends_with(separator) || path[..start].ends_with('/');
        let boundary_after =
            end == path.len() || path[end..].starts_with(separator) || path[end..].starts_with('/');
        boundary_before && boundary_after
    })
}

pub(super) fn expand_target_paths_with_excludes(
    paths: &[PathBuf],
    recursive: bool,
    accept_empty_list: bool,
    backup_suffix: Option<&str>,
    excludes: &[String],
    matched_excludes: &mut [bool],
) -> Result<Vec<PathBuf>, CliError> {
    let mut expanded = Vec::new();
    for path in paths {
        expand_target_path(
            path,
            recursive,
            accept_empty_list,
            backup_suffix,
            excludes,
            matched_excludes,
            &mut expanded,
        )?;
    }
    Ok(expanded)
}

fn expand_target_path(
    path: &Path,
    recursive: bool,
    accept_empty_list: bool,
    backup_suffix: Option<&str>,
    excludes: &[String],
    matched_excludes: &mut [bool],
    expanded: &mut Vec<PathBuf>,
) -> Result<(), CliError> {
    let filename = path
        .file_name()
        .ok_or_else(|| CliError::new(format!("missing filename in {}", path.display()), 2))?;
    if !os_contains_pattern_char(filename) {
        if recursive {
            return Err(CliError::new("recursive option with no wildcard", 2));
        }
        expanded.push(path.to_path_buf());
        return Ok(());
    }

    let target = path.to_string_lossy();
    let (directory, filename) = split_target_path(path)?;
    let patterns = split_target_patterns(&filename)?;
    let has_wildcard = patterns.iter().any(|pattern| target_has_wildcard(pattern));

    if recursive && !has_wildcard {
        return Err(CliError::new("recursive option with no wildcard", 2));
    }

    if has_wildcard || patterns.len() > 1 {
        let start_len = expanded.len();
        collect_matching_files(
            &directory,
            &patterns,
            recursive,
            backup_suffix,
            excludes,
            matched_excludes,
            expanded,
        )?;
        if expanded.len() == start_len && !(accept_empty_list && has_wildcard) {
            return Err(CliError::new(format!("no file to process {target}"), 1));
        }
    } else {
        expanded.push(directory.join(&patterns[0]));
    }
    Ok(())
}

#[cfg(unix)]
fn os_contains_pattern_char(value: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;

    value
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'*' | b'?' | b',' | b';'))
}

#[cfg(windows)]
fn os_contains_pattern_char(value: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;

    value
        .encode_wide()
        .any(|unit| matches!(unit, 0x2a | 0x3f | 0x2c | 0x3b))
}

#[cfg(not(any(unix, windows)))]
fn os_contains_pattern_char(value: &OsStr) -> bool {
    value
        .to_string_lossy()
        .chars()
        .any(|ch| matches!(ch, '*' | '?' | ',' | ';'))
}

fn split_target_path(path: &Path) -> Result<(PathBuf, OsString), CliError> {
    let filename = path
        .file_name()
        .ok_or_else(|| CliError::new(format!("missing filename in {}", path.display()), 2))?;
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    Ok((directory, filename.to_os_string()))
}

fn split_utf8_target_patterns(filename: &OsStr) -> Option<Vec<OsString>> {
    Some(
        filename
            .to_str()?
            .split([',', ';'])
            .map(str::trim)
            .filter(|pattern| !pattern.is_empty())
            .map(OsString::from)
            .collect(),
    )
}

#[cfg(unix)]
fn split_target_patterns(filename: &OsStr) -> Result<Vec<OsString>, CliError> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    if let Some(patterns) = split_utf8_target_patterns(filename) {
        return finish_target_patterns(filename, patterns);
    }
    let patterns = filename
        .as_bytes()
        .split(|byte| matches!(byte, b',' | b';'))
        .map(|pattern| pattern.trim_ascii_start().trim_ascii_end().to_vec())
        .filter(|pattern| !pattern.is_empty())
        .map(OsString::from_vec)
        .collect::<Vec<_>>();
    finish_target_patterns(filename, patterns)
}

#[cfg(windows)]
fn split_target_patterns(filename: &OsStr) -> Result<Vec<OsString>, CliError> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    if let Some(patterns) = split_utf8_target_patterns(filename) {
        return finish_target_patterns(filename, patterns);
    }
    let units = filename.encode_wide().collect::<Vec<_>>();
    let patterns = units
        .split(|unit| matches!(*unit, 0x2c | 0x3b))
        .map(|pattern| {
            let start = pattern
                .iter()
                .position(|unit| !matches!(*unit, 0x20 | 0x09))
                .unwrap_or(pattern.len());
            let end = pattern
                .iter()
                .rposition(|unit| !matches!(*unit, 0x20 | 0x09))
                .map_or(start, |index| index + 1);
            OsString::from_wide(&pattern[start..end])
        })
        .filter(|pattern| !pattern.is_empty())
        .collect::<Vec<_>>();
    finish_target_patterns(filename, patterns)
}

#[cfg(not(any(unix, windows)))]
fn split_target_patterns(filename: &OsStr) -> Result<Vec<OsString>, CliError> {
    let patterns = filename
        .to_string_lossy()
        .split([',', ';'])
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .map(OsString::from)
        .collect::<Vec<_>>();
    finish_target_patterns(filename, patterns)
}

fn finish_target_patterns(
    filename: &OsStr,
    patterns: Vec<OsString>,
) -> Result<Vec<OsString>, CliError> {
    if patterns.is_empty() {
        Err(CliError::new(
            format!("missing filename in {}", Path::new(filename).display()),
            2,
        ))
    } else {
        Ok(patterns)
    }
}

fn has_wildcard(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

#[cfg(unix)]
fn target_has_wildcard(pattern: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;

    pattern
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, b'*' | b'?'))
}

#[cfg(windows)]
fn target_has_wildcard(pattern: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;

    pattern
        .encode_wide()
        .any(|unit| matches!(unit, 0x2a | 0x3f))
}

#[cfg(not(any(unix, windows)))]
fn target_has_wildcard(pattern: &OsStr) -> bool {
    has_wildcard(&pattern.to_string_lossy())
}

#[cfg(unix)]
fn os_starts_with_dot(value: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;

    value.as_bytes().starts_with(b".")
}

#[cfg(windows)]
fn os_starts_with_dot(value: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;

    value.encode_wide().next() == Some(u16::from(b'.'))
}

#[cfg(not(any(unix, windows)))]
fn os_starts_with_dot(value: &OsStr) -> bool {
    value.to_string_lossy().starts_with('.')
}

#[cfg(unix)]
fn os_ends_with(value: &OsStr, suffix: &str) -> bool {
    use std::os::unix::ffi::OsStrExt;

    value.as_bytes().ends_with(suffix.as_bytes())
}

#[cfg(windows)]
fn os_ends_with(value: &OsStr, suffix: &str) -> bool {
    use std::os::windows::ffi::OsStrExt;

    let value = value.encode_wide().collect::<Vec<_>>();
    let suffix = OsStr::new(suffix).encode_wide().collect::<Vec<_>>();
    value.ends_with(&suffix)
}

#[cfg(not(any(unix, windows)))]
fn os_ends_with(value: &OsStr, suffix: &str) -> bool {
    value.to_string_lossy().ends_with(suffix)
}

fn collect_matching_files(
    directory: &Path,
    patterns: &[OsString],
    recursive: bool,
    backup_suffix: Option<&str>,
    excludes: &[String],
    matched_excludes: &mut [bool],
    expanded: &mut Vec<PathBuf>,
) -> Result<(), CliError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| {
            CliError::new(
                format!("failed to read directory {}: {error}", directory.display()),
                1,
            )
        })?
        .collect::<Result<Vec<_>, io::Error>>()
        .map_err(|error| {
            CliError::new(
                format!("failed to read directory {}: {error}", directory.display()),
                1,
            )
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    let mut subdirectories = Vec::new();
    for entry in entries {
        let name = entry.file_name();
        if os_starts_with_dot(&name) {
            continue;
        }

        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            CliError::new(format!("failed to read {}: {error}", path.display()), 1)
        })?;
        if file_type.is_dir() {
            if mark_matching_excludes(&path, excludes, matched_excludes) {
                continue;
            }
            if recursive {
                let metadata = entry.metadata().map_err(|error| {
                    CliError::new(format!("failed to read {}: {error}", path.display()), 1)
                })?;
                if is_user_writable(&metadata) {
                    subdirectories.push(path);
                }
            }
            continue;
        }
        if !file_type.is_file()
            || backup_suffix.is_some_and(|suffix| os_ends_with(&name, suffix))
            || !patterns
                .iter()
                .any(|pattern| target_wildcard_match(pattern, &name))
        {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| {
            CliError::new(format!("failed to read {}: {error}", path.display()), 1)
        })?;
        if !is_user_writable(&metadata) || mark_matching_excludes(&path, excludes, matched_excludes)
        {
            continue;
        }
        expanded.push(path);
    }

    for directory in subdirectories {
        collect_matching_files(
            &directory,
            patterns,
            recursive,
            backup_suffix,
            excludes,
            matched_excludes,
            expanded,
        )?;
    }
    Ok(())
}

#[cfg(unix)]
fn is_user_writable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o200 != 0
}

#[cfg(not(unix))]
fn is_user_writable(metadata: &fs::Metadata) -> bool {
    !metadata.permissions().readonly()
}

#[cfg(unix)]
fn target_wildcard_match(pattern: &OsStr, text: &OsStr) -> bool {
    use std::os::unix::ffi::OsStrExt;

    if let (Some(pattern), Some(text)) = (pattern.to_str(), text.to_str()) {
        return wildcard_match(pattern, text);
    }
    wildcard_match_sequence(pattern.as_bytes(), text.as_bytes(), b'*', b'?')
}

#[cfg(windows)]
fn target_wildcard_match(pattern: &OsStr, text: &OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;

    if let (Some(pattern), Some(text)) = (pattern.to_str(), text.to_str()) {
        return wildcard_match(pattern, text);
    }
    wildcard_match_sequence(
        &pattern.encode_wide().collect::<Vec<_>>(),
        &text.encode_wide().collect::<Vec<_>>(),
        0x2a,
        0x3f,
    )
}

#[cfg(not(any(unix, windows)))]
fn target_wildcard_match(pattern: &OsStr, text: &OsStr) -> bool {
    wildcard_match(&pattern.to_string_lossy(), &text.to_string_lossy())
}

fn wildcard_match_sequence<T: Copy + Eq>(pattern: &[T], text: &[T], star: T, question: T) -> bool {
    let mut pattern_index = 0;
    let mut text_index = 0;
    let mut star_index = None;
    let mut star_text_index = 0;

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == question || pattern[pattern_index] == text[text_index])
        {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == star {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_text_index = text_index;
        } else if let Some(star_index) = star_index {
            pattern_index = star_index + 1;
            star_text_index += 1;
            text_index = star_text_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == star {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    wildcard_match_sequence(
        &pattern.chars().collect::<Vec<_>>(),
        &text.chars().collect::<Vec<_>>(),
        '*',
        '?',
    )
}

pub(super) fn validate_target_path(path: &Path) -> Result<(), CliError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(CliError::new(
                format!("input file not found: {}", path.display()),
                1,
            ));
        }
        Err(error) => {
            return Err(CliError::new(
                format!("failed to inspect {}: {error}", path.display()),
                1,
            ));
        }
    };
    if metadata.is_dir() {
        return Err(CliError::new(
            format!("directory targets are not supported: {}", path.display()),
            2,
        ));
    }
    if !metadata.is_file() {
        return Err(CliError::new(
            format!("input file not found: {}", path.display()),
            1,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("cstyle-cli-{stamp}-{name}"))
    }

    fn expand_target_paths(
        paths: &[PathBuf],
        recursive: bool,
        accept_empty_list: bool,
        backup_suffix: Option<&str>,
    ) -> Result<Vec<PathBuf>, CliError> {
        expand_target_paths_with_excludes(
            paths,
            recursive,
            accept_empty_list,
            backup_suffix,
            &[],
            &mut [],
        )
    }

    #[test]
    fn every_exclude_matching_the_same_path_counts_as_matched() {
        let mut paths = vec![PathBuf::from("src/source.c")];
        let excludes = vec!["*.c".to_string(), "source.c".to_string()];
        let mut matched = vec![false; excludes.len()];

        paths.retain(|path| !mark_matching_excludes(path, &excludes, &mut matched));

        assert!(paths.is_empty());
        assert_eq!(matched, vec![true, true]);
    }

    #[test]
    fn exclude_matches_directory_segments_on_boundaries() {
        let path = PathBuf::from("src/firmware/lib/picolibc/iconv.c");
        assert!(exclude_matches("src/firmware/lib", &path));
        assert!(exclude_matches("firmware/lib", &path));
        assert!(exclude_matches("lib", &path));
        assert!(exclude_matches("picolibc", &path));
        assert!(exclude_matches("iconv.c", &path));

        assert!(!exclude_matches("ib", &path));
        assert!(!exclude_matches("lib/picolib", &path));
        assert!(!exclude_matches("lib", &PathBuf::from("src/mylib/y.c")));
    }

    #[test]
    fn partial_unicode_exclude_segment_does_not_panic_or_match() {
        assert!(!exclude_matches("α", &PathBuf::from("αβ/source.c")));
    }

    #[cfg(unix)]
    #[test]
    fn direct_non_utf8_target_does_not_require_pattern_decoding() {
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(b"source-\xff.c".to_vec()));

        let expanded = expand_target_paths(std::slice::from_ref(&path), false, false, None)
            .expect("expand direct path");

        assert_eq!(expanded, [path]);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_wildcard_pattern_can_match_an_empty_list() {
        use std::os::unix::ffi::OsStringExt;

        let dir = temp_path("non-utf8-wildcard");
        fs::create_dir_all(&dir).expect("create wildcard dir");
        let wildcard = OsString::from_vec(b"*-\xff.c".to_vec());
        let filename = OsString::from_vec(b"source-\xff.c".to_vec());

        let expanded = expand_target_paths(&[dir.join(&wildcard)], false, true, None)
            .expect("expand non-UTF-8 wildcard");

        fs::remove_dir_all(dir).expect("remove wildcard dir");
        assert!(expanded.is_empty());
        assert!(target_wildcard_match(&wildcard, &filename));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_wildcard_pattern_matches_a_non_utf8_file() {
        use std::os::unix::ffi::OsStringExt;

        let dir = temp_path("non-utf8-wildcard-file");
        fs::create_dir_all(&dir).expect("create wildcard dir");
        let wildcard = OsString::from_vec(b"*-\xff.c".to_vec());
        let path = dir.join(OsString::from_vec(b"source-\xff.c".to_vec()));
        fs::write(&path, "int value;\n").expect("write non-UTF-8 path");

        let expanded = expand_target_paths(&[dir.join(wildcard)], false, false, None)
            .expect("expand non-UTF-8 wildcard");

        fs::remove_dir_all(dir).expect("remove wildcard dir");
        assert_eq!(expanded, [path]);
    }

    #[test]
    fn expands_recursive_wildcard_targets() {
        let dir = temp_path("targets");
        let nested = dir.join("nested");
        let hidden = dir.join(".hidden");
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::create_dir_all(&hidden).expect("create hidden dir");
        fs::write(dir.join("a.c"), "int a;\n").expect("write c file");
        fs::write(nested.join("b.c"), "int b;\n").expect("write nested c file");
        fs::write(dir.join("a.h"), "int a;\n").expect("write h file");
        fs::write(nested.join("b.h"), "int b;\n").expect("write nested h file");
        fs::write(hidden.join("skip.c"), "int skip;\n").expect("write hidden c file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let readonly = dir.join("readonly.c");
            fs::write(&readonly, "int readonly;\n").expect("write readonly c file");
            let mut permissions = fs::metadata(&readonly)
                .expect("readonly metadata")
                .permissions();
            permissions.set_mode(0o444);
            fs::set_permissions(&readonly, permissions).expect("set readonly permissions");
        }

        let paths =
            expand_target_paths(&[dir.join("*.c"), dir.join("*.h")], true, false, None).unwrap();

        assert_eq!(
            paths,
            vec![
                dir.join("a.c"),
                nested.join("b.c"),
                dir.join("a.h"),
                nested.join("b.h"),
            ]
        );

        fs::remove_dir_all(dir).expect("remove target dir");
    }

    #[cfg(unix)]
    #[test]
    fn recursive_wildcards_do_not_follow_symlinked_directories() {
        let dir = temp_path("symlink-targets");
        let outside = temp_path("symlink-outside");
        fs::create_dir_all(&dir).expect("create target dir");
        fs::create_dir_all(&outside).expect("create outside dir");
        fs::write(dir.join("inside.c"), "int inside;\n").expect("write inside file");
        let outside_file = outside.join("escape.c");
        fs::write(&outside_file, "int escape;\n").expect("write outside file");
        std::os::unix::fs::symlink(&outside, dir.join("link")).expect("create directory symlink");
        std::os::unix::fs::symlink(&outside_file, dir.join("file-link.c"))
            .expect("create file symlink");

        let paths = expand_target_paths(&[dir.join("*.c")], true, false, None).unwrap();

        assert_eq!(paths, vec![dir.join("inside.c")]);
        fs::remove_dir_all(dir).expect("remove target dir");
        fs::remove_dir_all(outside).expect("remove outside dir");
    }

    #[test]
    fn rejects_recursive_without_wildcard() {
        let error = expand_target_paths(&[PathBuf::from("input.c")], true, false, None)
            .expect_err("recursive without wildcard must fail");

        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.to_string(), "recursive option with no wildcard");
    }

    #[cfg(unix)]
    #[test]
    fn target_validation_reports_permission_errors() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_path("target-permission");
        let locked = root.join("locked");
        let path = locked.join("input.c");
        fs::create_dir_all(&locked).expect("create target dir");
        fs::write(&path, "int value;\n").expect("write target");
        let mut permissions = fs::metadata(&locked)
            .expect("target metadata")
            .permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&locked, permissions).expect("lock target dir");

        let result = validate_target_path(&path);

        let mut permissions = fs::metadata(&locked)
            .expect("target metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&locked, permissions).expect("unlock target dir");
        fs::remove_dir_all(root).expect("remove target dir");
        let error = result.expect_err("inaccessible target must fail");
        assert!(error.to_string().contains("Permission denied"), "{error}");
    }

    #[test]
    fn validates_target_paths() {
        let file = temp_path("input.c");
        let dir = temp_path("input-dir");
        let missing = temp_path("missing.c");
        fs::write(&file, "int main(){}\n").expect("write input file");
        fs::create_dir_all(&dir).expect("create input dir");

        assert!(validate_target_path(&file).is_ok());
        assert_error(&dir, "directory targets are not supported");
        assert_error(&missing, "input file not found");

        fs::remove_file(file).expect("remove input file");
        fs::remove_dir_all(dir).expect("remove input dir");
    }

    fn assert_error(path: &Path, expected: &str) {
        let error = validate_target_path(path).expect_err("target must fail");
        assert!(error.to_string().contains(expected), "error: {error}");
    }
}
