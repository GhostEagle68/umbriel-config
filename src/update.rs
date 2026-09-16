//! Update check against GitHub releases; the HTTP call runs off-thread.

use crate::config::discovery;
use semver::Version;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::ffi::OsStr;
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

/// How this build was installed. Only a tarball copy replaces itself;
/// every other kind belongs to a tool that must do the update.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstallKind {
    Tarball,
    Cargo,
    Package,
    Source,
}

/// Pure classification, so the rules are testable without an install.
pub fn install_kind(exe: &Path, home: Option<&OsStr>, cargo_home: Option<&OsStr>) -> InstallKind {
    if exe
        .components()
        .any(|part| part.as_os_str() == OsStr::new("target"))
    {
        return InstallKind::Source;
    }
    let cargo_bin = cargo_home
        .map(|home| Path::new(home).join("bin"))
        .or_else(|| home.map(|home| Path::new(home).join(".cargo/bin")));
    if let (Some(parent), Some(cargo_bin)) = (exe.parent(), cargo_bin)
        && parent == cargo_bin
    {
        return InstallKind::Cargo;
    }
    if exe.starts_with("/usr") || exe.starts_with("/opt") {
        return InstallKind::Package;
    }
    InstallKind::Tarball
}

/// The running build's kind; an unreadable path counts as `Package`, the
/// one kind that never touches the binary.
pub fn current_install_kind() -> InstallKind {
    let Ok(exe) = std::env::current_exe() else {
        return InstallKind::Package;
    };
    let home = std::env::var_os("HOME");
    let cargo_home = std::env::var_os("CARGO_HOME");
    install_kind(&exe, home.as_deref(), cargo_home.as_deref())
}

/// How to update a build the app can't replace itself.
pub fn update_hint(kind: InstallKind, version: &str) -> String {
    match kind {
        // Cargo skips pre-release versions unless one is named, so the
        // version is part of the command, never optional.
        InstallKind::Cargo => format!("Update with: cargo install umbriel-config@{version}"),
        InstallKind::Package => "Update with your package manager.".to_owned(),
        InstallKind::Source => "Update with: git pull, then cargo build --release".to_owned(),
        InstallKind::Tarball => String::new(),
    }
}

/// Download the release tarball for this architecture, check it against
/// the published sha256, and replace the running binary. Renaming over a
/// running executable is safe on Linux: the old inode stays until exit.
pub fn install(version: &str) -> Result<(), String> {
    let asset = format!(
        "https://github.com/GhostEagle68/umbriel-config/releases/download/v{version}/umbriel-config-{}-linux.tar.gz",
        std::env::consts::ARCH
    );
    let tarball = get_bytes(&asset)?;
    let published = get_bytes(&format!("{asset}.sha256"))?;
    let published = String::from_utf8_lossy(&published);
    let published = published.split_whitespace().next().unwrap_or_default();
    let actual = format!("{:x}", Sha256::digest(&tarball));
    if !actual.eq_ignore_ascii_case(published) {
        return Err("checksum mismatch — nothing was installed".to_owned());
    }
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let staged = exe.with_file_name(".umbriel-config.new");
    unpack_binary(&tarball, &staged)?;
    std::fs::rename(&staged, &exe).map_err(|err| {
        let _ = std::fs::remove_file(&staged);
        format!("could not replace {}: {err}", exe.display())
    })
}

fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    ureq::get(url)
        .header("User-Agent", USER_AGENT)
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .build()
        .call()
        .map_err(|err| err.to_string())?
        .body_mut()
        .with_config()
        // The tarball is already at ureq's 10 MB default; leave headroom.
        .limit(64 * 1024 * 1024)
        .read_to_vec()
        .map_err(|err| err.to_string())
}

/// Pull just `bin/umbriel-config` out of the tarball and stage it next to
/// the current binary, so the replacing rename stays on one filesystem.
fn unpack_binary(tarball: &[u8], staged: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    for entry in archive.entries().map_err(|err| err.to_string())? {
        let mut entry = entry.map_err(|err| err.to_string())?;
        let is_binary = entry
            .path()
            .map(|path| path.ends_with("bin/umbriel-config"))
            .unwrap_or(false);
        if !is_binary {
            continue;
        }
        let mut file = std::fs::File::create(staged).map_err(|err| err.to_string())?;
        std::io::copy(&mut entry, &mut file).map_err(|err| err.to_string())?;
        file.set_permissions(std::fs::Permissions::from_mode(0o755))
            .map_err(|err| err.to_string())?;
        file.sync_all().map_err(|err| err.to_string())?;
        return Ok(());
    }
    Err("the release tarball has no bin/umbriel-config".to_owned())
}

/// Record of the last automatic check, so startup checks happen at most
/// once a day. Disposable cache: unreadable means "check now".
fn stamp_path(env: &discovery::Env) -> PathBuf {
    state_path(env, "last-update-check")
}

fn state_path(env: &discovery::Env, name: &str) -> PathBuf {
    let base = if let Some(state_home) = env.xdg_state_home.as_deref() {
        PathBuf::from(state_home)
    } else {
        let home = env
            .home
            .as_deref()
            .unwrap_or_else(|| std::ffi::OsStr::new(""));
        Path::new(home).join(".local/state")
    };
    base.join("umbriel-config").join(name)
}

/// The channels/in-app-update notice shows once, ever: the first launch
/// after updating into a build that has them.
pub fn notice_should_show(env: &discovery::Env) -> bool {
    !state_path(env, "notice-release-channels").exists()
}

pub fn notice_mark_shown(env: &discovery::Env) {
    let path = state_path(env, "notice-release-channels");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, "shown\n");
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

    #[test]
    fn install_kind_reads_the_binary_location() {
        let home = OsStr::new("/home/t");
        let cargo = OsStr::new("/home/t/.cargo");
        let kind = |path: &str, cargo_home: Option<&OsStr>| {
            install_kind(Path::new(path), Some(home), cargo_home)
        };
        assert_eq!(
            kind("/home/t/.local/bin/umbriel-config", None),
            InstallKind::Tarball
        );
        assert_eq!(
            kind("/home/t/.cargo/bin/umbriel-config", None),
            InstallKind::Cargo
        );
        // CARGO_HOME wins over the ~/.cargo default when it is set.
        assert_eq!(
            kind("/home/t/.cargo/bin/umbriel-config", Some(cargo)),
            InstallKind::Cargo
        );
        assert_eq!(kind("/usr/bin/umbriel-config", None), InstallKind::Package);
        assert_eq!(
            kind(
                "/home/t/src/umbriel-config/target/release/umbriel-config",
                None
            ),
            InstallKind::Source
        );
    }

    #[test]
    fn release_notice_shows_once_ever() {
        let root = std::env::temp_dir().join(format!("umbriel-notice-{}", std::process::id()));
        let env = discovery::Env {
            xdg_state_home: Some(root.clone().into_os_string()),
            ..Default::default()
        };
        assert!(notice_should_show(&env));
        notice_mark_shown(&env);
        assert!(!notice_should_show(&env));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn cargo_hint_always_names_the_version() {
        assert_eq!(
            update_hint(InstallKind::Cargo, "0.3.0-beta.2"),
            "Update with: cargo install umbriel-config@0.3.0-beta.2"
        );
        assert!(update_hint(InstallKind::Tarball, "0.3.0").is_empty());
    }
}
