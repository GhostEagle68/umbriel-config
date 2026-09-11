//! Update check against GitHub releases; the HTTP call runs off-thread.

use crate::config::discovery;
use semver::Version;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DAY_SECS: u64 = 24 * 60 * 60;
const RELEASES_URL: &str =
    "https://api.github.com/repos/GhostEagle68/umbriel-config/releases?per_page=1";
const USER_AGENT: &str = "umbriel-config";

#[derive(Debug, PartialEq)]
pub enum Verdict {
    UpToDate,
    /// The newer version plus its release notes (the GitHub `body`,
    /// which release.yml fills from the changelog; `None` offline-ish).
    UpdateAvailable {
        version: String,
        notes: Option<String>,
    },
}

/// Pure comparison — no network. `None` when either side won't parse.
pub fn compare(current: &str, latest_tag: &str) -> Option<Verdict> {
    Some(match compare_versions(current, latest_tag)? {
        // `compare_versions` orders latest against current.
        Ordering::Greater => Verdict::UpdateAvailable {
            version: latest_tag.trim_start_matches('v').to_owned(),
            notes: None,
        },
        _ => Verdict::UpToDate,
    })
}

fn compare_versions(current: &str, latest_tag: &str) -> Option<Ordering> {
    let current = Version::parse(current).ok()?;
    let latest = Version::parse(latest_tag.trim_start_matches('v')).ok()?;
    Some(latest.cmp(&current))
}

#[derive(serde::Deserialize)]
struct Release {
    #[serde(rename = "tag_name")]
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
}

pub fn check() -> Result<Verdict, String> {
    let releases: Vec<Release> = ureq::get(RELEASES_URL)
        .header("User-Agent", USER_AGENT)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .call()
        .map_err(|err| err.to_string())?
        .body_mut()
        .read_json()
        .map_err(|err| err.to_string())?;
    let Some(release) = releases.first() else {
        return Err("no releases published yet".to_owned());
    };
    let notes = release
        .body
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty());
    compare_versions(env!("CARGO_PKG_VERSION"), &release.tag_name)
        .map(|ordering| {
            if ordering == Ordering::Greater {
                Verdict::UpdateAvailable {
                    version: release.tag_name.trim_start_matches('v').to_owned(),
                    notes: notes.map(str::to_owned),
                }
            } else {
                Verdict::UpToDate
            }
        })
        .ok_or_else(|| format!("unparseable version '{}'", release.tag_name))
}

/// Record of the last automatic check, so startup checks happen at most
/// once a day. Disposable cache: unreadable means "check now".
fn stamp_path(env: &discovery::Env) -> PathBuf {
    let base = if let Some(state_home) = env.xdg_state_home.as_deref() {
        PathBuf::from(state_home)
    } else {
        let home = env
            .home
            .as_deref()
            .unwrap_or_else(|| std::ffi::OsStr::new(""));
        Path::new(home).join(".local/state")
    };
    base.join("umbriel-config/last-update-check")
}

pub fn should_auto_check(env: &discovery::Env) -> bool {
    let last = std::fs::read_to_string(stamp_path(env))
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok());
    let Some(last) = last else {
        return true;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(last) >= DAY_SECS
}

pub fn mark_checked(env: &discovery::Env) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let path = stamp_path(env);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, format!("{now}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_orders_semver_and_strips_v() {
        assert_eq!(
            compare("0.1.1-alpha.2", "v0.1.1"),
            Some(Verdict::UpdateAvailable {
                version: "0.1.1".to_owned(),
                notes: None
            })
        );
        assert_eq!(compare("0.1.2-alpha.1", "v0.1.1"), Some(Verdict::UpToDate));
        assert_eq!(compare("0.1.1", "v0.1.1"), Some(Verdict::UpToDate));
        assert_eq!(compare("0.1.1", "garbage"), None);
        assert_eq!(
            compare("0.1.1-alpha.2", "v0.1.1-alpha.3"),
            Some(Verdict::UpdateAvailable {
                version: "0.1.1-alpha.3".to_owned(),
                notes: None
            })
        );
    }
}
