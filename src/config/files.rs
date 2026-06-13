use super::parser::{apply_source, parse_source};
use super::{ASTYLE_CONFIG_FILE_NAME, CONFIG_FILE_NAME, ConfigError, FormatOptions};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub fn load_from_current_dir() -> Result<FormatOptions, ConfigError> {
    let current_dir = env::current_dir()
        .map_err(|error| ConfigError::new(format!("failed to read current directory: {error}")))?;
    load_from_dir(&current_dir)
}

pub fn load_from_dir(dir: &Path) -> Result<FormatOptions, ConfigError> {
    let path = dir.join(CONFIG_FILE_NAME);
    match fs::read_to_string(&path) {
        Ok(source) => parse_source(&path, &source),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let fallback_path = dir.join(ASTYLE_CONFIG_FILE_NAME);
            match fs::read_to_string(&fallback_path) {
                Ok(source) => parse_source(&fallback_path, &source),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    Ok(FormatOptions::default())
                }
                Err(error) => Err(ConfigError::io(&fallback_path, error)),
            }
        }
        Err(error) => Err(ConfigError::io(&path, error)),
    }
}

pub fn load_from_file(path: &Path) -> Result<FormatOptions, ConfigError> {
    let mut options = FormatOptions::default();
    apply_file(&mut options, path)?;
    Ok(options)
}

pub(crate) fn load_optional_file(path: &Path) -> Result<Option<FormatOptions>, ConfigError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ConfigError::io(path, error)),
    };
    parse_source(path, &source).map(Some)
}

pub fn apply_file(options: &mut FormatOptions, path: &Path) -> Result<(), ConfigError> {
    let source = fs::read_to_string(path).map_err(|error| ConfigError::io(path, error))?;
    let mut updated = options.clone();
    apply_source(path, &source, &mut updated)?;
    *options = updated;
    Ok(())
}

pub fn apply_project_file(
    options: &mut FormatOptions,
    name: impl AsRef<OsStr>,
    start_dir: &Path,
    required: bool,
) -> Result<(), ConfigError> {
    let name = name.as_ref();
    match find_project_file(name, start_dir)? {
        Some(path) => apply_file(options, &path),
        None if required => Err(ConfigError::new(format!(
            "cannot open project option file {}",
            Path::new(name).display()
        ))),
        None => Ok(()),
    }
}

pub fn find_project_file(
    name: impl AsRef<OsStr>,
    start_dir: &Path,
) -> Result<Option<PathBuf>, ConfigError> {
    let name = name.as_ref();
    if name.is_empty() {
        return Ok(None);
    }
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Ok(None);
    }

    let mut dir = if start_dir.is_absolute() {
        start_dir.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| {
                ConfigError::new(format!("failed to read current directory: {error}"))
            })?
            .join(start_dir)
    };
    match fs::metadata(&dir) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            dir.pop();
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            dir.pop();
        }
        Err(error) => return Err(ConfigError::io(&dir, error)),
    }
    match fs::canonicalize(&dir) {
        Ok(canonical) => dir = canonical,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(ConfigError::io(&dir, error)),
    }

    loop {
        let path = dir.join(name);
        if project_file_exists(&path)? {
            return Ok(Some(path));
        }
        if name == OsStr::new(ASTYLE_CONFIG_FILE_NAME) {
            let old_path = dir.join("_astylerc");
            if project_file_exists(&old_path)? {
                return Ok(Some(old_path));
            }
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

fn project_file_exists(path: &Path) -> Result<bool, ConfigError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(ConfigError::io(path, error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        let dir = env::temp_dir().join(format!("cstyle-config-{stamp}-{name}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn failed_option_file_application_leaves_options_unchanged() {
        let dir = temp_dir("atomic-apply");
        let path = dir.join("options.rc");
        fs::write(&path, "pad-oper\nunknown-option\n").expect("write options");
        let mut options = FormatOptions::default();
        let original = options.clone();

        let result = apply_file(&mut options, &path);
        fs::remove_dir_all(dir).expect("remove temp dir");

        assert!(result.is_err());
        assert_eq!(options, original);
    }

    #[test]
    fn load_from_dir_prefers_cstylerc_and_falls_back_to_astylerc() {
        let dir = temp_dir("config-dir");
        let child = dir.join("child");
        fs::create_dir_all(&child).expect("create child dir");
        fs::write(dir.join(CONFIG_FILE_NAME), "indent=spaces=2\n").expect("write cstyle config");
        fs::write(dir.join(ASTYLE_CONFIG_FILE_NAME), "indent=spaces=3\n")
            .expect("write legacy config");
        fs::write(child.join(ASTYLE_CONFIG_FILE_NAME), "indent=spaces=4\n")
            .expect("write child config");
        fs::write(dir.join("_astylerc"), "indent=spaces=5\n").expect("write old config");

        let parent_options = load_from_dir(&dir).expect("load parent config");
        let child_options = load_from_dir(&child).expect("load child config");

        assert_eq!(parent_options.indent_width, 2);
        assert_eq!(child_options.indent_width, 4);
        fs::remove_dir_all(dir).expect("remove temp dir");
    }

    #[test]
    fn project_search_rejects_paths_instead_of_file_names() {
        let root = temp_dir("project-name");
        let nested = root.join("nested");
        fs::create_dir(&nested).expect("create nested dir");
        fs::write(nested.join("options.rc"), "style=allman\n").expect("write options");

        let result = find_project_file("nested/options.rc", &root).expect("search project file");

        fs::remove_dir_all(root).expect("remove temp dir");
        assert_eq!(result, None);
    }

    #[cfg(unix)]
    #[test]
    fn config_lookup_reports_inaccessible_directory() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_dir("config-permission");
        let locked = root.join("locked");
        fs::create_dir(&locked).expect("create config dir");
        let mut permissions = fs::metadata(&locked)
            .expect("locked metadata")
            .permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&locked, permissions).expect("lock config dir");

        let result = load_from_dir(&locked);

        let mut permissions = fs::metadata(&locked)
            .expect("locked metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&locked, permissions).expect("unlock config dir");
        fs::remove_dir_all(root).expect("remove temp dir");
        assert!(result.is_err(), "inaccessible config path must fail");
    }

    #[cfg(unix)]
    #[test]
    fn project_search_reports_inaccessible_start_directory() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_dir("project-permission");
        let locked = root.join("locked");
        let child = locked.join("child");
        fs::create_dir_all(&child).expect("create project dirs");
        fs::write(root.join(ASTYLE_CONFIG_FILE_NAME), "indent=spaces=2\n")
            .expect("write parent project options");
        let mut permissions = fs::metadata(&locked)
            .expect("locked metadata")
            .permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&locked, permissions).expect("lock project dir");

        let result = find_project_file(ASTYLE_CONFIG_FILE_NAME, &child);

        let mut permissions = fs::metadata(&locked)
            .expect("locked metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&locked, permissions).expect("unlock project dir");
        fs::remove_dir_all(root).expect("remove temp dir");
        assert!(result.is_err(), "inaccessible search path must fail");
    }

    #[test]
    fn load_from_file_reads_explicit_config_path() {
        let dir = temp_dir("config-file");
        let path = dir.join(".astylerc");
        fs::write(&path, "indent=spaces=3\n").expect("write explicit config");

        let options = load_from_file(&path).expect("load explicit config");

        assert_eq!(options.indent_width, 3);
        fs::remove_dir_all(dir).expect("remove temp dir");
    }
}
