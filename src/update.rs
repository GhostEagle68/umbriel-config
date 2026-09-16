//! Update check against GitHub releases; the HTTP call runs off-thread.

use crate::config::discovery;
use semver::Version;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DAY_SECS: u64 = 24 * 60 * 60;
// The Pre-release channel ranks this list by version; Stable follows
// GitHub's own "latest", which is never a pre-release.
const RELEASES_URL: &str =
    "https://api.github.com/repos/GhostEagle68/umbriel-config/releases?per_page=20";
const LATEST_URL: &str = "https://api.github.com/repos/GhostEagle68/umbriel-config/releases/latest";
const USER_AGENT: &str = "umbriel-config";

#[derive(Debug, PartialEq)]
pub enum Verdict {
    UpToDate,
    /// Stable channel, but no stable release exists yet (GitHub 404s).
    NoRelease,
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

/// GET + JSON with the shared agent settings. `Ok(None)` is a 404 —
/// `/releases/latest` answers that until the first stable release.
fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<Option<T>, String> {
    match ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .call()
    {
        Ok(mut response) => response
            .body_mut()
            .read_json()
            .map(Some)
            .map_err(|err| err.to_string()),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

/// Highest version among the releases — the list is ordered by date, so
/// a beta published after a stable would otherwise win. Unparseable
/// tags are ignored.
fn newest(releases: Vec<Release>) -> Option<Release> {
    releases
        .into_iter()
        .filter_map(|release| Some((tag_version(&release)?, release)))
        .max_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, release)| release)
}

fn tag_version(release: &Release) -> Option<Version> {
    Version::parse(release.tag_name.trim_start_matches('v')).ok()
}

/// `prereleases` picks the channel. Either way the result is only
/// offered when it is newer than the running build, so switching to
/// Stable from a newer beta never proposes a downgrade.
pub fn check(prereleases: bool) -> Result<Verdict, String> {
    let release = if prereleases {
        let releases: Vec<Release> =
            get_json(RELEASES_URL)?.ok_or_else(|| "no releases published yet".to_owned())?;
        match newest(releases) {
            Some(release) => release,
            None => return Err("no releases published yet".to_owned()),
        }
    } else {
        match get_json::<Release>(LATEST_URL)? {
            Some(release) => release,
            None => return Ok(Verdict::NoRelease),
        }
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

    fn release(tag: &str) -> Release {
        Release {
            tag_name: tag.to_owned(),
            body: None,
        }
    }

    #[test]
    fn newest_ranks_by_version_not_by_position() {
        let picked = newest(vec![
            release("v0.3.0-beta.9"),
            release("v0.3.0"),
            release("v0.3.0-beta.10"),
            release("nightly"),
        ]);
        assert_eq!(picked.unwrap().tag_name, "v0.3.0");
        let picked = newest(vec![release("v0.3.0-beta.9"), release("v0.3.0-beta.10")]);
        assert_eq!(picked.unwrap().tag_name, "v0.3.0-beta.10");
        assert!(newest(vec![release("nightly")]).is_none());
    }
}
