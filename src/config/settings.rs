//! App preferences — umbriel-config's own settings, never umbriel's config.
//! Simple `key = value` lines; a missing or unreadable file reads as defaults.

use super::discovery;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub check_updates_on_start: bool,
    pub window_width: u32,
    pub window_height: u32,
    /// true = dark design, false = light design.
    pub dark: bool,
}

pub const DEFAULT: Settings = Settings {
    check_updates_on_start: true,
    window_width: 960,
    window_height: 640,
    dark: true,
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
            "check_updates_on_start = {}\nwindow_width = {}\nwindow_height = {}\ndark = {}\n",
            settings.check_updates_on_start,
            settings.window_width,
            settings.window_height,
            settings.dark
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
            }
        );
        store(&e, &DEFAULT).unwrap();
        assert_eq!(load(&e), DEFAULT);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn parse_ignores_unknown_keys_and_bad_values() {
        let settings = parse(
            "unknown = 1\ncheck_updates_on_start = false\nwindow_width = oops\nwindow_height = 700\ndark = false\nbad_dark = maybe\n",
        );
        assert!(!settings.check_updates_on_start);
        assert_eq!(settings.window_width, DEFAULT.window_width);
        assert_eq!(settings.window_height, 700);
        assert!(!settings.dark);
        // A malformed `dark` line falls back to the default (dark).
        let settings = parse("dark = maybe\n");
        assert!(settings.dark);
    }
}
