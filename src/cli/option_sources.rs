use super::args::{ConfigSelection, ProjectConfigSelection};
use crate::config::{self, ConfigFileOptions};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const CSTYLE_OPTIONS_ENV: &str = "CSTYLE_OPTIONS";
const ASTYLE_OPTIONS_ENV: &str = "ARTISTIC_STYLE_OPTIONS";
const CSTYLE_PROJECT_OPTIONS_ENV: &str = "CSTYLE_PROJECT_OPTIONS";
const ASTYLE_PROJECT_OPTIONS_ENV: &str = "ARTISTIC_STYLE_PROJECT_OPTIONS";

pub(super) fn load_selected_config(
    selection: &ConfigSelection,
    get_env: &impl Fn(&'static str) -> Option<OsString>,
) -> Result<ConfigFileOptions, config::ConfigError> {
    match selection {
        ConfigSelection::Auto => load_auto_config(get_env),
        ConfigSelection::File(path) => config::load_config_file(path),
        ConfigSelection::None => Ok(ConfigFileOptions::default()),
    }
}

pub(super) fn apply_selected_project_config(
    options: &mut ConfigFileOptions,
    selection: &ProjectConfigSelection,
    paths: &[PathBuf],
    stdin_path: Option<&Path>,
    get_env: &impl Fn(&'static str) -> Option<OsString>,
) -> Result<(), config::ConfigError> {
    match selection {
        ProjectConfigSelection::Auto => {
            if let Some(name) = env_fallback(
                get_env,
                CSTYLE_PROJECT_OPTIONS_ENV,
                ASTYLE_PROJECT_OPTIONS_ENV,
            ) {
                if name == OsStr::new("none") {
                    return Ok(());
                }
                config::apply_project_config_file(
                    options,
                    &name,
                    project_start_dir(paths, stdin_path),
                    false,
                )
            } else {
                Ok(())
            }
        }
        ProjectConfigSelection::FileName(name) => config::apply_project_config_file(
            options,
            name,
            project_start_dir(paths, stdin_path),
            true,
        ),
        ProjectConfigSelection::None => Ok(()),
    }
}

fn load_auto_config(
    get_env: &impl Fn(&'static str) -> Option<OsString>,
) -> Result<ConfigFileOptions, config::ConfigError> {
    if let Some(path) = env_fallback(get_env, CSTYLE_OPTIONS_ENV, ASTYLE_OPTIONS_ENV) {
        return config::load_config_file(&PathBuf::from(path));
    }
    for name in [config::CONFIG_FILE_NAME, config::ASTYLE_CONFIG_FILE_NAME] {
        if let Some(options) = config::load_optional_config_file(Path::new(name))? {
            return Ok(options);
        }
    }
    if let Some(home) = get_env("HOME") {
        let path = PathBuf::from(home).join(config::ASTYLE_CONFIG_FILE_NAME);
        if let Some(options) = config::load_optional_config_file(&path)? {
            return Ok(options);
        }
    }
    Ok(ConfigFileOptions::default())
}

fn env_fallback(
    get_env: &impl Fn(&'static str) -> Option<OsString>,
    primary: &'static str,
    fallback: &'static str,
) -> Option<OsString> {
    get_env(primary)
        .filter(|value| !value.is_empty())
        .or_else(|| get_env(fallback).filter(|value| !value.is_empty()))
}

fn project_start_dir<'a>(paths: &'a [PathBuf], stdin_path: Option<&'a Path>) -> &'a Path {
    paths
        .first()
        .map(PathBuf::as_path)
        .or(stdin_path)
        .and_then(|path| path.parent())
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
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

    #[cfg(unix)]
    #[test]
    fn auto_config_reports_inaccessible_home_options_path() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_path("home-options-permission");
        let home = root.join("home");
        fs::create_dir_all(&home).expect("create home dir");
        let mut permissions = fs::metadata(&home).expect("home metadata").permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&home, permissions).expect("lock home dir");
        let get_env = |name| match name {
            "HOME" => Some(home.clone().into_os_string()),
            _ => None,
        };

        let result = load_auto_config(&get_env);

        let mut permissions = fs::metadata(&home).expect("home metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&home, permissions).expect("unlock home dir");
        fs::remove_dir_all(root).expect("remove home dir");
        assert!(result.is_err(), "inaccessible config lookup must fail");
    }

    #[test]
    fn auto_config_falls_back_to_home_legacy_options() {
        let dir = temp_path("home-options");
        fs::create_dir_all(&dir).expect("create home options dir");
        fs::write(
            dir.join(config::ASTYLE_CONFIG_FILE_NAME),
            "indent=spaces=6\n",
        )
        .expect("write home legacy rc");
        let get_env = |name| match name {
            "HOME" => Some(dir.clone().into_os_string()),
            _ => None,
        };

        let options = load_auto_config(&get_env).expect("load home legacy rc");

        assert_eq!(options.format.indent_width, 6);
        fs::remove_dir_all(dir).expect("remove home options dir");
    }

    #[test]
    fn empty_primary_options_environment_uses_nonempty_fallback() {
        let dir = temp_path("empty-primary-env-options");
        fs::create_dir_all(&dir).expect("create env dir");
        let fallback = dir.join("fallback.rc");
        fs::write(&fallback, "indent=spaces=5\n").expect("write fallback options");
        let get_env = |name| match name {
            CSTYLE_OPTIONS_ENV => Some(OsString::new()),
            ASTYLE_OPTIONS_ENV => Some(fallback.clone().into_os_string()),
            _ => None,
        };

        let options =
            load_selected_config(&ConfigSelection::Auto, &get_env).expect("load fallback options");

        assert_eq!(options.format.indent_width, 5);
        fs::remove_dir_all(dir).expect("remove env dir");
    }

    #[test]
    fn loads_cstyle_env_options_before_artistic_style_fallback() {
        let dir = temp_path("env-options");
        fs::create_dir_all(&dir).expect("create env dir");
        let cstyle_path = dir.join("cstyle.rc");
        let legacy_path = dir.join("legacy.rc");
        fs::write(&cstyle_path, "indent=spaces=3\n").expect("write cstyle rc");
        fs::write(&legacy_path, "indent=spaces=4\n").expect("write legacy rc");
        let get_env = |name| match name {
            CSTYLE_OPTIONS_ENV => Some(cstyle_path.clone().into_os_string()),
            ASTYLE_OPTIONS_ENV => Some(legacy_path.clone().into_os_string()),
            _ => None,
        };

        let options = load_selected_config(&ConfigSelection::Auto, &get_env).unwrap();

        assert_eq!(options.format.indent_width, 3);
        fs::remove_dir_all(dir).expect("remove env dir");
    }

    #[test]
    fn loads_artistic_style_env_options_as_fallback() {
        let dir = temp_path("fallback-env-options");
        fs::create_dir_all(&dir).expect("create env dir");
        let path = dir.join("legacy.rc");
        fs::write(&path, "indent=spaces=5\n").expect("write legacy rc");
        let get_env = |name| match name {
            ASTYLE_OPTIONS_ENV => Some(path.clone().into_os_string()),
            _ => None,
        };

        let options = load_selected_config(&ConfigSelection::Auto, &get_env).unwrap();

        assert_eq!(options.format.indent_width, 5);
        fs::remove_dir_all(dir).expect("remove env dir");
    }

    #[test]
    fn loads_cstyle_project_env_options_before_artistic_style_fallback() {
        let dir = temp_path("project-env");
        let child = dir.join("src");
        fs::create_dir_all(&child).expect("create project dirs");
        fs::write(dir.join("cstyle-project.rc"), "indent=spaces=6\n")
            .expect("write cstyle project rc");
        fs::write(dir.join("legacy-project.rc"), "indent=spaces=7\n")
            .expect("write legacy project rc");
        let get_env = |name| match name {
            CSTYLE_PROJECT_OPTIONS_ENV => Some(OsString::from("cstyle-project.rc")),
            ASTYLE_PROJECT_OPTIONS_ENV => Some(OsString::from("legacy-project.rc")),
            _ => None,
        };
        let mut options = ConfigFileOptions::default();
        apply_selected_project_config(
            &mut options,
            &ProjectConfigSelection::Auto,
            &[child.join("*.c")],
            None,
            &get_env,
        )
        .unwrap();

        assert_eq!(options.format.indent_width, 6);
        fs::remove_dir_all(dir).expect("remove project dir");
    }

    #[test]
    fn loads_artistic_style_project_env_options_as_fallback() {
        let dir = temp_path("project-env-fallback");
        let child = dir.join("src");
        fs::create_dir_all(&child).expect("create project dirs");
        fs::write(dir.join("legacy-project.rc"), "indent=spaces=7\n")
            .expect("write legacy project rc");
        let get_env = |name| match name {
            ASTYLE_PROJECT_OPTIONS_ENV => Some(OsString::from("legacy-project.rc")),
            _ => None,
        };
        let mut options = ConfigFileOptions::default();
        apply_selected_project_config(
            &mut options,
            &ProjectConfigSelection::Auto,
            &[child.join("*.c")],
            None,
            &get_env,
        )
        .unwrap();

        assert_eq!(options.format.indent_width, 7);
        fs::remove_dir_all(dir).expect("remove project dir");
    }

    #[test]
    fn project_none_disables_project_env_options() {
        let dir = temp_path("project-env-none");
        fs::create_dir_all(&dir).expect("create project dir");
        fs::write(dir.join("project.rc"), "indent=spaces=6\n").expect("write project rc");
        let get_env = |name| match name {
            CSTYLE_PROJECT_OPTIONS_ENV => Some(OsString::from("project.rc")),
            _ => None,
        };
        let mut options = ConfigFileOptions::default();
        apply_selected_project_config(
            &mut options,
            &ProjectConfigSelection::None,
            &[dir.join("*.c")],
            None,
            &get_env,
        )
        .unwrap();

        assert_eq!(options, ConfigFileOptions::default());
        fs::remove_dir_all(dir).expect("remove project dir");
    }

    #[test]
    fn project_option_requires_existing_project_file() {
        let dir = temp_path("missing-project");
        fs::create_dir_all(&dir).expect("create project dir");
        let get_env = |_name| None;
        let mut options = ConfigFileOptions::default();
        let error = apply_selected_project_config(
            &mut options,
            &ProjectConfigSelection::FileName(OsString::from("missing.rc")),
            &[dir.join("*.c")],
            None,
            &get_env,
        )
        .expect_err("required project file must fail");

        assert_eq!(
            error.to_string(),
            "cannot open project option file missing.rc"
        );
        fs::remove_dir_all(dir).expect("remove project dir");
    }

    #[test]
    fn project_search_uses_stdin_path_when_no_file_targets_exist() {
        let dir = temp_path("stdin-project");
        let child = dir.join("src");
        fs::create_dir_all(&child).expect("create project dirs");
        fs::write(dir.join("project.rc"), "indent=spaces=6\n").expect("write project rc");
        let get_env = |_name| None;
        let mut options = ConfigFileOptions::default();
        apply_selected_project_config(
            &mut options,
            &ProjectConfigSelection::FileName(OsString::from("project.rc")),
            &[],
            Some(&child.join("input.c")),
            &get_env,
        )
        .unwrap();

        assert_eq!(options.format.indent_width, 6);
        fs::remove_dir_all(dir).expect("remove project dir");
    }

    #[test]
    fn project_option_finds_legacy_astylerc_name() {
        let dir = temp_path("legacy-project");
        let child = dir.join("src");
        fs::create_dir_all(&child).expect("create project dirs");
        fs::write(dir.join("_astylerc"), "indent=spaces=8\n").expect("write legacy project rc");
        let get_env = |_name| None;
        let mut options = ConfigFileOptions::default();
        apply_selected_project_config(
            &mut options,
            &ProjectConfigSelection::FileName(OsString::from(".astylerc")),
            &[child.join("*.c")],
            None,
            &get_env,
        )
        .unwrap();

        assert_eq!(options.format.indent_width, 8);
        fs::remove_dir_all(dir).expect("remove project dir");
    }
}
