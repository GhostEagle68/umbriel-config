//! App preferences — umbriel-config's own settings, never umbriel's config.
//! One key for now; a missing or unreadable file reads as defaults.

use super::discovery;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub check_updates_on_start: bool,
}

pub const DEFAULT: Settings = Settings {
    check_updates_on_start: true,
};

pub fn path(env: &discovery::Env) -> PathBuf {
    let base = if let Some(config_home) = env.xdg_config_home.as_deref() {
        PathBuf::from(config_home)
    } else {
        let home = env.home.as_deref().unwrap_or_else(|| OsStr::new(""));
        Path::new(home).join(".config")
    };
    base.join("umbriel-config/settings.toml")
}

pub fn load(env: &discovery::Env) -> Settings {
    let Ok(text) = std::fs::read_to_string(path(env)) else {
        return DEFAULT;
    };
    Settings {
        check_updates_on_start: text
            .lines()
            .any(|line| line.trim() == "check_updates_on_start = true"),
    }
}

pub fn store(env: &discovery::Env, settings: &Settings) -> std::io::Result<()> {
    let path = path(env);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        format!(
            "check_updates_on_start = {}\n",
            settings.check_updates_on_start
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn env(config_home: Option<&str>, home: Option<&str>) -> discovery::Env {
        discovery::Env {
            xdg_config_home: config_home.map(OsString::from),
            home: home.map(OsString::from),
            ..Default::default()
        }
    }

    #[test]
    fn settings_round_trip() {
        let root = std::env::temp_dir().join(format!("umbriel-settings-{}", std::process::id()));
        let e = env(Some(root.to_str().unwrap()), Some("/home/t"));
        assert_eq!(load(&e), DEFAULT);
        store(
            &e,
            &Settings {
                check_updates_on_start: false,
            },
        )
        .unwrap();
        assert!(!load(&e).check_updates_on_start);
        store(&e, &DEFAULT).unwrap();
        assert_eq!(load(&e), DEFAULT);
        std::fs::remove_dir_all(&root).ok();
    }
}
