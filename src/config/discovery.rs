//! The config we edit is always the user's own:
//! `$XDG_CONFIG_HOME/umbriel/config.toml` (else `$HOME/.config`). Umbriel
//! falls back to `$XDG_CONFIG_DIRS` and its packaged default when that file
//! is missing, but those are system files we can't write, so a missing user
//! file is a fresh start instead. The packaged default is found through the
//! `$XDG_DATA_DIRS` entries: the compositor checks only its compile-time data
//! dir; we scan the XDG data dirs (default `/usr/local/share:/usr/share`) so
//! both install styles are covered.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// Config location relative to a config dir, e.g. `umbriel/config.toml`.
const CONFIG_RELATIVE_PATH: &str = "umbriel/config.toml";

/// Environment inputs to the lookup, captured explicitly so tests never
/// mutate process environment variables.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub xdg_config_home: Option<OsString>,
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
            xdg_data_dirs: get("XDG_DATA_DIRS"),
            xdg_state_home: get("XDG_STATE_HOME"),
            home: get("HOME"),
        }
    }
}

/// The user-writable config path, mirroring the compositor's `userConfigPath`.
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

/// The config to edit: the user's own file, whether or not it exists yet.
pub fn resolve(env: &Env) -> PathBuf {
    user_config_path(env)
}

/// `resolve` against the real process environment.
pub fn resolve_process() -> PathBuf {
    resolve(&Env::from_process())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(config_home: Option<&str>, home: Option<&str>) -> Env {
        Env {
            xdg_config_home: config_home.map(OsString::from),
            xdg_data_dirs: None,
            xdg_state_home: None,
            home: home.map(OsString::from),
        }
    }

    #[test]
    fn resolve_is_the_user_path() {
        assert_eq!(
            resolve(&env(None, Some("/home/tester"))),
            PathBuf::from("/home/tester/.config/umbriel/config.toml")
        );
        assert_eq!(
            resolve(&env(Some("/custom/cfg"), Some("/home/tester"))),
            PathBuf::from("/custom/cfg/umbriel/config.toml")
        );
        assert_eq!(
            resolve(&env(None, None)),
            PathBuf::from(".config/umbriel/config.toml")
        );
    }

    #[test]
    fn resolve_never_picks_a_system_config() {
        // A packaged default exists but the user has no config yet: that is
        // a fresh start, not the read-only system file.
        let root = std::env::temp_dir().join(format!("umbriel-discovery-{}", std::process::id()));
        let share = root.join("share");
        std::fs::create_dir_all(share.join("umbriel")).unwrap();
        std::fs::write(share.join("umbriel/config.toml"), b"").unwrap();

        let e = Env {
            xdg_data_dirs: Some(share.into_os_string()),
            ..env(Some(root.join("cfg").to_str().unwrap()), None)
        };
        assert_eq!(resolve(&e), root.join("cfg/umbriel/config.toml"));

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
            ..env(None, None)
        };
        assert_eq!(
            packaged_default(&found),
            Some(share.join("umbriel/config.toml"))
        );

        let missing = Env {
            xdg_data_dirs: Some("/nonexistent-umbriel-data".into()),
            ..env(None, None)
        };
        assert_eq!(packaged_default(&missing), None);

        std::fs::remove_dir_all(&root).ok();
    }
}
