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

// Public halves of the minisign keys that sign releases. Stable releases
// are signed offline by the owner: `stable`, with `stable-backup` held
// back so a lost key doesn't strand installed copies. Canary builds are
// signed by CI with its own key, which the Stable channel never accepts.
const STABLE_KEYS: [&str; 2] = [
    "RWRzWiLAE3f9/ya8WVazdB8ifmVjVxcoCRyjNPILEAJObVuePRfDJZlo",
    "RWRwaqHb1jOXEO7kIKp/+33t/iZg7dIUfFTCCVEwr41H7e0C8MXHNysJ",
];
const CANARY_KEYS: [&str; 1] = ["RWTZCOZ2M15yAECyWl1YiMbKclqPFU6xu+/m56CpLwSHilFAJvb9mi/u"];

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
    match compare(env!("CARGO_PKG_VERSION"), &release.tag_name) {
        Some(Verdict::UpdateAvailable { version, .. }) => Ok(Verdict::UpdateAvailable {
            version,
            notes: notes.map(str::to_owned),
        }),
        Some(verdict) => Ok(verdict),
        None => Err(format!("unparseable version '{}'", release.tag_name)),
    }
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

pub fn short(sha: &str) -> &str {
    &sha[..7.min(sha.len())]
}

/// The canary notes group entries under headings as cliff.toml does,
/// each entry's first line ending `(<short sha>)`, and list every commit
/// newest first in a `<!-- commits: … -->` line. Keep the entries newer
/// than `build` (an unknown build keeps them all), drop headings left
/// empty, and stop at `## Install`, which is for new installs.
fn new_to(body: &str, build: Option<&str>) -> String {
    let order: Vec<&str> = body
        .lines()
        .find_map(|line| line.strip_prefix("<!-- commits: ")?.strip_suffix(" -->"))
        .map(|list| list.split(' ').collect())
        .unwrap_or_default();
    let seen = build
        .and_then(|build| order.iter().position(|sha| *sha == short(build)))
        .map_or(&[][..], |at| &order[at..]);
    // Drop each entry the build has: its `- ` line and indented body.
    let mut kept: Vec<&str> = Vec::new();
    let mut skipping = false;
    for line in body.lines().take_while(|line| *line != "## Install") {
        if line.starts_with("<!-- commits: ") {
            continue;
        }
        if line.starts_with("- ") {
            skipping = seen.iter().any(|sha| line.ends_with(&format!("({sha})")));
        } else if !line.is_empty() && !line.starts_with("  ") {
            skipping = false;
        }
        if !skipping {
            kept.push(line);
        }
    }
    // A heading stays while an entry sits under it, before the next
    // heading of its level or above.
    let level = |line: &str| {
        line.starts_with('#')
            .then(|| line.chars().take_while(|c| *c == '#').count())
    };
    let mut lines: Vec<&str> = kept
        .iter()
        .enumerate()
        .filter(|(at, line)| match level(line) {
            None => true,
            Some(depth) => kept[at + 1..]
                .iter()
                .take_while(|next| level(next).is_none_or(|next| next > depth))
                .any(|next| next.starts_with("- ")),
        })
        .map(|(_, line)| *line)
        .collect();
    lines.dedup_by(|a, b| a.is_empty() && b.is_empty());
    lines.join("\n")
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
/// check it against the published sha256 and its minisign signature, and
/// replace the running binary. Renaming over a running executable is safe on Linux: the old
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
    // Nothing is unpacked, let alone run, before the signature holds.
    let signature = get_bytes(&format!("{asset}.minisig")).map_err(|err| {
        format!("could not fetch the release signature ({err}) — nothing was installed")
    })?;
    check_release(tag, &tarball, &String::from_utf8_lossy(&signature))?;
    let exe = std::env::current_exe().map_err(|err| err.to_string())?;
    let staged = exe.with_file_name(".umbriel-config.new");
    unpack_binary(&tarball, &staged).inspect_err(|_| {
        let _ = std::fs::remove_file(&staged);
    })?;
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

/// Whether `signature` is a valid signature over `tarball` from one of the
/// channel's keys, made for this very release: the signed comment names
/// the version (Stable), so an old signed tarball can't stand in for a new
/// one. Canary's comment only has to say it is a canary build.
fn check_release(tag: &str, tarball: &[u8], signature: &str) -> Result<(), String> {
    let keys: &[&str] = if tag == "canary" {
        &CANARY_KEYS
    } else {
        &STABLE_KEYS
    };
    check_signed_for(keys, tag, tarball, signature)
}

fn check_signed_for(
    keys: &[&str],
    tag: &str,
    tarball: &[u8],
    signature: &str,
) -> Result<(), String> {
    let comment = verify_signed(keys, tarball, signature)?;
    let matches = match tag {
        "canary" => comment.starts_with("umbriel-config canary "),
        _ => comment == format!("umbriel-config {tag}"),
    };
    if matches {
        Ok(())
    } else {
        Err("the signature is for a different release — nothing was installed".to_owned())
    }
}

/// Verify `signature` over `bytes` against any of `keys` (base64 public
/// keys); returns the signed comment.
fn verify_signed(keys: &[&str], bytes: &[u8], signature: &str) -> Result<String, String> {
    let signature = minisign_verify::Signature::decode(signature)
        .map_err(|_| "unreadable signature — nothing was installed".to_owned())?;
    let signed_by_a_key = keys.iter().any(|key| {
        minisign_verify::PublicKey::from_base64(key)
            .is_ok_and(|key| key.verify(bytes, &signature, false).is_ok())
    });
    if signed_by_a_key {
        Ok(signature.trusted_comment().to_owned())
    } else {
        Err("signature check failed — nothing was installed".to_owned())
    }
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

/// How many canary updates the changelog keeps notes for.
const CANARY_KEPT: usize = 5;

/// Notes of an installed canary, by its short commit: the build that
/// replaces this one shows them once, and the changelog lists the last
/// few (every canary shares a version, so the bundled changelog can't).
pub fn save_canary_notes(env: &discovery::Env, sha: &str, notes: &str) {
    let dir = state_path(env, "canary-notes");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(format!("{sha}.md")), notes);
    for (old, _) in canary_notes(env).into_iter().skip(CANARY_KEPT) {
        let _ = std::fs::remove_file(dir.join(format!("{old}.md")));
    }
}

/// The saved canary notes, newest first: (short commit, notes).
pub fn canary_notes(env: &discovery::Env) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(state_path(env, "canary-notes")) else {
        return Vec::new();
    };
    let mut saved: Vec<(SystemTime, String, String)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let sha = path.file_stem()?.to_str()?.to_owned();
            let saved_at = entry.metadata().ok()?.modified().ok()?;
            let notes = std::fs::read_to_string(&path).ok()?;
            (!notes.trim().is_empty()).then_some((saved_at, sha, notes))
        })
        .collect();
    saved.sort_by_key(|(saved_at, _, _)| std::cmp::Reverse(*saved_at));
    saved
        .into_iter()
        .map(|(_, sha, notes)| (sha, notes))
        .collect()
}

/// The running canary's notes, the first time it runs.
pub fn take_canary_notes(env: &discovery::Env, build: &str) -> Option<String> {
    let sha = short(build);
    let shown = state_path(env, "canary-notes-shown");
    if std::fs::read_to_string(&shown).is_ok_and(|seen| seen == sha) {
        return None;
    }
    let notes = std::fs::read_to_string(state_path(env, "canary-notes").join(format!("{sha}.md")))
        .ok()
        .filter(|notes| !notes.trim().is_empty())?;
    let _ = std::fs::write(shown, sha);
    Some(notes)
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

    // A throwaway key, never a release key, signed `FIXTURE_PAYLOAD` with
    // the signed comment "umbriel-config v9.9.9".
    const FIXTURE_KEY: &str = "RWRVikrDLLO6GrKhYLxL319tswGBY0kY7VppfBXAqHlYpjaothfokMQ2";
    const FIXTURE_PAYLOAD: &[u8] = b"release bytes";
    const FIXTURE_SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RURVikrDLLO6Gvrm1qFsxQXgUwWgKFc56swIKpaBgNP4EdYNpbF1QqpO7U0YyX+edaLknBV6im+qAeyz9uAJ6yuUa5shZazB4Qw=
trusted comment: umbriel-config v9.9.9
jzVnvcj6mdV5+u4Hk2T7gsuigPV0YYqqHWCkT/fU5PutnJsrJei1dnecW2hBpr0+UnGPciYnsxdramZp85p+DA==
";

    #[test]
    fn a_valid_signature_verifies_and_returns_its_signed_comment() {
        let comment = verify_signed(&[FIXTURE_KEY], FIXTURE_PAYLOAD, FIXTURE_SIGNATURE).unwrap();
        assert_eq!(comment, "umbriel-config v9.9.9");
    }

    #[test]
    fn tampered_bytes_an_unlisted_key_or_a_broken_signature_fail() {
        assert!(verify_signed(&[FIXTURE_KEY], b"release bytez", FIXTURE_SIGNATURE).is_err());
        assert!(verify_signed(&STABLE_KEYS, FIXTURE_PAYLOAD, FIXTURE_SIGNATURE).is_err());
        assert!(verify_signed(&[FIXTURE_KEY], FIXTURE_PAYLOAD, "not a signature").is_err());
        let retitled = FIXTURE_SIGNATURE.replace("v9.9.9", "v9.9.10");
        assert!(verify_signed(&[FIXTURE_KEY], FIXTURE_PAYLOAD, &retitled).is_err());
    }

    #[test]
    fn the_embedded_release_keys_are_valid_public_keys() {
        for key in STABLE_KEYS.iter().chain(&CANARY_KEYS) {
            assert!(
                minisign_verify::PublicKey::from_base64(key).is_ok(),
                "{key}"
            );
        }
        assert!(
            STABLE_KEYS.iter().all(|key| !CANARY_KEYS.contains(key)),
            "the channels must not share a key"
        );
    }

    const FIXTURE_CANARY_SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RURVikrDLLO6GthNtQkEGIi3oZxV7romyNf1lFZHgrMox7U6gFuOqqlukt1Rf2ke0Hxnc95J61ki+TZeWFHMTNluKKdzbhFE7wI=
trusted comment: umbriel-config canary abc1234
95nQlk4s5stI74L5Q2kjahklr8JmGuPj5XuKFKMdpUmcsXYCQUo24oQJWTv3QVOJoLzlr9hlVhQ4X4GlWeQODA==
";

    #[test]
    fn a_signature_only_counts_for_the_release_it_names() {
        let keys = [FIXTURE_KEY];
        let check =
            |tag: &str, signature: &str| check_signed_for(&keys, tag, FIXTURE_PAYLOAD, signature);
        assert!(check("v9.9.9", FIXTURE_SIGNATURE).is_ok());
        // An old signed tarball can't stand in for another version, not
        // even one that merely starts the same way.
        assert!(check("v9.9.10", FIXTURE_SIGNATURE).is_err());
        assert!(check("v9.9", FIXTURE_SIGNATURE).is_err());
        assert!(check("v9.9.9-beta.1", FIXTURE_SIGNATURE).is_err());
        // Canary and Stable signatures don't cross over.
        assert!(check("canary", FIXTURE_SIGNATURE).is_err());
        assert!(check("canary", FIXTURE_CANARY_SIGNATURE).is_ok());
        assert!(check("v9.9.9", FIXTURE_CANARY_SIGNATURE).is_err());
    }

    #[test]
    fn a_release_signed_by_no_channel_key_is_refused() {
        // The fixture key signs a matching comment, yet is on neither list.
        assert!(check_release("v9.9.9", FIXTURE_PAYLOAD, FIXTURE_SIGNATURE).is_err());
        assert!(check_release("canary", FIXTURE_PAYLOAD, FIXTURE_SIGNATURE).is_err());
    }

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
    fn canary_notes_keep_the_last_few_and_show_once() {
        let root = std::env::temp_dir().join(format!("umbriel-canary-{}", std::process::id()));
        let env = discovery::Env {
            xdg_state_home: Some(root.clone().into_os_string()),
            ..Default::default()
        };
        let dir = state_path(&env, "canary-notes");
        for n in 0..7 {
            save_canary_notes(&env, &format!("aaaaaa{n}"), &format!("- change {n}"));
            // Newest first goes by save time; make each one distinct.
            let file = std::fs::File::options()
                .write(true)
                .open(dir.join(format!("aaaaaa{n}.md")))
                .unwrap();
            file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1000 + n))
                .unwrap();
        }
        save_canary_notes(&env, "aaaaaa6", "- change 6");
        let saved: Vec<String> = canary_notes(&env).into_iter().map(|(sha, _)| sha).collect();
        assert_eq!(saved.len(), CANARY_KEPT);
        assert_eq!(saved[0], "aaaaaa6");
        assert!(!saved.contains(&"aaaaaa0".to_owned()));
        assert_eq!(
            take_canary_notes(&env, "aaaaaa6ffff").as_deref(),
            Some("- change 6")
        );
        assert_eq!(take_canary_notes(&env, "aaaaaa6ffff"), None);
        // Still listed for the changelog after What's new showed it.
        assert_eq!(canary_notes(&env).len(), CANARY_KEPT);
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
        // As canary.yml writes them: grouped by cliff.toml, newest last
        // within a group, with a skipped `ci` commit (ddddddd) that has
        // no entry.
        let body = "Untested build of `ddddddd` from dev.\n\
                    <!-- commits: ddddddd ccccccc bbbbbbb aaaaaaa -->\n\
                    \n\
                    ### 🚀 Features\n\
                    \n\
                    #### Shaders\n\
                    \n\
                    - Old shader thing · 2026-09-26 10:00 (aaaaaaa)\n\
                    - New shader thing · 2026-09-27 10:00 (ccccccc)\n\
                    \x20 Its body.\n\
                    \n\
                    \x20 - A detail.\n\
                    \n\
                    ### 🐛 Fixed\n\
                    \n\
                    #### Updates\n\
                    \n\
                    - Old fix · 2026-09-26 11:00 (bbbbbbb)\n\
                    \x20 Its body.\n\
                    \n\
                    ## Install\n\
                    \n\
                    curl …";
        let expected = "Untested build of `ddddddd` from dev.\n\
                        \n\
                        ### 🚀 Features\n\
                        \n\
                        #### Shaders\n\
                        \n\
                        - New shader thing · 2026-09-27 10:00 (ccccccc)\n\
                        \x20 Its body.\n\
                        \n\
                        \x20 - A detail.\n";
        assert_eq!(new_to(body, Some("bbbbbbb1234")), expected);
        // A build outside the list, or none at all, sees every entry.
        let all = new_to(body, Some("9999999"));
        assert!(all.contains("(aaaaaaa)") && all.contains("(bbbbbbb)"));
        assert!(!all.contains("## Install") && !all.contains("<!--"));
        assert_eq!(new_to(body, None), all);
    }

    #[test]
    fn previous_build_sits_next_to_the_binary() {
        assert_eq!(
            previous_path(Path::new("/home/u/.local/bin/umbriel-config")),
            Path::new("/home/u/.local/bin/umbriel-config.prev")
        );
    }
}
