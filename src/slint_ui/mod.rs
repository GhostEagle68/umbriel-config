//! Slint shell for umbriel-config; the migration plan lives in
//! `.design/ui-plan.md`. Phase 0: window chrome, sidebar navigation, and a
//! read-only status projection of the loaded config. The UI-agnostic library
//! does all real work — this module only presents it and forwards intents.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;

use slint::{
    CloseRequestResponse, ComponentHandle, LogicalSize, Model, SharedString, VecModel, WindowSize,
};
use umbriel_config::config::{
    discovery, document::ConfigDocument, includes, schema, settings as app_settings, validate,
};
use umbriel_config::update;

slint::include_modules!();

/// Shell state, re-homing the egui `App` fields with the same semantics.
/// Fields unused in Phase 0 become load-bearing in Phases 1–3.
struct Shell {
    path: PathBuf,
    doc: ConfigDocument,
    healthy: bool,
    load_error: Option<String>,
    schema: Vec<schema::Entry>,
    includes: includes::IncludeChain,
    // Per chain index: each doc's leaf values as last saved on disk. A row
    // whose current value differs from this snapshot is "changed".
    saved: Vec<BTreeMap<String, String>>,
}

impl Shell {
    fn load(path: &Path, env: &discovery::Env) -> Self {
        let (doc, healthy, load_error) = match ConfigDocument::load(path) {
            Ok(doc) => (doc, true, None),
            Err(err) => {
                // A missing file is a fresh start: saving creates it. Anything
                // else (a broken file) keeps saving disabled so it is never
                // overwritten from here.
                let healthy = err.is_not_found();
                let load_error = (!healthy).then_some(err.to_string());
                (
                    ConfigDocument::from_str("").expect("empty TOML parses"),
                    healthy,
                    load_error,
                )
            }
        };
        let schema = discovery::packaged_default(env)
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| schema::assemble(&text))
            .unwrap_or_default();
        let includes = includes::load_chain(&doc, path);
        let mut shell = Shell {
            path: path.to_path_buf(),
            doc,
            healthy,
            load_error,
            schema,
            includes,
            saved: Vec::new(),
        };
        shell.reset_saved();
        shell
    }

    /// Re-capture the on-disk baselines; a save resets the "changed" marks.
    #[allow(dead_code)] // wired when the save flow lands
    fn reset_saved(&mut self) {
        let main = self.includes.docs.len();
        self.saved = (0..=main)
            .map(|i| doc_at(self, i).leaf_values().into_iter().collect())
            .collect();
    }

    fn any_modified(&self) -> bool {
        self.doc.is_modified() || self.includes.docs.iter().any(|inc| inc.doc.is_modified())
    }
}

pub fn run(path: PathBuf) -> anyhow::Result<()> {
    let env = discovery::Env::from_process();
    let settings = app_settings::load(&env);
    let shell = Rc::new(RefCell::new(Shell::load(&path, &env)));

    let app = AppWindow::new().map_err(|err| anyhow::anyhow!("window creation failed: {err}"))?;

    {
        let shell = shell.borrow();
        app.set_config_path(shell.path.display().to_string().into());
        app.set_include_count(shell.includes.docs.len() as i32);
        app.set_include_note(shell.includes.notes.join("; ").into());
    }
    app.set_dirty(false);
    app.set_app_version(env!("CARGO_PKG_VERSION").into());
    app.set_check_updates_on_start(settings.check_updates_on_start);

    let sections = section_names(&shell.borrow().schema);
    if let Some(first) = sections.first() {
        app.set_current_section(first.clone());
        app.set_page_title(first.clone());
        refill_section(&app, &shell.borrow(), first);
    }
    app.set_sections(model(sections));
    app.set_status(status_line(&shell.borrow()));
    {
        // Destination picker model, main first: index 0 = main, i = include i-1.
        let shell = shell.borrow();
        let labels = file_labels(&shell);
        let main = labels.len() - 1;
        let mut destinations = vec![labels[main].clone()];
        destinations.extend(labels[..main].iter().cloned());
        app.set_destinations(Rc::new(VecModel::from(destinations)).into());
    }

    app.window().set_size(WindowSize::Logical(LogicalSize::new(
        settings.window_width as f32,
        settings.window_height as f32,
    )));

    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_section_selected(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            app.set_current_section(name.clone());
            app.set_page_title(name.clone());
            app.set_page(Page::Section);
            refill_section(&app, &shell, &name);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_search_edited(move |query| {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            let needle = query.trim().to_owned();
            if needle.is_empty() {
                // Empty box = leave search; restore the section overview.
                let section = app.get_current_section().to_string();
                app.set_page(Page::Section);
                app.set_page_title(section.clone().into());
                refill_section(&app, &shell, &section);
                return;
            }
            let sets = chain_path_sets(&shell);
            let labels = file_labels(&shell);
            let rows: Vec<SectionRow> = shell
                .schema
                .iter()
                .filter(|entry| schema::matches(entry, &needle))
                .map(|entry| {
                    let dotted = entry.path.join(".");
                    let home = entry_home(&sets, &dotted).unwrap_or(sets.len() - 1);
                    SectionRow {
                        label: entry.label.clone().into(),
                        home_label: labels.get(home).cloned().unwrap_or_default(),
                        home_section: entry.section.split('.').next().unwrap_or("other").into(),
                    }
                })
                .collect();
            app.set_search_rows(Rc::new(VecModel::from(rows)).into());
            app.set_page(Page::Search);
            app.set_page_title(SharedString::from(needle));
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_view_file(move |index| {
            let Some(app) = weak.upgrade() else { return };
            if index < 0 {
                return;
            }
            let index = index as usize;
            let shell = shell.borrow();
            let main = shell.includes.docs.len();
            if index > main {
                return;
            }
            let label = file_labels(&shell)
                .get(index)
                .cloned()
                .unwrap_or_default()
                .to_string();
            let path = if index == main {
                shell.path.display().to_string()
            } else {
                shell.includes.docs[index].path.display().to_string()
            };
            let doc = doc_at(&shell, index);
            let modified = if doc.is_modified() {
                " · unsaved edits included"
            } else {
                ""
            };
            app.set_popup_file_title(format!("{label}{modified}").into());
            app.set_popup_file_path(path.into());
            app.set_popup_file_text(doc.text().into());
            app.set_show_file_popup(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_save_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            let entries = build_save_entries(&shell);
            if entries.is_empty() {
                app.set_status("Nothing to save.".into());
                return;
            }
            app.set_save_entries(Rc::new(VecModel::from(entries)).into());
            app.set_show_save_popup(true);
        });
    }
    {
        let weak = app.as_weak();
        app.on_save_dest_chosen(move |key, destination| {
            let Some(app) = weak.upgrade() else { return };
            let entries = app.get_save_entries();
            let Some(entries) = entries.as_any().downcast_ref::<VecModel<SaveEntry>>() else {
                return;
            };
            for i in 0..entries.row_count() {
                if let Some(mut entry) = entries.row_data(i)
                    && entry.key.as_str() == key.as_str()
                {
                    entry.dest_label = destination.to_string().into();
                    entries.set_row_data(i, entry);
                    break;
                }
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_reset_entry(move |key| {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut shell = shell.borrow_mut();
                reset_key(&mut shell, &key);
            }
            let shell = shell.borrow();
            let entries = build_save_entries(&shell);
            app.set_show_save_popup(!entries.is_empty());
            app.set_save_entries(Rc::new(VecModel::from(entries)).into());
            app.set_changed_count(changed_count(&shell));
            refresh_row(&app, &shell, &key);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_discard_all(move || {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut shell = shell.borrow_mut();
                let main = shell.includes.docs.len();
                let mut changed: Vec<String> = Vec::new();
                for i in 0..=main {
                    let saved = shell.saved.get(i);
                    let current: BTreeMap<String, String> =
                        doc_at(&shell, i).leaf_values().into_iter().collect();
                    for (key, value) in &current {
                        if saved.and_then(|values| values.get(key)) != Some(value) {
                            changed.push(key.clone());
                        }
                    }
                }
                for key in &changed {
                    reset_key(&mut shell, key);
                }
            }
            let shell = shell.borrow();
            refill_section(&app, &shell, app.get_current_section().as_str());
            app.set_show_save_popup(false);
            app.set_changed_count(changed_count(&shell));
            app.set_status("Discarded all unsaved changes.".into());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_save_confirmed(move || {
            let Some(app) = weak.upgrade() else { return };
            let mut shell = shell.borrow_mut();

            // The popup's entries, with each key's chosen destination.
            let model = app.get_save_entries();
            let Some(model) = model.as_any().downcast_ref::<VecModel<SaveEntry>>() else {
                return;
            };
            let entries: Vec<SaveEntry> = (0..model.row_count())
                .filter_map(|i| model.row_data(i))
                .collect();
            if entries.is_empty() {
                return;
            }

            let labels = file_labels(&shell);
            let sets = chain_path_sets(&shell);
            let main = shell.includes.docs.len();

            // Apply per-key destinations. Moving a key = write the value
            // into the target file first, then remove it from the old one —
            // a value is never lost mid-move.
            for entry in &entries {
                let Some(dest) = labels
                    .iter()
                    .position(|label| label.as_str() == entry.dest_label.as_str())
                else {
                    continue;
                };
                let Some(home) = entry_home(&sets, &entry.key) else {
                    continue;
                };
                if home == dest {
                    continue;
                }
                let parts: Vec<&str> = entry.key.split('.').collect();
                let target = if dest == main {
                    &mut shell.doc
                } else {
                    &mut shell.includes.docs[dest].doc
                };
                if target.set_leaf_text(&entry.key, &entry.value) {
                    let source = if home == main {
                        &mut shell.doc
                    } else {
                        &mut shell.includes.docs[home].doc
                    };
                    source.remove_table(&parts);
                }
            }

            // Write every modified doc, then validate through umbriel.
            let mut saved_files = 0;
            for inc in &mut shell.includes.docs {
                if inc.doc.is_modified() {
                    if let Err(err) = inc.doc.save(&inc.path) {
                        app.set_status(format!("save failed: {err}").into());
                        return;
                    }
                    saved_files += 1;
                }
            }
            if shell.doc.is_modified() {
                let path = shell.path.clone();
                if let Err(err) = shell.doc.save(&path) {
                    app.set_status(format!("save failed: {err}").into());
                    return;
                }
                saved_files += 1;
            }
            let report = validate::validate(&shell.path);
            shell.reset_saved();

            app.set_dirty(false);
            app.set_changed_count(0);
            match report {
                Ok(report) if report.is_ok() => {
                    app.set_validate_note(String::new().into());
                    app.set_status(
                        format!("Saved {saved_files} file(s); umbriel has validated the config.")
                            .into(),
                    );
                }
                Ok(report) => {
                    let messages: Vec<String> = report
                        .diagnostics
                        .iter()
                        .map(|d| d.message().to_owned())
                        .collect();
                    app.set_validate_note(messages.join("; ").into());
                    app.set_status(
                        format!("Saved {saved_files} file(s), but umbriel has complaints — see the banner.")
                            .into(),
                    );
                }
                Err(err) => {
                    app.set_validate_note(format!("umbriel could not be run: {err}").into());
                    app.set_status(format!("Saved {saved_files} file(s) without validation.").into());
                }
            }
            let section = app.get_current_section().to_string();
            refill_section(&app, &shell, &section);
        });
    }
    {
        let weak = app.as_weak();
        let count = shell.borrow().schema.len();
        app.on_sync_schema_requested(move || {
            if let Some(app) = weak.upgrade() {
                app.set_status(format!("{count} settings in the current schema").into());
            }
        });
    }
    {
        let weak = app.as_weak();
        app.on_settings_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_page(Page::Settings);
            // Fetch the compositor's latest commit once per run; the
            // "Checking…" note doubles as the in-flight guard.
            if app.get_upstream_note().is_empty() {
                let weak = app.as_weak();
                app.set_upstream_note("Checking…".into());
                std::thread::spawn(move || {
                    let result = fetch_latest_commit();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = weak.upgrade() {
                            let note = match result {
                                Ok(note) => note,
                                Err(err) => {
                                    format!("Couldn't fetch the latest commit: {err}")
                                }
                            };
                            app.set_upstream_note(note.into());
                        }
                    });
                });
            }
        });
    }
    {
        let weak = app.as_weak();
        app.on_check_updates_requested(move || {
            start_update_check(weak.clone(), None);
        });
    }
    {
        let env = env.clone();
        app.on_check_updates_toggled(move |checked| {
            let mut settings = app_settings::load(&env);
            settings.check_updates_on_start = checked;
            let _ = app_settings::store(&env, &settings);
        });
    }
    app.on_open_url(|url| {
        let _ = std::process::Command::new("xdg-open")
            .arg(url.as_str())
            .spawn();
    });
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_exit_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            store_window_settings(&app, &env);
            let _ = app.hide();
            let _ = slint::quit_event_loop();
        });
    }
    // The WM ✕ goes through here: guard unsaved edits, else persist the
    // window size and let the window close.
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.window().on_close_requested(move || {
            let Some(app) = weak.upgrade() else {
                return CloseRequestResponse::HideWindow;
            };
            if app.get_dirty() {
                app.set_show_exit_confirm(true);
                CloseRequestResponse::KeepWindowShown
            } else {
                store_window_settings(&app, &env);
                CloseRequestResponse::HideWindow
            }
        });
    }

    {
        {
            let weak = app.as_weak();
            let shell = Rc::clone(&shell);
            app.on_set_value(move |key, value| {
                let Some(app) = weak.upgrade() else { return };
                commit_edit(&app, &shell, &key, &value);
            });
        }
        {
            let weak = app.as_weak();
            let shell = Rc::clone(&shell);
            app.on_set_slider_value(move |key, value| {
                let Some(app) = weak.upgrade() else { return };
                let raw = slider_text(&shell.borrow().schema, &key, value);
                commit_edit(&app, &shell, &key, &raw);
            });
        }
        {
            let weak = app.as_weak();
            let shell = Rc::clone(&shell);
            app.on_slider_preview(move |key, value| {
                let Some(app) = weak.upgrade() else { return };
                let raw = slider_text(&shell.borrow().schema, &key, value);
                preview_row(&app, &key, &raw);
            });
        }
        /// Shared write path for text edits and slider releases: format the raw
        /// input per Kind, write through set_leaf_text, then refresh the row.
        fn commit_edit(app: &AppWindow, shell: &Rc<RefCell<Shell>>, key: &str, raw: &str) {
            let formatted = {
                let shell = shell.borrow();
                let kind = shell
                    .schema
                    .iter()
                    .find(|entry| entry.path.join(".") == key)
                    .map(|entry| &entry.kind);
                commit_value(kind, raw)
            };
            let value_text = match formatted {
                Ok(value_text) => value_text,
                Err(err) => {
                    app.set_status(err.into());
                    return;
                }
            };
            let home = {
                let shell = shell.borrow();
                let sets = chain_path_sets(&shell);
                // Writes follow ownership: an existing key is edited where
                // it lives. Where a brand-new key goes is chosen in the
                // save popup.
                entry_home(&sets, key).unwrap_or(shell.includes.docs.len())
            };
            let accepted = {
                let mut shell = shell.borrow_mut();
                let main = shell.includes.docs.len();
                let doc: &mut ConfigDocument = if home == main {
                    &mut shell.doc
                } else if let Some(inc) = shell.includes.docs.get_mut(home) {
                    &mut inc.doc
                } else {
                    return;
                };
                doc.set_leaf_text(key, &value_text)
            };
            if !accepted {
                app.set_status(format!("umbriel would reject {key} = {value_text}").into());
                return;
            }
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            refresh_row(app, &shell, key);
            app.set_changed_count(changed_count(&shell));
        }
    }

    if settings.check_updates_on_start && update::should_auto_check(&env) {
        start_update_check(app.as_weak(), Some(env.clone()));
    }

    app.run()
        .map_err(|err| anyhow::anyhow!("event loop failed: {err}"))?;
    Ok(())
}

/// Persist the app settings with the current window size (logical px).
/// The toggle is read from the live property so a change made on the
/// Settings page survives exit.
fn store_window_settings(app: &AppWindow, env: &discovery::Env) {
    let scale = app.window().scale_factor();
    let size = app.window().size();
    let _ = app_settings::store(
        env,
        &app_settings::Settings {
            check_updates_on_start: app.get_check_updates_on_start(),
            window_width: (size.width as f32 / scale) as u32,
            window_height: (size.height as f32 / scale) as u32,
        },
    );
}

/// Background update check; results land on the UI thread. `env` is only
/// passed for automatic startup checks — a manual check doesn't move the
/// once-a-day stamp (egui parity).
fn start_update_check(weak: slint::Weak<AppWindow>, env: Option<discovery::Env>) {
    let Some(app) = weak.upgrade() else { return };
    app.set_update_note("Checking…".into());
    std::thread::spawn(move || {
        let result = update::check();
        if result.is_ok()
            && let Some(env) = env.as_ref()
        {
            update::mark_checked(env);
        }
        let _ = slint::invoke_from_event_loop(move || {
            let Some(app) = weak.upgrade() else { return };
            match result {
                Ok(update::Verdict::UpToDate) => app.set_update_note("Up to date.".into()),
                Ok(update::Verdict::UpdateAvailable(version)) => {
                    app.set_update_available(true);
                    app.set_update_note(
                        format!("Version {version} available — see the releases page.").into(),
                    );
                }
                Err(err) => app.set_update_note(format!("Couldn't check: {err}").into()),
            }
        });
    });
}

/// Latest commit of the Umbriel compositor, one display line:
/// short sha • date • first message line. Offline shows the error and
/// the next app run tries again.
fn fetch_latest_commit() -> Result<String, String> {
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
    let commits: Vec<Commit> =
        ureq::get("https://api.github.com/repos/noctalia-dev/umbriel/commits?per_page=1")
            .set("User-Agent", "umbriel-config")
            .timeout(std::time::Duration::from_secs(10))
            .call()
            .map_err(|err| err.to_string())?
            .into_json()
            .map_err(|err| err.to_string())?;
    let Some(latest) = commits.first() else {
        return Err("no commits found".to_owned());
    };
    let sha: String = latest.sha.chars().take(7).collect();
    let message = latest.commit.message.lines().next().unwrap_or_default();
    let date = latest.commit.author.date.get(..10).unwrap_or_default();
    Ok(format!("{sha} • {date} • {message}"))
}

/// Top-level section names (sorted, deduplicated) across the schema.
fn section_names(entries: &[schema::Entry]) -> Vec<SharedString> {
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

/// Sidebar file labels: includes in chain order, main last (chain indexing
/// convention: include i = i, main = includes.docs.len()).
fn file_labels(shell: &Shell) -> Vec<SharedString> {
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

fn status_line(shell: &Shell) -> SharedString {
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

fn model(strings: Vec<SharedString>) -> slint::ModelRc<SharedString> {
    Rc::new(VecModel::from(strings)).into()
}

/// Per-doc leaf-path sets over the include chain: includes in order, main
/// last (chain indexing: include i = i, main = includes.docs.len()).
fn chain_path_sets(shell: &Shell) -> Vec<BTreeSet<String>> {
    let mut sets: Vec<BTreeSet<String>> = shell
        .includes
        .docs
        .iter()
        .map(|inc| inc.doc.value_paths().into_iter().collect())
        .collect();
    sets.push(shell.doc.value_paths().into_iter().collect());
    sets
}

/// Effective home of a dotted path: main wins over includes, earlier
/// includes win over later (merge order — mirrors egui's entry_home).
fn entry_home(sets: &[BTreeSet<String>], dotted: &str) -> Option<usize> {
    let main = sets.len() - 1;
    if sets[main].contains(dotted) {
        return Some(main);
    }
    sets.iter().position(|set| set.contains(dotted))
}

/// The doc at a chain index (include i = i, main = includes.docs.len()).
fn doc_at(shell: &Shell, file_index: usize) -> &ConfigDocument {
    if file_index == shell.includes.docs.len() {
        &shell.doc
    } else {
        &shell.includes.docs[file_index].doc
    }
}

/// Number of chain keys whose current value differs from the saved
/// snapshot (newly created keys count too).
fn changed_count(shell: &Shell) -> i32 {
    let main = shell.includes.docs.len();
    let mut count = 0;
    for i in 0..=main {
        let saved = shell.saved.get(i);
        let current: BTreeMap<String, String> =
            doc_at(shell, i).leaf_values().into_iter().collect();
        for (key, value) in &current {
            if saved.and_then(|values| values.get(key)) != Some(value) {
                count += 1;
            }
        }
    }
    count
}

/// The current diff across the chain, in save-popup form.
fn build_save_entries(shell: &Shell) -> Vec<SaveEntry> {
    let main = shell.includes.docs.len();
    let labels = file_labels(shell);
    let mut entries: Vec<SaveEntry> = Vec::new();
    for i in 0..=main {
        let saved = shell.saved.get(i);
        let current: BTreeMap<String, String> =
            doc_at(shell, i).leaf_values().into_iter().collect();
        for (key, value) in &current {
            if saved.and_then(|values| values.get(key)) == Some(value) {
                continue;
            }
            let label = shell
                .schema
                .iter()
                .find(|entry| entry.path.join(".") == key.as_str())
                .map(|entry| entry.label.clone())
                .unwrap_or_else(|| key.clone());
            entries.push(SaveEntry {
                key: key.clone().into(),
                label: label.into(),
                value: value.clone().into(),
                dest_label: labels.get(i).cloned().unwrap_or_default(),
                dest_index: i as i32,
            });
        }
    }
    entries
}

/// Restore one key to its saved on-disk value; brand-new keys are removed.
fn reset_key(shell: &mut Shell, key: &str) {
    let sets = chain_path_sets(shell);
    let Some(home) = entry_home(&sets, key) else {
        return;
    };
    let main = shell.includes.docs.len();
    let saved_repr = shell
        .saved
        .get(home)
        .and_then(|values| values.get(key))
        .cloned();
    let doc = if home == main {
        &mut shell.doc
    } else {
        &mut shell.includes.docs[home].doc
    };
    match saved_repr {
        Some(repr) => {
            doc.set_leaf_text(key, &repr);
        }
        None => {
            let parts: Vec<&str> = key.split('.').collect();
            doc.remove_table(&parts);
        }
    }
}

/// Refill the section page: every setting of `section` across the chain.
fn refill_section(app: &AppWindow, shell: &Shell, section: &str) {
    app.set_file_groups(Rc::new(VecModel::from(section_groups(shell, section))).into());
    app.set_changed_count(changed_count(shell));
}

/// One section page: every schema entry of the top-level `section` across
/// the whole include chain, grouped by sub-section. Unset keys read "—"
/// and commit to main (the inline destination picker is a fast-follow).
fn section_groups(shell: &Shell, section: &str) -> Vec<FileGroup> {
    let sets = chain_path_sets(shell);
    let labels = file_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();

    let mut groups: BTreeMap<String, Vec<FileRow>> = BTreeMap::new();
    for entry in &shell.schema {
        if entry.section.split('.').next() != Some(section) {
            continue;
        }
        let dotted = entry.path.join(".");
        let home = entry_home(&sets, &dotted);
        let mut row = file_row(doc_at(shell, home.unwrap_or(main)), entry);
        row.available = home.is_none();
        row.home = home.map_or(-1, |home| home as i32);
        if let Some(label) = home.and_then(|home| labels.get(home)) {
            row.home_label = label.clone();
        }
        row.changed = match home {
            Some(home) => {
                current.get(home).and_then(|values| values.get(&dotted))
                    != shell.saved.get(home).and_then(|values| values.get(&dotted))
            }
            None => false,
        };
        groups.entry(entry.section.clone()).or_default().push(row);
    }

    let mut out: Vec<FileGroup> = groups
        .into_iter()
        .map(|(title, rows)| FileGroup {
            title: title.into(),
            rows: Rc::new(VecModel::from(rows)).into(),
        })
        .collect();

    // Keys beyond the schema fold onto the section that owns them (the
    // old Other-settings sweep): one read-only row per key, owned like
    // any other row. Only keys of this section are shown.
    let docs: Vec<&ConfigDocument> = shell.includes.docs.iter().map(|inc| &inc.doc).collect();
    let claims = schema::managed_claims(&docs);
    let schema_keys = schema::key_set(&shell.schema);
    let mut other: BTreeMap<String, FileRow> = BTreeMap::new();
    for (i, set) in sets.iter().enumerate() {
        let paths: Vec<String> = set.iter().cloned().collect();
        for path in schema::uncovered(&paths, &schema_keys, &claims) {
            if path.split('.').next() != Some(section) {
                continue;
            }
            // One row per key: the file that owns it (shadowed copies
            // in later includes are skipped).
            let Some(home) = entry_home(&sets, &path) else {
                continue;
            };
            if home != i || other.contains_key(&path) {
                continue;
            }
            let value = current
                .get(home)
                .and_then(|values| values.get(&path))
                .map(|raw| strip_decor(raw))
                .unwrap_or_else(|| "—".to_owned());
            other.insert(
                path.clone(),
                FileRow {
                    label: path.clone().into(),
                    value: value.into(),
                    key: path.clone().into(),
                    kind: ValueKind::Unset,
                    choices: choice_model(&schema::Kind::Text),
                    swatch: slint::Color::from_argb_u8(0, 0, 0, 0).into(),
                    checked: false,
                    hint: String::new().into(),
                    min: 0.0,
                    max: 0.0,
                    home: home as i32,
                    home_label: labels.get(home).cloned().unwrap_or_default(),
                    changed: current.get(home).and_then(|values| values.get(&path))
                        != shell.saved.get(home).and_then(|values| values.get(&path)),
                    available: false,
                },
            );
        }
    }
    if !other.is_empty() {
        out.push(FileGroup {
            title: "other".into(),
            rows: Rc::new(VecModel::from(other.into_values().collect::<Vec<_>>())).into(),
        });
    }
    out
}

/// One schema row, editor-ready: typed value, checked state, swatch, hint.
fn file_row(doc: &ConfigDocument, entry: &schema::Entry) -> FileRow {
    let value = typed_value(doc, entry).unwrap_or_else(|| "—".to_owned());
    let parts: Vec<&str> = entry.path.iter().map(String::as_str).collect();
    let (min, max) = kind_bounds(&entry.kind);
    FileRow {
        label: entry.label.clone().into(),
        value: value.clone().into(),
        key: entry.path.join(".").into(),
        kind: value_kind(&entry.kind),
        choices: choice_model(&entry.kind),
        swatch: if matches!(entry.kind, schema::Kind::Color) {
            swatch_for(&value)
        } else {
            slint::Color::from_argb_u8(0, 0, 0, 0).into()
        },
        checked: doc.get_bool(&parts).unwrap_or(false),
        hint: entry_hint(entry).into(),
        min,
        max,
        home: -1,
        home_label: String::new().into(),
        changed: false,
        available: false,
    }
}

fn refresh_row(app: &AppWindow, shell: &Shell, key: &str) {
    let sets = chain_path_sets(shell);
    let labels = file_labels(shell);
    let Some(entry) = shell
        .schema
        .iter()
        .find(|entry| entry.path.join(".") == key)
    else {
        return;
    };
    let home = entry_home(&sets, key).unwrap_or(shell.includes.docs.len());
    let current: BTreeMap<String, String> = doc_at(shell, home).leaf_values().into_iter().collect();
    let mut row = file_row(doc_at(shell, home), entry);
    row.home = home as i32;
    if let Some(label) = labels.get(home) {
        row.home_label = label.clone();
    }
    row.changed = current.get(key) != shell.saved.get(home).and_then(|values| values.get(key));

    let groups = app.get_file_groups();
    let Some(groups) = groups.as_any().downcast_ref::<VecModel<FileGroup>>() else {
        return;
    };
    for gi in 0..groups.row_count() {
        let Some(group) = groups.row_data(gi) else {
            continue;
        };
        let Some(rows) = group.rows.as_any().downcast_ref::<VecModel<FileRow>>() else {
            continue;
        };
        for ri in 0..rows.row_count() {
            let Some(old) = rows.row_data(ri) else {
                continue;
            };
            if old.key.as_str() == key {
                // Same text = nothing to re-render; keeps the editor's focus.
                if old.value != row.value {
                    rows.set_row_data(ri, row);
                }
                return;
            }
        }
    }
}

/// Dropdown vocabulary for Choice kinds; empty for everything else.
fn choice_model(kind: &schema::Kind) -> slint::ModelRc<SharedString> {
    let choices: Vec<SharedString> = match kind {
        schema::Kind::Choice(values) => values.iter().map(SharedString::from).collect(),
        schema::Kind::Curve => schema::BUILTIN_CURVES
            .iter()
            .map(|name| SharedString::from(*name))
            .collect(),
        _ => Vec::new(),
    };
    Rc::new(VecModel::from(choices)).into()
}

fn value_kind(kind: &schema::Kind) -> ValueKind {
    match kind {
        schema::Kind::Bool => ValueKind::Boolean,
        schema::Kind::Integer { .. } => ValueKind::Integer,
        schema::Kind::Float { .. } => ValueKind::Float,
        schema::Kind::Text => ValueKind::Text,
        schema::Kind::List => ValueKind::List,
        schema::Kind::Choice(_) => ValueKind::Choice,
        schema::Kind::Color => ValueKind::Color,
        schema::Kind::Curve => ValueKind::Curve,
    }
}

/// Slider bounds for number kinds that carry both a min and a max;
/// (0.0, 0.0) means un-ranged; no slider.
fn kind_bounds(kind: &schema::Kind) -> (f32, f32) {
    match kind {
        schema::Kind::Integer {
            min: Some(min),
            max: Some(max),
        } => (*min as f32, *max as f32),
        schema::Kind::Float {
            min: Some(min),
            max: Some(max),
        } => (*min as f32, *max as f32),
        _ => (0.0, 0.0),
    }
}

/// Text for a slider float, rounded for whole-number kinds so the write
/// path can parse it back as i64.
fn slider_text(schema: &[schema::Entry], key: &str, value: f32) -> String {
    let is_int = schema.iter().any(|entry| {
        entry.path.join(".") == key && matches!(entry.kind, schema::Kind::Integer { .. })
    });
    if is_int {
        format!("{}", value.round() as i64)
    } else {
        format!("{value}")
    }
}

/// Show a dragged slider's live value in the row's text box without
/// touching the document. The unchanged-text guard keeps the Slider's
/// `changed` callback from looping back through set_row_data.
fn preview_row(app: &AppWindow, key: &str, value_text: &str) {
    let groups = app.get_file_groups();
    let Some(groups) = groups.as_any().downcast_ref::<VecModel<FileGroup>>() else {
        return;
    };
    for gi in 0..groups.row_count() {
        let Some(group) = groups.row_data(gi) else {
            continue;
        };
        let Some(rows) = group.rows.as_any().downcast_ref::<VecModel<FileRow>>() else {
            continue;
        };
        for ri in 0..rows.row_count() {
            let Some(mut old) = rows.row_data(ri) else {
                continue;
            };
            if old.key.as_str() == key {
                if old.value.as_str() != value_text {
                    old.value = value_text.into();
                    rows.set_row_data(ri, old);
                }
                return;
            }
        }
    }
}

/// Commit form: turn editor input into the value text `set_leaf_text`
/// expects, validating/clamping per Kind. `Err` = user-facing rejection.
fn commit_value(kind: Option<&schema::Kind>, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    match kind {
        Some(schema::Kind::List) => {
            let inner = raw.trim().trim_start_matches('[').trim_end_matches(']');
            Ok(format!("[{inner}]"))
        }
        Some(schema::Kind::Bool) => Ok(raw.to_owned()),
        Some(schema::Kind::Integer { min, max }) => {
            let mut value: i64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a whole number"))?;
            if let Some(min) = min {
                value = value.max(*min);
            }
            if let Some(max) = max {
                value = value.min(*max);
            }
            Ok(value.to_string())
        }
        Some(schema::Kind::Float { min, max }) => {
            let mut value: f64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a number"))?;
            if let Some(min) = min {
                value = value.max(*min);
            }
            if let Some(max) = max {
                value = value.min(*max);
            }
            Ok(value.to_string())
        }
        // Strings in the file are quoted; Debug-format escapes the same way
        // TOML basic strings do for the ASCII values configs use.
        Some(
            schema::Kind::Text
            | schema::Kind::Choice(_)
            | schema::Kind::Color
            | schema::Kind::Curve,
        ) => Ok(format!("{raw:?}")),
        _ => Ok(raw.to_owned()),
    }
}

/// Hex color (#RRGGBB or #RRGGBBAA) to swatch brush; black when unparseable.
fn swatch_for(value: &str) -> slint::Brush {
    let hex = value.trim().trim_start_matches('#');
    let channel = |range: std::ops::Range<usize>| {
        hex.get(range)
            .and_then(|bits| u8::from_str_radix(bits, 16).ok())
            .unwrap_or(0)
    };
    slint::Brush::from(slint::Color::from_rgb_u8(
        channel(0..2),
        channel(2..4),
        channel(4..6),
    ))
}

/// Clean, editor-ready text for a schema key, read through the typed
/// getters — never the TOML repr, which carries trailing comments.
fn typed_value(doc: &ConfigDocument, entry: &schema::Entry) -> Option<String> {
    let parts: Vec<&str> = entry.path.iter().map(String::as_str).collect();
    match &entry.kind {
        schema::Kind::Bool => doc.get_bool(&parts).map(|v| v.to_string()),
        schema::Kind::Integer { .. } => doc.get_integer(&parts).map(|v| v.to_string()),
        schema::Kind::Float { .. } => doc.get_float(&parts).map(|v| v.to_string()),
        schema::Kind::List => list_text(doc, &parts),
        schema::Kind::Text
        | schema::Kind::Choice(_)
        | schema::Kind::Color
        | schema::Kind::Curve => doc.get_string(&parts),
    }
}

/// Comma-joined list display, mirroring egui's array_text.
fn list_text(doc: &ConfigDocument, parts: &[&str]) -> Option<String> {
    if let Some(values) = doc.get_integers(parts) {
        return Some(
            values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(values) = doc.get_floats(parts) {
        return Some(
            values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    doc.get_strings(parts).map(|values| values.join(", "))
}

/// Mined metadata shown beside the editor: range from the Kind payload,
/// then the mined unit. Empty when the key carries neither.
fn entry_hint(entry: &schema::Entry) -> String {
    let range = match &entry.kind {
        schema::Kind::Integer { min, max } => Some(range_text(
            min.map(|v| v.to_string()),
            max.map(|v| v.to_string()),
        )),
        schema::Kind::Float { min, max } => Some(range_text(
            min.map(|v| v.to_string()),
            max.map(|v| v.to_string()),
        )),
        _ => None,
    };
    let mut hints: Vec<String> = range.into_iter().filter(|r| !r.is_empty()).collect();
    if let Some(unit) = &entry.unit {
        hints.push(unit.clone());
    }
    hints.join(" • ")
}

fn range_text(min: Option<String>, max: Option<String>) -> String {
    match (min, max) {
        (Some(min), Some(max)) => format!("{min}–{max}"),
        (Some(min), None) => format!("≥ {min}"),
        (None, Some(max)) => format!("≤ {max}"),
        (None, None) => String::new(),
    }
}

/// Best-effort decor strip for non-schema values (repr may carry comments).
fn strip_decor(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.split_once(" #") {
        Some((value, _)) => value.trim_end().to_owned(),
        None => trimmed.to_owned(),
    }
}
