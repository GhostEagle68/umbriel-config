//! App preferences — umbriel-config's own settings, never umbriel's config.
//! Simple `key = value` lines; a missing or unreadable file reads as defaults.

use super::discovery;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub check_updates_on_start: bool,
    pub window_width: u32,
    pub window_height: u32,
    /// true = dark design, false = light design.
    pub dark: bool,
    /// Backup runs kept per location (minimum 1).
    pub backup_count: u32,
    /// Custom backup location; None = the state-directory default.
    pub backup_dir: Option<String>,
}

pub const DEFAULT: Settings = Settings {
    check_updates_on_start: true,
    window_width: 960,
    window_height: 640,
    dark: true,
    backup_count: 10,
    backup_dir: None,
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
    parse(&text)
}

/// Parse `key = value` lines; unknown keys and malformed values fall back
/// field-by-field to the defaults.
fn parse(text: &str) -> Settings {
    let mut settings = DEFAULT;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "check_updates_on_start" => settings.check_updates_on_start = value == "true",
            "window_width" => settings.window_width = value.parse().unwrap_or(DEFAULT.window_width),
            "window_height" => {
                settings.window_height = value.parse().unwrap_or(DEFAULT.window_height)
            }
            "dark" => settings.dark = value.parse().unwrap_or(DEFAULT.dark),
            "backup_count" => settings.backup_count = value.parse().unwrap_or(DEFAULT.backup_count),
            "backup_dir" => settings.backup_dir = (!value.is_empty()).then(|| value.to_owned()),
            _ => {}
        }
    }
    settings
}

pub fn store(env: &discovery::Env, settings: &Settings) -> std::io::Result<()> {
    let path = path(env);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        format!(
            "check_updates_on_start = {}\nwindow_width = {}\nwindow_height = {}\ndark = {}\nbackup_count = {}\nbackup_dir = {}\n",
            settings.check_updates_on_start,
            settings.window_width,
            settings.window_height,
            settings.dark,
            settings.backup_count,
            settings.backup_dir.as_deref().unwrap_or(""),
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
                window_width: 1280,
                window_height: 800,
                dark: false,
                backup_count: 3,
                backup_dir: Some("/tmp/b".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(
            load(&e),
            Settings {
                check_updates_on_start: false,
                window_width: 1280,
                window_height: 800,
                dark: false,
                backup_count: 3,
                backup_dir: Some("/tmp/b".to_owned()),
            }
        );
        store(&e, &DEFAULT).unwrap();
        assert_eq!(load(&e), DEFAULT);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn parse_ignores_unknown_keys_and_bad_values() {
        let settings = parse(
            "unknown = 1\ncheck_updates_on_start = false\nwindow_width = oops\nwindow_height = 700\ndark = false\nbackup_count = 3\nbackup_dir = /tmp/b\nbad_dark = maybe\n",
        );
        assert!(!settings.check_updates_on_start);
        assert_eq!(settings.window_width, DEFAULT.window_width);
        assert_eq!(settings.window_height, 700);
        assert!(!settings.dark);
        assert_eq!(settings.backup_count, 3);
        assert_eq!(settings.backup_dir.as_deref(), Some("/tmp/b"));
        // Malformed values fall back to the defaults per field.
        let settings = parse("dark = maybe\nbackup_count = oops\n");
        assert!(settings.dark);
        assert_eq!(settings.backup_count, DEFAULT.backup_count);
        assert_eq!(settings.backup_dir, None);
    }
}
