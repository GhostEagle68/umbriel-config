//! Last-seen schema snapshot (one dotted key per line) so the next run can
//! report what an umbriel update changed. Disposable cache: a missing or
//! unreadable file simply reads as "nothing seen before".

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use super::discovery;

/// Snapshot file for the given environment: `$XDG_STATE_HOME` (else
/// `$HOME/.local/state`) + `umbriel-config/schema.keys`.
pub fn snapshot_path(env: &discovery::Env) -> PathBuf {
    let base = if let Some(state_home) = env.xdg_state_home.as_deref() {
        PathBuf::from(state_home)
    } else {
        let home = env.home.as_deref().unwrap_or_else(|| OsStr::new(""));
        Path::new(home).join(".local/state")
    };
    base.join("umbriel-config/schema.keys")
}

/// Keys recorded by the last run; a missing or unreadable file is empty.
pub fn load(path: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(path)
        .map(|text| {
            text.lines()
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Record `keys` sorted, creating parent directories. Errors are the
/// caller's to ignore; the snapshot is a cache, not state we own.
pub fn store(path: &Path, keys: &BTreeSet<String>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = String::new();
    for key in keys {
        text.push_str(key);
        text.push('\n');
    }
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn env(state_home: Option<&str>, home: Option<&str>) -> discovery::Env {
        discovery::Env {
            xdg_state_home: state_home.map(OsString::from),
            home: home.map(OsString::from),
            ..Default::default()
        }
    }

    #[test]
    fn snapshot_path_prefers_xdg_state_home() {
        assert_eq!(
            snapshot_path(&env(Some("/state"), Some("/home/t"))),
            PathBuf::from("/state/umbriel-config/schema.keys")
        );
    }

    #[test]
    fn snapshot_path_falls_back_to_home() {
        assert_eq!(
            snapshot_path(&env(None, Some("/home/t"))),
            PathBuf::from("/home/t/.local/state/umbriel-config/schema.keys")
        );
    }

    #[test]
    fn store_then_load_round_trips() {
        let path = std::env::temp_dir().join(format!("umbriel-state-{}", std::process::id()));
        let keys = BTreeSet::from([
            "animation.windows_in.duration_ms".to_owned(),
            "general.xwayland".to_owned(),
        ]);
        store(&path, &keys).unwrap();
        assert_eq!(load(&path), keys);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_of_missing_file_is_empty() {
        assert!(load(Path::new("/nonexistent-umbriel-state/keys")).is_empty());
    }

    #[test]
    fn store_creates_missing_parent_dirs() {
        let root = std::env::temp_dir().join(format!("umbriel-state-dirs-{}", std::process::id()));
        let path = root.join("nested/deeper/schema.keys");
        store(&path, &BTreeSet::from(["x".to_owned()])).unwrap();
        assert!(path.is_file());
        std::fs::remove_dir_all(&root).ok();
    }
}
