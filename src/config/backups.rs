//! Config backups: dated snapshot runs under the state directory. Every
//! save (and the manual "Back up now") writes one run holding the on-disk
//! content of every chain file about to be overwritten, so a bad save is
//! always reversible. Pure primitives — the shell decides when to call
//! them and maps backup file names back onto the include chain.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::discovery;

/// Default backup base: `$XDG_STATE_HOME` (else `$HOME/.local/state`) +
/// `umbriel-config/backups`. Deliberately outside the config directory:
/// restore must survive mistakes made in that directory.
pub fn default_base(env: &discovery::Env) -> PathBuf {
    let base = if let Some(state_home) = env.xdg_state_home.as_deref() {
        PathBuf::from(state_home)
    } else {
        let home = env.home.as_deref().unwrap_or_else(|| OsStr::new(""));
        Path::new(home).join(".local/state")
    };
    base.join("umbriel-config/backups")
}

/// One snapshot run, as the restore browser shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct RunInfo {
    /// Directory name: `YYYY-MM-DDTHH-MM-SS` (UTC), `-N` suffixed when
    /// two runs land in the same second.
    pub id: String,
    /// The main config path the run was taken for.
    pub origin: String,
    /// What triggered the run: `"save"` or `"manual"`.
    pub trigger: String,
    /// Backup file names in the run, sorted.
    pub files: Vec<String>,
}

/// Write one run: every `(path, on-disk content)` pair is stored under
/// its file name, plus an `origin` file whose first line is the main
/// config path and whose second line is `trigger = <trigger>`. Returns
/// the run id. Callers treat backup failures as best-effort and must
/// never let them block a save.
pub fn snapshot_run(
    base: &Path,
    main_path: &Path,
    trigger: &str,
    files: &[(PathBuf, String)],
) -> std::io::Result<String> {
    std::fs::create_dir_all(base)?;
    let id = next_run_id(&run_ids(base), &utc_stamp(now_secs()));
    let dir = base.join(&id);
    std::fs::create_dir_all(&dir)?;
    for (path, content) in files {
        let name = path.file_name().unwrap_or_else(|| OsStr::new("file"));
        std::fs::write(dir.join(name), content)?;
    }
    std::fs::write(
        dir.join("origin"),
        format!("{}\ntrigger = {trigger}\n", main_path.display()),
    )?;
    Ok(id)
}

/// Runs newest-first. Directories that don't look like run ids are not
/// ours and are ignored, not fatal.
pub fn list_runs(base: &Path) -> Vec<RunInfo> {
    run_ids(base)
        .into_iter()
        .filter_map(|id| {
            let origin = std::fs::read_to_string(base.join(&id).join("origin")).ok()?;
            let mut lines = origin.lines();
            let main = lines.next()?.trim().to_owned();
            let trigger = lines
                .find_map(|line| line.strip_prefix("trigger = "))
                .unwrap_or("save")
                .trim()
                .to_owned();
            let mut files: Vec<String> = std::fs::read_dir(base.join(&id))
                .ok()?
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| name != "origin")
                .collect();
            files.sort();
            Some(RunInfo {
                id,
                origin: main,
                trigger,
                files,
            })
        })
        .collect()
}

/// Backup contents for one run: `(file name, content)` pairs, sorted.
pub fn read_run(base: &Path, id: &str) -> std::io::Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(base.join(id))? {
        let entry = entry?;
        let name = entry.file_name().into_string().unwrap_or_default();
        if name == "origin" {
            continue;
        }
        let content = std::fs::read_to_string(entry.path())?;
        out.push((name, content));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Delete the oldest runs beyond `keep`; returns how many were removed.
/// Per-run removal errors are ignored — pruning is best-effort, and a
/// lowered count takes effect on the next snapshot.
pub fn prune_runs(base: &Path, keep: usize) -> usize {
    let mut removed = 0;
    for id in run_ids(base).into_iter().skip(keep) {
        if std::fs::remove_dir_all(base.join(id)).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn run_ids(base: &Path) -> Vec<String> {
    let mut ids: Vec<String> = std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().into_string().ok())
        // `2026-09-09T10-00-00` plus optional `-N` collision suffixes —
        // the T at index 10 is the real marker.
        .filter(|name| name.len() > 10 && name.as_bytes()[10] == b'T')
        .collect();
    ids.sort();
    ids.reverse();
    ids
}

/// `stamp`, or the first free `stamp-N` once that second is taken.
fn next_run_id(existing: &[String], stamp: &str) -> String {
    let mut id = stamp.to_owned();
    let mut n = 2;
    while existing.iter().any(|e| e == &id) {
        id = format!("{stamp}-{n}");
        n += 1;
    }
    id
}

/// UTC wall-clock stamp for a run id. Colons become hyphens so the id is
/// a valid directory name on every filesystem.
fn utc_stamp(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    let rem = secs % 86_400;
    format!(
        "{y:04}-{m:02}-{d:02}T{h:02}-{mi:02}-{s:02}",
        h = rem / 3_600,
        mi = (rem % 3_600) / 60,
        s = rem % 60,
    )
}

/// Days since 1970-01-01 → (year, month, day), Howard Hinnant's
/// civil-from-days. The only date math in the codebase — update.rs's
/// once-a-day stamp uses raw epoch seconds and needs no formatting.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_base(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("umbriel-backups-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn utc_stamp_formats_known_epochs() {
        assert_eq!(utc_stamp(0), "1970-01-01T00-00-00");
        assert_eq!(utc_stamp(86_400), "1970-01-02T00-00-00");
        assert_eq!(utc_stamp(951_782_400), "2000-02-29T00-00-00");
        assert_eq!(utc_stamp(1_000_000_000), "2001-09-09T01-46-40");
    }

    #[test]
    fn next_run_id_suffixes_on_collision() {
        let id = "2026-09-09T10-00-00";
        assert_eq!(next_run_id(&[], id), id);
        assert_eq!(next_run_id(&[id.to_owned()], id), format!("{id}-2"));
        assert_eq!(
            next_run_id(&[id.to_owned(), format!("{id}-2")], id),
            format!("{id}-3")
        );
    }

    #[test]
    fn snapshot_list_read_prune_round_trip() {
        let base = temp_base("roundtrip");
        let files = vec![
            (PathBuf::from("/cfg/config.toml"), "a = 1\n".to_owned()),
            (PathBuf::from("/cfg/monitors.toml"), "b = 2\n".to_owned()),
        ];
        let id = snapshot_run(&base, Path::new("/cfg/config.toml"), "save", &files).unwrap();

        let runs = list_runs(&base);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, id);
        assert_eq!(runs[0].origin, "/cfg/config.toml");
        assert_eq!(runs[0].trigger, "save");
        assert_eq!(runs[0].files, vec!["config.toml", "monitors.toml"]);
        assert_eq!(
            read_run(&base, &id).unwrap()[0],
            ("config.toml".to_owned(), "a = 1\n".to_owned())
        );

        // A second run in the same second gets a suffix; pruning keeps
        // the newest.
        let id2 = snapshot_run(&base, Path::new("/cfg/config.toml"), "manual", &files).unwrap();
        assert_ne!(id2, id);
        assert_eq!(prune_runs(&base, 1), 1);
        let runs = list_runs(&base);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, id2);
        assert_eq!(runs[0].trigger, "manual");

        std::fs::remove_dir_all(&base).ok();
    }
}
