//! Order: `$XDG_CONFIG_HOME/umbriel/config.toml` (else `$HOME/.config`), then
//! each `$XDG_CONFIG_DIRS` entry (default `/etc/xdg`), then each
//! `$XDG_DATA_DIRS` entry for the packaged default. The compositor checks only
//! its compile-time data dir; we scan the XDG data dirs (default
//! `/usr/local/share:/usr/share`) so both install styles are covered.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Config location relative to a config dir, e.g. `umbriel/config.toml`.
const CONFIG_RELATIVE_PATH: &str = "umbriel/config.toml";

/// Environment inputs to the lookup, captured explicitly so tests never
/// mutate process environment variables.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub xdg_config_home: Option<OsString>,
    pub xdg_config_dirs: Option<OsString>,
    pub xdg_data_dirs: Option<OsString>,
    pub xdg_state_home: Option<OsString>,
    pub home: Option<OsString>,
}

impl Env {
    /// Capture the current process environment.
    pub fn from_process() -> Self {
        fn get(key: &str) -> Option<OsString> {
            std::env::var_os(key).filter(|value| !value.is_empty())
        }
        Self {
            xdg_config_home: get("XDG_CONFIG_HOME"),
            xdg_config_dirs: get("XDG_CONFIG_DIRS"),
            xdg_data_dirs: get("XDG_DATA_DIRS"),
            xdg_state_home: get("XDG_STATE_HOME"),
            home: get("HOME"),
        }
    }
}

/// First (user-writable) candidate, mirroring the compositor's `userConfigPath`.
fn user_config_path(env: &Env) -> PathBuf {
    if let Some(dir) = &env.xdg_config_home {
        return Path::new(dir).join(CONFIG_RELATIVE_PATH);
    }
    if let Some(home) = &env.home {
        return Path::new(home).join(".config").join(CONFIG_RELATIVE_PATH);
    }
    Path::new(".config").join(CONFIG_RELATIVE_PATH)
}

/// Colon-split an XDG-style variable; empty segments are skipped.
/// Unix-only, like the compositor's own splitting.
fn split_dirs(value: &OsStr) -> Vec<PathBuf> {
    value
        .to_string_lossy()
        .split(':')
        .filter(|segment| !segment.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// All candidates in Umbriel's precedence order.
pub fn candidates(env: &Env) -> Vec<PathBuf> {
    let mut list = vec![user_config_path(env)];

    let config_dirs = env
        .xdg_config_dirs
        .as_deref()
        .unwrap_or_else(|| OsStr::new("/etc/xdg"));
    for dir in split_dirs(config_dirs) {
        list.push(dir.join(CONFIG_RELATIVE_PATH));
    }

    list.extend(data_dir_candidates(env));
    list
}

fn data_dir_candidates(env: &Env) -> Vec<PathBuf> {
    let data_dirs = env
        .xdg_data_dirs
        .as_deref()
        .unwrap_or_else(|| OsStr::new("/usr/local/share:/usr/share"));
    split_dirs(data_dirs)
        .into_iter()
        .map(|dir| dir.join(CONFIG_RELATIVE_PATH))
        .collect()
}

/// The installed packaged default config, when a data dir candidate exists.
pub fn packaged_default(env: &Env) -> Option<PathBuf> {
    data_dir_candidates(env)
        .into_iter()
        .find(|path| path.is_file())
}

/// The config a running Umbriel would load: the first candidate that exists,
/// else the user path (where a new config would be created).
pub fn resolve(env: &Env) -> PathBuf {
    candidates(env)
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| user_config_path(env))
}

/// `resolve` against the real process environment.
pub fn resolve_process() -> PathBuf {
    resolve(&Env::from_process())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(config_home: Option<&str>, config_dirs: Option<&str>, home: Option<&str>) -> Env {
        Env {
            xdg_config_home: config_home.map(OsString::from),
            xdg_config_dirs: config_dirs.map(OsString::from),
            xdg_data_dirs: None,
            xdg_state_home: None,
            home: home.map(OsString::from),
        }
    }

    fn paths(env: &Env) -> Vec<String> {
        candidates(env)
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn default_env_falls_back_to_home_then_system_dirs() {
        let list = paths(&env(None, None, Some("/home/tester")));
        assert_eq!(
            list,
            vec![
                "/home/tester/.config/umbriel/config.toml",
                "/etc/xdg/umbriel/config.toml",
                "/usr/local/share/umbriel/config.toml",
                "/usr/share/umbriel/config.toml",
            ]
        );
    }

    #[test]
    fn xdg_config_home_wins_over_home() {
        let list = paths(&env(Some("/custom/cfg"), None, Some("/home/tester")));
        assert_eq!(list[0], "/custom/cfg/umbriel/config.toml");
    }

    #[test]
    fn missing_home_yields_relative_user_path() {
        let list = paths(&env(None, None, None));
        assert_eq!(list[0], ".config/umbriel/config.toml");
    }

    #[test]
    fn config_dirs_split_in_order_and_skip_empty_segments() {
        let e = Env {
            xdg_config_dirs: Some("/a::/b".into()),
            ..env(None, None, Some("/h"))
        };
        let list = paths(&e);
        assert_eq!(list[1], "/a/umbriel/config.toml");
        assert_eq!(list[2], "/b/umbriel/config.toml");
        assert_eq!(list.len(), 5); // 1 user + 2 config dirs + 2 default data dirs
    }

    #[test]
    fn data_dirs_override_the_defaults() {
        let e = Env {
            xdg_data_dirs: Some("/opt/share:/srv/share".into()),
            ..env(None, None, Some("/h"))
        };
        let list = paths(&e);
        assert_eq!(list[2], "/opt/share/umbriel/config.toml");
        assert_eq!(list[3], "/srv/share/umbriel/config.toml");
        assert_eq!(list.len(), 4);
    }

    #[test]
    fn resolve_prefers_first_existing_candidate() {
        let root = std::env::temp_dir().join(format!("umbriel-discovery-{}", std::process::id()));
        let cfg_dir = root.join("cfg");
        let config = cfg_dir.join("umbriel/config.toml");
        std::fs::create_dir_all(cfg_dir.join("umbriel")).unwrap();
        std::fs::write(&config, b"").unwrap();

        let e = env(Some(cfg_dir.to_str().unwrap()), None, None);
        assert_eq!(resolve(&e), config);

        let missing = Env {
            xdg_data_dirs: Some("/nonexistent-umbriel-data".into()),
            ..env(Some("/nonexistent-umbriel-test"), None, None)
        };
        assert_eq!(
            resolve(&missing),
            PathBuf::from("/nonexistent-umbriel-test/umbriel/config.toml")
        );

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn packaged_default_is_first_existing_data_dir_candidate() {
        let root = std::env::temp_dir().join(format!("umbriel-packaged-{}", std::process::id()));
        let share = root.join("share");
        std::fs::create_dir_all(share.join("umbriel")).unwrap();
        std::fs::write(share.join("umbriel/config.toml"), b"").unwrap();

        let found = Env {
            xdg_data_dirs: Some(share.clone().into_os_string()),
            ..env(None, None, None)
        };
        assert_eq!(
            packaged_default(&found),
            Some(share.join("umbriel/config.toml"))
        );

        let missing = Env {
            xdg_data_dirs: Some("/nonexistent-umbriel-data".into()),
            ..env(None, None, None)
        };
        assert_eq!(packaged_default(&missing), None);

        std::fs::remove_dir_all(&root).ok();
    }
}
