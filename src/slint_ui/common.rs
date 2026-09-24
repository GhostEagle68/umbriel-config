//! Helpers shared across page modules: chain-document access, saved-state
//! diffing, ownership lookups, and small display utilities.

use super::*;

/// The doc at a chain index (include i = i, main = includes.docs.len()).
pub(super) fn doc_at(shell: &Shell, file_index: usize) -> &ConfigDocument {
    if file_index == shell.includes.docs.len() {
        &shell.doc
    } else {
        &shell.includes.docs[file_index].doc
    }
}

pub(super) fn doc_at_mut(shell: &mut Shell, file_index: usize) -> &mut ConfigDocument {
    if file_index == shell.includes.docs.len() {
        &mut shell.doc
    } else {
        &mut shell.includes.docs[file_index].doc
    }
}

/// Docs in umbriel's precedence: includes first, the main file last (it
/// wins). Same indexing as `doc_at` — keybind source_file indices refer
/// to it.
pub(super) fn chain_docs(shell: &Shell) -> Vec<&ConfigDocument> {
    let mut docs: Vec<&ConfigDocument> = shell.includes.docs.iter().map(|inc| &inc.doc).collect();
    docs.push(&shell.doc);
    docs
}

/// File paths in chain order (includes first, main last), parallel to
/// `chain_docs`.
pub(super) fn chain_paths(shell: &Shell) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = shell
        .includes
        .docs
        .iter()
        .map(|inc| inc.path.clone())
        .collect();
    paths.push(shell.path.clone());
    paths
}

/// Per-doc leaf-path sets over the include chain: includes in order, main
/// last (chain indexing: include i = i, main = includes.docs.len()).
pub(super) fn chain_path_sets(shell: &Shell) -> Vec<BTreeSet<String>> {
    let mut sets: Vec<BTreeSet<String>> = shell
        .includes
        .docs
        .iter()
        .map(|inc| inc.doc.value_paths().into_iter().collect())
        .collect();
    sets.push(shell.doc.value_paths().into_iter().collect());
    sets
}

/// Effective home of a dotted path: the last document in chain order
/// that sets it. Umbriel applies includes in list order and the main
/// file last, and "values are replaced by the last file that sets
/// them" (docs/user/configuration.md), so main wins, then later
/// includes over earlier ones.
pub(super) fn entry_home(sets: &[BTreeSet<String>], dotted: &str) -> Option<usize> {
    sets.iter().rposition(|set| set.contains(dotted))
}

/// Sidebar file labels: includes in chain order, main last (chain indexing
/// convention: include i = i, main = includes.docs.len()).
pub(super) fn setting_labels(shell: &Shell) -> Vec<SharedString> {
    let mut labels: Vec<SharedString> = shell
        .includes
        .docs
        .iter()
        .map(|doc| SharedString::from(doc.label.as_str()))
        .collect();
    let main = shell
        .path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.toml".to_owned());
    labels.push(SharedString::from(format!("{main} (main)")));
    labels
}

/// Keys whose state differs from the saved snapshot, as `(key,
/// current value)` — `None` marks a deletion: the snapshot has the key
/// but the doc no longer does (e.g. a shader cleared back to no
/// shader). Deletions are changes like any other; the save popup,
/// count, and discard all go through this one diff.
pub(super) fn diff_against_saved(
    current: &BTreeMap<String, String>,
    saved: Option<&BTreeMap<String, String>>,
) -> Vec<(String, Option<String>)> {
    let mut diff: Vec<(String, Option<String>)> = current
        .iter()
        .filter(|(key, value)| saved.and_then(|values| values.get(*key)) != Some(*value))
        .map(|(key, value)| (key.clone(), Some(value.clone())))
        .collect();
    if let Some(saved) = saved {
        for key in saved.keys() {
            if !current.contains_key(key) {
                diff.push((key.clone(), None));
            }
        }
    }
    diff
}

/// Number of chain keys whose current state differs from the saved
/// snapshot (newly created keys and deletions count too).
pub(super) fn changed_count(shell: &Shell) -> i32 {
    let main = shell.includes.docs.len();
    let mut count = 0;
    for i in 0..=main {
        let saved = shell.saved.get(i);
        let current: BTreeMap<String, String> =
            doc_at(shell, i).leaf_values().into_iter().collect();
        count += diff_against_saved(&current, saved).len() as i32;
    }
    count
}

/// Restore one key to its saved on-disk value; brand-new keys are
/// removed. A deleted key still lives in its snapshot — that decides
/// which document gets it back.
pub(super) fn reset_key(shell: &mut Shell, key: &str) {
    let sets = chain_path_sets(shell);
    let main = shell.includes.docs.len();
    let home = match entry_home(&sets, key) {
        Some(home) => home,
        // The key is gone from every document; its snapshot knows where
        // it was before the delete.
        None => match shell
            .saved
            .iter()
            .position(|values| values.contains_key(key))
        {
            Some(home) => home,
            None => return,
        },
    };
    let doc = if home == main {
        &mut shell.doc
    } else {
        &mut shell.includes.docs[home].doc
    };
    // The saved item comes back as it is on disk; a new key is dropped
    // with the tables it created, so the file is left as it was.
    let parts: Vec<&str> = key.split('.').collect();
    doc.revert_leaf(&parts);
}

pub(super) fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// "hot_corners" → "Hot corners".
pub(super) fn prettify(name: &str) -> String {
    let mut owned = name.replace('_', " ");
    if let Some(first) = owned.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    owned
}

/// A card's expansion: the user's choice wins, else the default
/// (settings and monitor cards open, rule cards open only when alone).
pub(super) fn card_expanded(shell: &Shell, key: &str, default: bool) -> bool {
    shell.card_expanded.get(key).copied().unwrap_or(default)
}

/// Row scaffold shared by every fixed-vocabulary editor row: owned by
/// the main config, no metadata yet.
pub(super) fn blank_row(key: String, label: &str, home: i32) -> SettingRow {
    SettingRow {
        label: label.into(),
        value: String::new().into(),
        key: key.into(),
        kind: ValueKind::Text,
        choices: Rc::new(VecModel::<SharedString>::from(Vec::new())).into(),
        swatch: slint::Color::from_argb_u8(0, 0, 0, 0).into(),
        checked: false,
        hint: String::new().into(),
        min: 0.0,
        max: 0.0,
        home,
        home_label: String::new().into(),
        changed: false,
        available: false,
        is_new: false,
        preview: String::new().into(),
        error: String::new().into(),
    }
}

pub(super) fn status_line(shell: &Shell) -> SharedString {
    if !shell.healthy {
        let reason = shell.load_error.as_deref().unwrap_or("unknown error");
        return format!("config failed to load: {reason}").into();
    }
    let modified = if shell.doc.is_modified() {
        "modified"
    } else {
        "clean"
    };
    format!(
        "{} settings • {} included file(s) • main {}",
        shell.schema.len(),
        shell.includes.docs.len(),
        modified,
    )
    .into()
}

/// Home-collapsed path for display: `$HOME/…` becomes `~/…`.
pub(super) fn pretty_path(path: &Path, env: &discovery::Env) -> String {
    let text = path.to_string_lossy().into_owned();
    match env.home.as_deref() {
        Some(home) => {
            let home = home.to_string_lossy();
            match text.strip_prefix(home.as_ref()) {
                Some(rest) => format!("~{rest}"),
                None => text,
            }
        }
        None => text,
    }
}

/// Mirror the include chain into the Settings page's file list.
pub(super) fn refresh_include_files(app: &AppWindow, shell: &Shell, env: &discovery::Env) {
    let files: Vec<SharedString> = shell
        .includes
        .docs
        .iter()
        .map(|inc| pretty_path(&inc.path, env).into())
        .collect();
    app.set_include_files(Rc::new(VecModel::from(files)).into());
}

/// Top-level section names (sorted, deduplicated) across the schema.
pub(super) fn section_names(entries: &[schema::Entry]) -> Vec<SharedString> {
    let mut names: Vec<String> = entries
        .iter()
        .map(|entry| {
            entry
                .section
                .split('.')
                .next()
                .unwrap_or("other")
                .to_owned()
        })
        .collect();
    names.sort();
    names.dedup();
    names.into_iter().map(SharedString::from).collect()
}

/// Dropdown vocabulary for Choice kinds; empty for everything else.
pub(super) fn choice_model(kind: &schema::Kind) -> slint::ModelRc<SharedString> {
    let choices: Vec<SharedString> = match kind {
        schema::Kind::Choice(values) | schema::Kind::OpenChoice(values) => {
            values.iter().map(SharedString::from).collect()
        }
        schema::Kind::Curve => schema::BUILTIN_CURVES
            .iter()
            .map(|name| SharedString::from(*name))
            .collect(),
        _ => Vec::new(),
    };
    Rc::new(VecModel::from(choices)).into()
}

/// Newest commit of a GitHub repo via the commits API:
/// (full sha, first message line, author date). `None` when GitHub is
/// unreachable — callers treat that as "unknown", never as "outdated".
pub(super) fn github_latest_commit(url: &str) -> Option<(String, String, String)> {
    #[derive(serde::Deserialize)]
    struct Commit {
        sha: String,
        commit: Detail,
    }
    #[derive(serde::Deserialize)]
    struct Detail {
        message: String,
        author: Author,
    }
    #[derive(serde::Deserialize)]
    struct Author {
        date: String,
    }
    let commits: Vec<Commit> = ureq::get(url)
        .header("User-Agent", "umbriel-config")
        .config()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    let latest = commits.into_iter().next()?;
    let message = latest
        .commit
        .message
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    Some((latest.sha, message, latest.commit.author.date))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_set(keys: &[&str]) -> BTreeSet<String> {
        keys.iter().map(|key| (*key).to_owned()).collect()
    }

    #[test]
    fn entry_home_is_the_last_file_that_sets_the_key() {
        // Chain order: include a, include b, main.
        let sets = [key_set(&["x", "y"]), key_set(&["x"]), key_set(&["z"])];
        assert_eq!(
            entry_home(&sets, "x"),
            Some(1),
            "later include beats earlier"
        );
        assert_eq!(entry_home(&sets, "y"), Some(0));
        assert_eq!(entry_home(&sets, "z"), Some(2));
        let everywhere = [key_set(&["x"]), key_set(&["x"]), key_set(&["x"])];
        assert_eq!(
            entry_home(&everywhere, "x"),
            Some(2),
            "main beats every include"
        );
        assert_eq!(entry_home(&sets, "missing"), None);
    }

    #[test]
    fn diff_against_saved_includes_deletions() {
        // The shader-clear regression: a key removed from the doc must
        // still count as a change, or Save finds "nothing to save".
        let saved_map = BTreeMap::from([
            ("a.changed".to_owned(), "1".to_owned()),
            ("b.kept".to_owned(), "2".to_owned()),
            ("c.removed".to_owned(), "3".to_owned()),
        ]);
        let current = BTreeMap::from([
            ("a.changed".to_owned(), "9".to_owned()),
            ("b.kept".to_owned(), "2".to_owned()),
        ]);
        assert_eq!(
            diff_against_saved(&current, Some(&saved_map)),
            vec![
                ("a.changed".to_owned(), Some("9".to_owned())),
                ("c.removed".to_owned(), None),
            ]
        );
        // No snapshot yet: every current key counts as new.
        assert_eq!(diff_against_saved(&current, None).len(), 2);
    }
}
