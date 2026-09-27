//! Update check against GitHub releases; the HTTP call runs off-thread.

use crate::config::discovery;
use crate::config::settings::Channel;
use semver::Version;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DAY_SECS: u64 = 24 * 60 * 60;
// Stable follows GitHub's own "latest", which is never a pre-release
// (so never the canary).
const LATEST_URL: &str = "https://api.github.com/repos/GhostEagle68/umbriel-config/releases/latest";
const CANARY_URL: &str =
    "https://api.github.com/repos/GhostEagle68/umbriel-config/releases/tags/canary";
const USER_AGENT: &str = "umbriel-config";

/// The commit CI built this binary from; only canary builds carry it.
pub const BUILD_SHA: Option<&str> = option_env!("UMBRIEL_CONFIG_BUILD_SHA");

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
    /// The commit a canary release was created from (`--target <sha>`).
    #[serde(default)]
    target_commitish: String,
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

/// Stable only offers a version newer than the running build, so
/// switching to Stable from a canary never proposes a downgrade. Canary
/// offers any canary built from a different commit.
pub fn check(channel: Channel) -> Result<Verdict, String> {
    let release = match channel {
        Channel::Stable => match get_json::<Release>(LATEST_URL)? {
            Some(release) => release,
            None => return Ok(Verdict::NoRelease),
        },
        Channel::Canary => return Ok(canary_verdict(BUILD_SHA, get_json(CANARY_URL)?)),
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

/// A build without a commit (a versioned release, cargo, source) is
/// offered the canary as-is. `None` release: none published, or the
/// workflow is between deleting the old canary and creating the new one.
fn canary_verdict(build: Option<&str>, release: Option<Release>) -> Verdict {
    match release {
        Some(release) if Some(release.target_commitish.as_str()) != build => {
            Verdict::UpdateAvailable {
                version: format!("canary {}", short(&release.target_commitish)),
                notes: release
                    .body
                    .map(|body| new_to(&body, build).trim().to_owned())
                    .filter(|body| !body.is_empty()),
            }
        }
        _ => Verdict::UpToDate,
    }
}

fn short(sha: &str) -> &str {
    &sha[..7.min(sha.len())]
}

/// The canary notes list commits newest first, each ending `(<short sha>)`;
/// keep only those after `build`. Unknown builds keep everything.
fn new_to(body: &str, build: Option<&str>) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let cut = build
        .map(|build| format!("({})", short(build)))
        .and_then(|mark| lines.iter().position(|line| line.ends_with(&mark)))
        .unwrap_or(lines.len());
    lines[..cut].join("\n")
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
pub fn update_hint(kind: InstallKind, channel: Channel, version: &str) -> String {
    match kind {
        // Canary builds exist only as release tarballs.
        InstallKind::Cargo if channel == Channel::Canary => {
            "Canary builds are tarballs: install one from the canary release page, or switch to Stable."
                .to_owned()
        }
        // Cargo skips pre-release versions unless one is named, so the
        // version is part of the command, never optional.
        InstallKind::Cargo => format!("Update with: cargo install umbriel-config@{version}"),
        InstallKind::Package => "Update with your package manager.".to_owned(),
        InstallKind::Source => "Update with: git pull, then cargo build --release".to_owned(),
        InstallKind::Tarball => String::new(),
    }
}

/// Download `tag`'s tarball (`v0.3.0`, `canary`) for this architecture,
/// check it against the published sha256, and replace the running
/// binary. Renaming over a running executable is safe on Linux: the old
/// inode stays until exit.
pub fn install(tag: &str) -> Result<(), String> {
    let asset = format!(
        "https://github.com/GhostEagle68/umbriel-config/releases/download/{tag}/umbriel-config-{}-linux.tar.gz",
        std::env::consts::ARCH
    );
    let tarball = get_bytes(&asset)?;
    let published = get_bytes(&format!("{asset}.sha256"))?;
    let published = String::from_utf8_lossy(&published);
    let published = published.split_whitespace().next().unwrap_or_default();
    let actual = sha256_hex(&tarball);
    if !actual.eq_ignore_ascii_case(published) {
        return Err("checksum mismatch — nothing was installed".to_owned());
    }
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let staged = exe.with_file_name(".umbriel-config.new");
    unpack_binary(&tarball, &staged)?;
    // The build being replaced stays next to it for `umbriel-config rollback`.
    if let Err(err) = std::fs::copy(&exe, previous_path(&exe)) {
        let _ = std::fs::remove_file(&staged);
        return Err(format!(
            "could not keep the current build for rollback: {err}"
        ));
    }
    std::fs::rename(&staged, &exe).map_err(|err| {
        let _ = std::fs::remove_file(&staged);
        format!("could not replace {}: {err}", exe.display())
    })
}

fn previous_path(exe: &Path) -> PathBuf {
    let mut name = exe.file_name().unwrap_or_default().to_os_string();
    name.push(".prev");
    exe.with_file_name(name)
}

/// Put back the build the last in-app update replaced.
pub fn rollback() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let previous = previous_path(&exe);
    if !previous.exists() {
        return Err(format!("no previous build at {}", previous.display()));
    }
    std::fs::rename(&previous, &exe)
        .map_err(|err| format!("could not restore {}: {err}", exe.display()))?;
    Ok(exe)
}

/// Notes of an installed canary, shown once by the build that replaces
/// this one (every canary shares a version, so the changelog can't).
pub fn save_canary_notes(env: &discovery::Env, notes: &str) {
    let path = state_path(env, "canary-notes.md");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, notes);
}

pub fn take_canary_notes(env: &discovery::Env) -> Option<String> {
    let path = state_path(env, "canary-notes.md");
    let notes = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(path);
    (!notes.trim().is_empty()).then_some(notes)
}

/// Lowercase hex SHA-256, the form published in `<asset>.sha256`.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
    discovery::state_dir(env).join(name)
}

/// The channels/in-app-update notice shows once, ever: the first launch
/// after updating into a build that has them.
pub fn notice_should_show(env: &discovery::Env) -> bool {
    !state_path(env, "notice-canary-channel").exists()
}

pub fn notice_mark_shown(env: &discovery::Env) {
    let path = state_path(env, "notice-canary-channel");
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
    fn sha256_hex_matches_the_published_form() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

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
            update_hint(InstallKind::Cargo, Channel::Stable, "0.3.0-beta.2"),
            "Update with: cargo install umbriel-config@0.3.0-beta.2"
        );
        assert!(
            update_hint(InstallKind::Cargo, Channel::Canary, "canary abc1234").contains("tarball")
        );
        assert!(update_hint(InstallKind::Tarball, Channel::Stable, "0.3.0").is_empty());
    }

    #[test]
    fn canary_verdict_compares_commits() {
        let canary = |sha: &str| {
            Some(Release {
                tag_name: "canary".to_owned(),
                body: None,
                target_commitish: sha.to_owned(),
            })
        };
        // A versioned or local build has no commit: any canary is new to it.
        assert!(matches!(
            canary_verdict(None, canary("abc1234ff")),
            Verdict::UpdateAvailable { .. }
        ));
        assert_eq!(
            canary_verdict(Some("abc1234ff"), canary("abc1234ff")),
            Verdict::UpToDate
        );
        assert_eq!(
            canary_verdict(Some("0000000"), canary("abc1234ff")),
            Verdict::UpdateAvailable {
                version: "canary abc1234".to_owned(),
                notes: None,
            }
        );
        // Between the workflow's delete and create there is no release.
        assert_eq!(canary_verdict(Some("0000000"), None), Verdict::UpToDate);
    }

    #[test]
    fn canary_notes_keep_only_commits_after_the_build() {
        let body = "## Coming in the next release\n\nShaders write presets.\n\n\
                    ## Commits\n\n- fix: c (ccccccc)\n- feat: b (bbbbbbb)\n- fix: a (aaaaaaa)";
        let trimmed = new_to(body, Some("bbbbbbb1234"));
        assert!(trimmed.contains("Shaders write presets."));
        assert!(trimmed.contains("(ccccccc)"));
        assert!(!trimmed.contains("(bbbbbbb)") && !trimmed.contains("(aaaaaaa)"));
        // A build older than the list, or none at all, sees everything.
        assert_eq!(new_to(body, Some("9999999")), body);
        assert_eq!(new_to(body, None), body);
    }

    #[test]
    fn previous_build_sits_next_to_the_binary() {
        assert_eq!(
            previous_path(Path::new("/home/u/.local/bin/umbriel-config")),
            Path::new("/home/u/.local/bin/umbriel-config.prev")
        );
    }
}
