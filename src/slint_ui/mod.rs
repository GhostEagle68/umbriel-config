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
    discovery, document::ConfigDocument, includes, outputs, schema, settings as app_settings,
    state, validate,
};
use umbriel_config::{live, update};

mod catalog;

slint::include_modules!();

/// Minimal starter written by the onboarding panel: comments only, so it
/// is valid TOML and loads as an empty healthy document.
const STARTER_CONFIG: &str = "\
# umbriel configuration — created by Umbriel Config.
# Reference: https://github.com/noctalia-dev/umbriel
";

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
    // Keys added by an umbriel update or the last sync: sidebar counts and
    // NEW badges.
    new_keys: BTreeSet<String>,
    // Guided-setup walk (create flow): the sections left to visit.
    guide: Option<Guide>,
    // Monitors detected when the guide was armed: feeds the outputs
    // card's resolution/refresh dropdowns and its recommendation hints.
    guide_monitors: Vec<live::LiveOutput>,
}

/// One guided-setup walk: the curated cards, in visit order. Each step is
/// its title plus the curated key definitions it shows.
struct Guide {
    steps: Vec<(String, Vec<&'static GuideKey>)>,
    index: usize,
}

/// The guided-setup steps: a hand-picked handful of the most-wanted
/// settings per card (outputs is prepended when monitors are detected;
/// keybinds joins when its editor exists). Each key carries a plain
/// label and a one-line description — the guide speaks human, not
/// config-file. Keys missing from the installed schema are skipped.
struct GuideKey {
    key: &'static str,
    label: &'static str,
    description: &'static str,
}

const GUIDE_STEPS: &[(&str, &[GuideKey])] = &[
    (
        "layout",
        &[
            GuideKey {
                key: "layout.mode",
                label: "Layout mode",
                description: "How windows are arranged, scrolling columns or the classic dwindle / master trees.",
            },
            GuideKey {
                key: "layout.gap",
                label: "Gap",
                description: "Space between windows",
            },
            GuideKey {
                key: "layout.scrolling.default_width_fraction",
                label: "New column width",
                description: "How much of the screen a new column starts with.",
            },
        ],
    ),
    (
        "input",
        &[
            GuideKey {
                key: "input.keyboard.layout",
                label: "Keyboard layout",
                description: "Country layout for your keyboard, e.g. us or de,us.",
            },
            GuideKey {
                key: "input.keyboard.numlock_toggle",
                label: "Num Lock on connect",
                description: "Turn Num Lock on whenever a keyboard connects.",
            },
            GuideKey {
                key: "input.middle_click_paste",
                label: "Middle-click paste",
                description: "Paste the selected text with a middle click.",
            },
            GuideKey {
                key: "input.touchpad.tap",
                label: "Tap to click",
                description: "Tapping the touchpad counts as a click.",
            },
            GuideKey {
                key: "input.cursor.size",
                label: "Cursor size",
                description: "Mouse cursor size.",
            },
        ],
    ),
    (
        "general",
        &[
            GuideKey {
                key: "general.xwayland",
                label: "XWayland",
                description: "Run X11 apps through xwayland-satellite. Changing this needs a fresh Umbriel session.",
            },
            GuideKey {
                key: "general.show_cheatsheet",
                label: "Cheat sheet at start",
                description: "Show the keybind overlay when the session starts.",
            },
            GuideKey {
                key: "general.focus_on_activate",
                label: "Let apps take focus",
                description: "Allow newly opened apps to demand focus.",
            },
            GuideKey {
                key: "general.autostart",
                label: "Autostart commands",
                description: "Commands run once when the session starts.",
            },
        ],
    ),
    (
        "appearance",
        &[
            GuideKey {
                key: "appearance.border_width",
                label: "Border width",
                description: "Thickness of the window border, in logical pixels.",
            },
            GuideKey {
                key: "appearance.corner_radius",
                label: "Corner radius",
                description: "How rounded the window corners are.",
            },
            GuideKey {
                key: "appearance.blur.radius",
                label: "Blur radius",
                description: "How far the blur spreads behind windows.",
            },
            GuideKey {
                key: "appearance.blur.passes",
                label: "Blur passes",
                description: "How many times the blur is applied, more is smoother.",
            },
            GuideKey {
                key: "colors.border.focused",
                label: "Focused border color",
                description: "Border color of the focused window, as hex (e.g. #7AA3FFFF).",
            },
        ],
    ),
];

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
        // Startup drift: keys added since the last snapshot get NEW badges.
        // No snapshot yet (first run) flags nothing — egui's note guard,
        // applied to the badge set too.
        let seen = state::load(&state::snapshot_path(env));
        let new_keys = if seen.is_empty() {
            BTreeSet::new()
        } else {
            schema::diff(&seen, &schema::key_set(&schema))
                .added
                .into_iter()
                .collect()
        };
        let _ = state::store(&state::snapshot_path(env), &schema::key_set(&schema));
        let includes = includes::load_chain(&doc, path);
        let mut shell = Shell {
            path: path.to_path_buf(),
            doc,
            healthy,
            load_error,
            schema,
            includes,
            saved: Vec::new(),
            new_keys,
            guide: None,
            guide_monitors: Vec::new(),
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

    // First-run state (plan-onboarding.md): a machine without a config
    // gets the panel — with the guided walk when umbriel is present.
    // Umbriel missing additionally arms the quiet banner + empty state.
    let umbriel_present = discovery::packaged_default(&env).is_some();
    let mode = setup_mode(umbriel_present, path.exists());
    app.set_show_onboarding(matches!(
        mode,
        SetupMode::PlainInstall | SetupMode::FreshWithUmbriel
    ));
    app.set_umbriel_missing(!umbriel_present);
    app.set_schema_empty(shell.borrow().schema.is_empty());

    let nav = section_nav(&shell.borrow());
    let configured_outputs = !outputs::configured(&shell.borrow().doc).is_empty();
    let landing = nav
        .iter()
        .find(|entry| !entry.is_header && (configured_outputs || entry.id != catalog::OUTPUTS_ID));
    if let Some(first) = landing {
        let (title, description) = page_meta(&first.id);
        app.set_current_section(first.id.clone());
        app.set_page_title(title.into());
        app.set_page_description(description.into());
        refill_page(&app, &shell.borrow(), &first.id);
    }
    app.set_sections(Rc::new(VecModel::from(nav)).into());
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
            // Outputs page: rescan on open — fast-fails when no
            // compositor is reachable and keeps the last detection.
            if name.as_str() == catalog::OUTPUTS_ID {
                shell.borrow_mut().guide_monitors = live::outputs().unwrap_or_default();
            }
            let shell = shell.borrow();
            app.set_current_section(name.clone());
            let (title, description) = page_meta(&name);
            app.set_page_title(title.into());
            app.set_page_description(description.into());
            app.set_page(Page::Section);
            refill_page(&app, &shell, &name);
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
                let (title, description) = page_meta(&section);
                app.set_page_title(title.into());
                app.set_page_description(description.into());
                refill_page(&app, &shell, &section);
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
                        home_section: page_id_for_section(&entry.section).into(),
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
            app.set_file_stats(status_line(&shell));
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
            refill_page(&app, &shell, app.get_current_section().as_str());
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
            refill_page(&app, &shell, &section);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let env = env.clone();
        app.on_sync_schema_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            // egui semantics: re-read the packaged default, diff old vs
            // fresh, store the fresh snapshot; the added keys get badges.
            let fresh = discovery::packaged_default(&env)
                .and_then(|path| std::fs::read_to_string(path).ok())
                .map(|text| schema::assemble(&text))
                .unwrap_or_default();
            let fresh_set = schema::key_set(&fresh);
            let mut shell = shell.borrow_mut();
            let drift = schema::diff(&schema::key_set(&shell.schema), &fresh_set);
            let _ = state::store(&state::snapshot_path(&env), &fresh_set);
            shell.new_keys = drift.added.iter().cloned().collect();
            let empty = fresh.is_empty();
            let status = if empty {
                SharedString::from("No packaged default found; install umbriel and sync again.")
            } else if drift.is_empty() {
                SharedString::from("Schema is up to date.")
            } else {
                SharedString::from(format!("Synced from umbriel: {}.", drift.summary()))
            };
            shell.schema = fresh;
            app.set_status(status);
            app.set_schema_empty(empty);
            app.set_sections(Rc::new(VecModel::from(section_nav(&shell))).into());
            let section = app.get_current_section().to_string();
            refill_page(&app, &shell, &section);
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
        let shell = Rc::clone(&shell);
        app.on_onboarding_create(move || {
            let Some(app) = weak.upgrade() else { return };
            // The panel only shows while the file is absent; never clobber
            // a config that appeared meanwhile.
            let created = {
                let shell = shell.borrow_mut();
                if shell.path.exists() {
                    app.set_show_onboarding(false);
                    return;
                }
                if let Some(parent) = shell.path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&shell.path, STARTER_CONFIG) {
                    Ok(()) => {
                        app.set_show_onboarding(false);
                        app.set_status("Created config.toml.".into());
                        true
                    }
                    Err(err) => {
                        app.set_status(format!("Couldn't create config.toml: {err}").into());
                        false
                    }
                }
            };
            if !created {
                return;
            }
            // With a schema source, walk the frequent sections with the
            // suggested defaults filled in; without one there is nothing
            // to guide through (the empty state explains).
            if !start_guide(&mut shell.borrow_mut()) {
                return;
            }
            guide_show_step(&app, &shell.borrow());
            app.set_show_guide(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_guide_back(move || {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut shell = shell.borrow_mut();
                let Some(guide) = shell.guide.as_mut() else {
                    return;
                };
                if guide.index == 0 {
                    return;
                }
                guide.index -= 1;
            }
            guide_show_step(&app, &shell.borrow());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_guide_relaunch(move || {
            let Some(app) = weak.upgrade() else { return };
            if app.get_dirty() {
                app.set_status(
                    "Save or discard your changes before running the guided setup.".into(),
                );
                return;
            }
            if !start_guide(&mut shell.borrow_mut()) {
                app.set_status(
                    "Nothing to guide through yet — install umbriel and sync the schema.".into(),
                );
                return;
            }
            guide_show_step(&app, &shell.borrow());
            app.set_show_guide(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_guide_next(move || {
            let Some(app) = weak.upgrade() else { return };
            // Advance one step, or — on the last — write the config and
            // land in the app. Borrowed in an inner scope so the step
            // data outlives the &mut.
            let advance = {
                let mut shell = shell.borrow_mut();
                let Some(guide) = shell.guide.as_mut() else {
                    return;
                };
                guide.index += 1;
                guide.index < guide.steps.len()
            };
            if advance {
                guide_show_step(&app, &shell.borrow());
                return;
            }
            // Finish: write every modified doc, then validate through
            // umbriel (absent installs skip validation gracefully).
            let mut shell = shell.borrow_mut();
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
            shell.guide = None;
            app.set_show_guide(false);
            app.set_dirty(false);
            app.set_changed_count(0);
            match report {
                Ok(report) if report.is_ok() => {
                    app.set_validate_note(String::new().into());
                    app.set_status("Config saved, Umbriel picks it up automatically.".into());
                }
                Ok(report) => {
                    let messages: Vec<String> = report
                        .diagnostics
                        .iter()
                        .map(|d| d.message().to_owned())
                        .collect();
                    app.set_validate_note(messages.join("; ").into());
                    app.set_status(
                        format!("Config saved ({saved_files} file(s)); umbriel reported problems.")
                            .into(),
                    );
                }
                Err(err) => {
                    app.set_validate_note(err.to_string().into());
                    app.set_status(format!("Config saved ({saved_files} file(s)).").into());
                }
            }
        });
    }
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
                if key.starts_with("output.") {
                    // Output fields aren't schema-backed; their fixed
                    // vocabulary formats the input instead.
                    format_output_value(&shell, key, raw)
                } else {
                    let kind = shell
                        .schema
                        .iter()
                        .find(|entry| entry.path.join(".") == key)
                        .map(|entry| &entry.kind);
                    commit_value(kind, raw)
                }
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
            // A new resolution changes which refreshes exist — rebuild
            // that dropdown too, even if its text stayed the same.
            if key.starts_with("output.") && key.ends_with(".resolution") {
                let name = &key["output.".len()..key.len() - ".resolution".len()];
                rebuild_row(app, &shell, &format!("output.{name}.refresh"), true);
            }
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
fn refill_page(app: &AppWindow, shell: &Shell, page_id: &str) {
    app.set_file_groups(Rc::new(VecModel::from(page_groups(shell, page_id))).into());
    app.set_changed_count(changed_count(shell));
}

/// Which page a schema sub-section belongs to: its claimed page, else the
/// first page covering its top-level area, else the MORE fallback page
/// (whose id is the top-level name itself).
fn page_id_for_section(section: &str) -> String {
    if let Some((_, id)) = catalog::claimed_sections().find(|(claimed, _)| *claimed == section) {
        return (*id).to_owned();
    }
    let top = section.split('.').next().unwrap_or(section);
    if let Some((claimed, id)) =
        catalog::claimed_sections().find(|(claimed, _)| claimed.split('.').next() == Some(top))
    {
        let _ = claimed;
        return (*id).to_owned();
    }
    top.to_owned()
}

/// NEW-key counts per page id (sidebar badges).
fn page_new_counts(shell: &Shell) -> BTreeMap<String, i32> {
    let mut counts: BTreeMap<String, i32> = BTreeMap::new();
    for key in &shell.new_keys {
        let page_id = if key.starts_with("output.") {
            catalog::OUTPUTS_ID.to_owned()
        } else {
            match shell
                .schema
                .iter()
                .find(|entry| entry.dotted() == *key)
                .map(|entry| page_id_for_section(&entry.section))
            {
                Some(page_id) => page_id,
                None => continue,
            }
        };
        *counts.entry(page_id).or_default() += 1;
    }
    counts
}

/// "hot_corners" → "Hot corners".
fn prettify(name: &str) -> String {
    let mut owned = name.replace('_', " ");
    if let Some(first) = owned.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    owned
}

/// Header title + subtitle for a page.
fn page_meta(page_id: &str) -> (String, String) {
    match catalog::page(page_id) {
        Some(page) => (page.title.to_owned(), page.description.to_owned()),
        None => (
            prettify(page_id),
            "Settings umbriel hasn't grouped yet, the catalog will catch up.".to_owned(),
        ),
    }
}

/// The sidebar model: grouped human pages (catalog + MORE fallback) with
/// small header rows and NEW counts per page.
fn section_nav(shell: &Shell) -> Vec<SectionNav> {
    fn push_page(
        nav: &mut Vec<SectionNav>,
        claimed_tops: &mut Vec<&'static str>,
        counts: &BTreeMap<String, i32>,
        page: &'static catalog::Page,
    ) {
        for top in catalog::page_top_levels(page) {
            if !claimed_tops.contains(&top) {
                claimed_tops.push(top);
            }
        }
        nav.push(SectionNav {
            label: page.title.into(),
            id: page.id.into(),
            new_count: counts.get(page.id).copied().unwrap_or(0),
            is_header: false,
        });
    }

    let counts = page_new_counts(shell);
    let mut nav: Vec<SectionNav> = Vec::new();
    let mut claimed_tops: Vec<&'static str> = Vec::new();
    // Outputs leads the sidebar and needs no redundant group header.
    if let Some(page) = catalog::page(catalog::OUTPUTS_ID) {
        push_page(&mut nav, &mut claimed_tops, &counts, page);
    }
    for group in catalog::GROUPS.iter().filter(|group| **group != "outputs") {
        nav.push(SectionNav {
            label: catalog::group_title(group).into(),
            id: String::new().into(),
            new_count: 0,
            is_header: true,
        });
        for page in catalog::PAGES.iter().filter(|page| page.group == *group) {
            push_page(&mut nav, &mut claimed_tops, &counts, page);
        }
    }
    // Top-level areas the catalog doesn't claim surface under MORE.
    let more: Vec<SharedString> = section_names(&shell.schema)
        .into_iter()
        .filter(|name| !claimed_tops.contains(&name.as_str()))
        .collect();
    if !more.is_empty() {
        nav.push(SectionNav {
            label: catalog::group_title(catalog::MORE_GROUP).into(),
            id: String::new().into(),
            new_count: 0,
            is_header: true,
        });
        for name in more {
            nav.push(SectionNav {
                new_count: counts.get(name.as_str()).copied().unwrap_or(0),
                label: prettify(&name).into(),
                id: name.to_string().into(),
                is_header: false,
            });
        }
    }
    nav
}

/// One settings page's cards.
fn page_groups(shell: &Shell, page_id: &str) -> Vec<FileGroup> {
    if page_id == catalog::OUTPUTS_ID {
        return output_groups(shell);
    }
    if let Some(page) = catalog::page(page_id) {
        return catalog_page_groups(shell, page);
    }
    fallback_page_groups(shell, page_id)
}

/// A catalog page: one card per curated sub-section, then any
/// sub-sections of the same areas the catalog doesn't claim yet, then
/// the uncovered-keys card.
fn catalog_page_groups(shell: &Shell, page: &catalog::Page) -> Vec<FileGroup> {
    let sets = chain_path_sets(shell);
    let labels = file_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();

    let mut groups: Vec<FileGroup> = Vec::new();
    let mut claimed: Vec<&str> = Vec::new();
    for card in page.cards {
        claimed.push(card.section);
        let rows: Vec<FileRow> = shell
            .schema
            .iter()
            .filter(|entry| entry.section == card.section)
            .map(|entry| schema_row(shell, &sets, &labels, &current, entry))
            .collect();
        if !rows.is_empty() {
            groups.push(FileGroup {
                title: card.title.into(),
                rows: Rc::new(VecModel::from(rows)).into(),
            });
        }
    }

    let tops = catalog::page_top_levels(page);
    let mut auto: BTreeMap<String, Vec<FileRow>> = BTreeMap::new();
    for entry in &shell.schema {
        let top = entry.section.split('.').next().unwrap_or("");
        if !tops.contains(&top) || claimed.contains(&entry.section.as_str()) {
            continue;
        }
        let row = schema_row(shell, &sets, &labels, &current, entry);
        auto.entry(prettify(&entry.section)).or_default().push(row);
    }
    for (title, rows) in auto {
        groups.push(FileGroup {
            title: title.into(),
            rows: Rc::new(VecModel::from(rows)).into(),
        });
    }
    if let Some(other) = other_group(shell, &sets, &labels, &current, &tops) {
        groups.push(other);
    }
    groups
}

/// A MORE fallback page: every sub-section of one top-level area.
fn fallback_page_groups(shell: &Shell, top: &str) -> Vec<FileGroup> {
    let sets = chain_path_sets(shell);
    let labels = file_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();

    let mut groups: BTreeMap<String, Vec<FileRow>> = BTreeMap::new();
    for entry in &shell.schema {
        if entry.section.split('.').next() != Some(top) {
            continue;
        }
        let row = schema_row(shell, &sets, &labels, &current, entry);
        groups
            .entry(prettify(&entry.section))
            .or_default()
            .push(row);
    }
    let mut out: Vec<FileGroup> = groups
        .into_iter()
        .map(|(title, rows)| FileGroup {
            title: title.into(),
            rows: Rc::new(VecModel::from(rows)).into(),
        })
        .collect();
    if let Some(other) = other_group(shell, &sets, &labels, &current, &[top]) {
        out.push(other);
    }
    out
}

/// Keys beyond the schema fold onto the page that owns their area (the
/// old Other-settings sweep): one read-only row per key, owned like any
/// other row. Only keys whose top-level matches one of `tops` are shown.
fn other_group(
    shell: &Shell,
    sets: &[BTreeSet<String>],
    labels: &[SharedString],
    current: &[BTreeMap<String, String>],
    tops: &[&str],
) -> Option<FileGroup> {
    let docs: Vec<&ConfigDocument> = shell.includes.docs.iter().map(|inc| &inc.doc).collect();
    let claims = schema::managed_claims(&docs);
    let schema_keys = schema::key_set(&shell.schema);
    let mut other: BTreeMap<String, FileRow> = BTreeMap::new();
    for (i, set) in sets.iter().enumerate() {
        let paths: Vec<String> = set.iter().cloned().collect();
        for path in schema::uncovered(&paths, &schema_keys, &claims) {
            let top = path.split('.').next().unwrap_or_default();
            if !tops.contains(&top) {
                continue;
            }
            // One row per key: the file that owns it (shadowed copies
            // in later includes are skipped).
            let Some(home) = entry_home(sets, &path) else {
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
                    is_new: false,
                },
            );
        }
    }
    if other.is_empty() {
        return None;
    }
    Some(FileGroup {
        title: "other".into(),
        rows: Rc::new(VecModel::from(other.into_values().collect::<Vec<_>>())).into(),
    })
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
        is_new: false,
    }
}

fn refresh_row(app: &AppWindow, shell: &Shell, key: &str) {
    rebuild_row(app, shell, key, false);
}

/// Rebuild a row and overwrite it even when the text is unchanged — for
/// when a sibling edit changes this row's options (resolution → refresh).
fn rebuild_row(app: &AppWindow, shell: &Shell, key: &str, force: bool) {
    let sets = chain_path_sets(shell);
    let labels = file_labels(shell);
    let row = if let Some(entry) = shell
        .schema
        .iter()
        .find(|entry| entry.path.join(".") == key)
    {
        let home = entry_home(&sets, key).unwrap_or(shell.includes.docs.len());
        let current: BTreeMap<String, String> =
            doc_at(shell, home).leaf_values().into_iter().collect();
        let mut row = file_row(doc_at(shell, home), entry);
        row.home = home as i32;
        if let Some(label) = labels.get(home) {
            row.home_label = label.clone();
        }
        row.changed = current.get(key) != shell.saved.get(home).and_then(|values| values.get(key));
        row
    } else if let Some(rest) = key.strip_prefix("output.") {
        let Some((name, field_key)) = rest.rsplit_once('.') else {
            return;
        };
        let monitor = shell
            .guide_monitors
            .iter()
            .find(|monitor| monitor.name == name);
        let current: BTreeMap<String, String> = doc_at(shell, shell.includes.docs.len())
            .leaf_values()
            .into_iter()
            .collect();
        let mut row = match field_key {
            "resolution" => {
                let Some(monitor) = monitor else { return };
                resolution_choice_row(shell, name, monitor, &current)
            }
            "refresh" => {
                let Some(monitor) = monitor else { return };
                refresh_choice_row(shell, name, monitor, &current)
            }
            _ => {
                let Some(field) = outputs::FIELDS.iter().find(|field| field.key == field_key)
                else {
                    return;
                };
                output_row(shell, name, field, &current)
            }
        };
        row.home_label = labels
            .get(shell.includes.docs.len())
            .cloned()
            .unwrap_or_default();
        row
    } else {
        return;
    };

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
                // Same text = nothing to re-render; keeps the editor's
                // focus. A forced rebuild also swaps changed options.
                if force || old.value != row.value {
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

/// Card heading for one guided-setup step.
fn guide_title(section: &str, step: usize, total: usize) -> String {
    let display = if section == "output" {
        "outputs".to_owned()
    } else {
        let mut owned = section.to_owned();
        if let Some(first) = owned.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        owned
    };
    format!("Guided setup — {display} (step {step} of {total})")
}

/// Arm the guided walk: the schema sections get suggested defaults, and
/// any detected monitors become an outputs step with their live state
/// (enabled, mode, scale, position) plus umbriel's defaults filled in.
/// `false` when there is no schema at all (no umbriel detected).
fn start_guide(shell: &mut Shell) -> bool {
    // Pre-existing values are never clobbered: suggested defaults and
    // detected state only fill settings that are still unset. That makes
    // the walk safe to re-run from Settings.
    let main = shell.includes.docs.len();
    let current: BTreeMap<String, String> = doc_at(shell, main).leaf_values().into_iter().collect();
    let mut steps: Vec<(String, Vec<&'static GuideKey>)> = Vec::new();
    for (title, metas) in GUIDE_STEPS {
        let mut keys: Vec<&'static GuideKey> = Vec::new();
        for meta in metas.iter() {
            let Some(entry) = shell.schema.iter().find(|entry| entry.dotted() == meta.key) else {
                continue; // not in this umbriel's schema
            };
            keys.push(meta);
            if current.contains_key(meta.key) {
                continue;
            }
            let Some(default) = entry.default.as_ref() else {
                continue;
            };
            let default_text = match default {
                schema::Value::Bool(v) => format!("{v}"),
                schema::Value::Integer(v) => format!("{v}"),
                schema::Value::Float(v) => format!("{v}"),
                schema::Value::Text(v) => v.clone(),
            };
            let Ok(value_text) = commit_value(Some(&entry.kind), &default_text) else {
                continue;
            };
            shell.doc.set_leaf_text(meta.key, &value_text);
        }
        if !keys.is_empty() {
            steps.push(((*title).to_owned(), keys));
        }
    }
    // Everything the app can detect is filled in the same way: each
    // connected monitor joins the walk as an outputs card — first, since
    // output setup is the most important decision.
    let monitors = live::outputs().unwrap_or_default();
    for monitor in &monitors {
        let name = monitor.name.as_str();
        let is_set = |field: &str| current.contains_key(&format!("output.{name}.{field}"));
        if !is_set("enabled") {
            shell
                .doc
                .set_bool(&["output", name, "enabled"], monitor.enabled);
        }
        if !is_set("mode")
            && let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index))
        {
            shell
                .doc
                .set_string(&["output", name, "mode"], &mode.label());
        }
        if !is_set("scale") {
            shell
                .doc
                .set_float(&["output", name, "scale"], monitor.scale);
        }
        if !is_set("position") {
            shell.doc.set_integers(
                &["output", name, "position"],
                &[i64::from(monitor.position.0), i64::from(monitor.position.1)],
            );
        }
        for field in outputs::FIELDS {
            if matches!(field.key, "enabled" | "mode" | "scale" | "position") {
                continue; // detection fills these
            }
            if is_set(field.key) {
                continue;
            }
            let path = ["output", name, field.key];
            match &field.default {
                Some(outputs::DefaultValue::Bool(value)) => {
                    shell.doc.set_bool(&path, *value);
                }
                Some(outputs::DefaultValue::Float(value)) => {
                    shell.doc.set_float(&path, *value);
                }
                Some(outputs::DefaultValue::Text(value)) => {
                    shell.doc.set_string(&path, value);
                }
                None => {}
            }
        }
    }
    if !monitors.is_empty() {
        steps.insert(0, ("output".to_owned(), Vec::new()));
    }
    if steps.is_empty() {
        return false;
    }
    // The filled-in values become the saved baseline: a row only shows
    // its dot once the user moves away from the suggestion.
    shell.reset_saved();
    shell.guide_monitors = monitors;
    shell.guide = Some(Guide { steps, index: 0 });
    true
}

/// Render the guide's current step into the file-groups model and update
/// the card heading/controls.
fn guide_show_step(app: &AppWindow, shell: &Shell) {
    let Some(guide) = shell.guide.as_ref() else {
        return;
    };
    let (title, metas) = &guide.steps[guide.index];
    if title == "output" {
        app.set_file_groups(Rc::new(VecModel::from(output_groups(shell))).into());
    } else {
        app.set_file_groups(Rc::new(VecModel::from(guide_groups(shell, metas))).into());
    }
    app.set_guide_title(guide_title(title, guide.index + 1, guide.steps.len()).into());
    app.set_guide_first(guide.index == 0);
    app.set_guide_last(guide.index == guide.steps.len() - 1);
}

/// One guided-setup card: the step's curated keys in order, resolved to
/// schema rows (keys this umbriel's schema lacks are skipped). The
/// curated label and description replace the mined ones — the guide
/// speaks human, not config-file.
fn guide_groups(shell: &Shell, metas: &[&'static GuideKey]) -> Vec<FileGroup> {
    let sets = chain_path_sets(shell);
    let labels = file_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();
    let rows: Vec<FileRow> = metas
        .iter()
        .filter_map(|meta| shell.schema.iter().find(|entry| entry.dotted() == meta.key))
        .map(|entry| {
            let mut row = schema_row(shell, &sets, &labels, &current, entry);
            if let Some(meta) = metas.iter().find(|meta| meta.key == entry.dotted()) {
                row.label = meta.label.into();
                row.hint = meta.description.into();
            }
            row
        })
        .collect();
    vec![FileGroup {
        title: String::new().into(),
        rows: Rc::new(VecModel::from(rows)).into(),
    }]
}

/// One schema row resolved against the chain: owner badge, changed dot,
/// NEW badge, availability.
fn schema_row(
    shell: &Shell,
    sets: &[BTreeSet<String>],
    labels: &[SharedString],
    current: &[BTreeMap<String, String>],
    entry: &schema::Entry,
) -> FileRow {
    let main = shell.includes.docs.len();
    let dotted = entry.dotted();
    let home = entry_home(sets, &dotted);
    let mut row = file_row(doc_at(shell, home.unwrap_or(main)), entry);
    row.is_new = shell.new_keys.contains(dotted.as_str());
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
    row
}

/// The outputs guide step: one card per configured monitor, its fields
/// editable through the standard row editors.
fn output_groups(shell: &Shell) -> Vec<FileGroup> {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let current: BTreeMap<String, String> = doc.leaf_values().into_iter().collect();
    let home_label = file_labels(shell).get(main).cloned().unwrap_or_default();
    // Configured monitors plus anything detected but not configured yet.
    let mut names: Vec<String> = outputs::configured(doc);
    for monitor in &shell.guide_monitors {
        if !names.contains(&monitor.name) {
            names.push(monitor.name.clone());
        }
    }
    names
        .iter()
        .map(|name| {
            let monitor = shell
                .guide_monitors
                .iter()
                .find(|monitor| monitor.name == *name);
            let mut rows: Vec<FileRow> = Vec::new();
            for field in outputs::FIELDS {
                if field.key == "mode" {
                    // With detected modes the single mode string splits
                    // into two welcoming dropdowns instead.
                    if let Some(monitor) = monitor.filter(|monitor| !monitor.modes.is_empty()) {
                        rows.push(resolution_choice_row(shell, name, monitor, &current));
                        rows.push(refresh_choice_row(shell, name, monitor, &current));
                        continue;
                    }
                }
                rows.push(output_row(shell, name, field, &current));
            }
            for row in &mut rows {
                row.home_label = home_label.clone();
            }
            FileGroup {
                title: name.clone().into(),
                rows: Rc::new(VecModel::from(rows)).into(),
            }
        })
        .collect()
}

/// Distinct resolutions (width x height) a monitor reports, in report order.
fn resolutions(monitor: &live::LiveOutput) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for mode in &monitor.modes {
        let label = format!("{}x{}", mode.width, mode.height);
        if !out.contains(&label) {
            out.push(label);
        }
    }
    out
}

/// Refresh texts a monitor offers at one resolution, formatted like the
/// mode labels ("164.977").
fn refreshes(monitor: &live::LiveOutput, resolution: &str) -> Vec<String> {
    monitor
        .modes
        .iter()
        .filter(|mode| format!("{}x{}", mode.width, mode.height) == resolution)
        .map(|mode| (mode.refresh_mhz as f64 / 1000.0).to_string())
        .collect()
}

/// The mode string of one configured output, split into resolution and
/// refresh halves.
fn mode_parts(doc: &ConfigDocument, name: &str) -> (String, String) {
    match doc
        .get_string(&["output", name, "mode"])
        .unwrap_or_default()
        .split_once('@')
    {
        Some((resolution, refresh)) => (resolution.to_owned(), refresh.to_owned()),
        None => (String::new(), String::new()),
    }
}

fn resolution_choice_row(
    shell: &Shell,
    name: &str,
    monitor: &live::LiveOutput,
    current: &BTreeMap<String, String>,
) -> FileRow {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let (mut resolution, _) = mode_parts(doc, name);
    if resolution.is_empty()
        && let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index))
    {
        resolution = format!("{}x{}", mode.width, mode.height);
    }
    let key = format!("output.{name}.resolution");
    let mut row = blank_output_row(shell, key.clone(), "Resolution");
    row.kind = ValueKind::Choice;
    row.choices = Rc::new(VecModel::from(
        resolutions(monitor)
            .iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
    .into();
    row.value = resolution.into();
    row.hint = "Modes your monitor reports — the detected one is selected.".into();
    row.changed = current.get(&key) != shell.saved.get(main).and_then(|values| values.get(&key));
    row
}

fn refresh_choice_row(
    shell: &Shell,
    name: &str,
    monitor: &live::LiveOutput,
    current: &BTreeMap<String, String>,
) -> FileRow {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let (mut resolution, mut refresh) = mode_parts(doc, name);
    if resolution.is_empty()
        && let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index))
    {
        resolution = format!("{}x{}", mode.width, mode.height);
        refresh = (mode.refresh_mhz as f64 / 1000.0).to_string();
    }
    let key = format!("output.{name}.refresh");
    let mut row = blank_output_row(shell, key.clone(), "Refresh rate");
    row.kind = ValueKind::Choice;
    row.choices = Rc::new(VecModel::from(
        refreshes(monitor, &resolution)
            .iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
    .into();
    row.value = refresh.into();
    row.hint = "Refresh rates your monitor supports at this resolution, in Hz.".into();
    row.changed = current.get(&key) != shell.saved.get(main).and_then(|values| values.get(&key));
    row
}

/// Row scaffold shared by every output editor row: owned by the main
/// config, no metadata yet.
fn blank_output_row(shell: &Shell, key: String, label: &str) -> FileRow {
    let main = shell.includes.docs.len();
    FileRow {
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
        home: main as i32,
        home_label: String::new().into(),
        changed: false,
        available: false,
        is_new: false,
    }
}

/// Plain-language description of an output field, shown under the label.
fn output_field_hint(key: &str) -> &'static str {
    match key {
        "enabled" => "This monitor is active, untick to switch it off.",
        "mode" => "Resolution and refresh rate combined, e.g. 3440x1440@164.977.",
        "position" => "Where the monitor sits, as x, y. Filled from your current layout.",
        "scale" => "Zoom level; 1.0 is native size.",
        "vrr" => {
            "Variable refresh rate (FreeSync/G-Sync). Recommended: fullscreen while gaming, otherwise disabled."
        }
        "hdr" => "HDR when the content supports it. Recommended: auto on HDR monitors.",
        "sdr_white" => "Brightness of normal (SDR) content in HDR mode, in nits. Recommended: 203.",
        "transform" => "Rotate or flip the display. Recommended: normal.",
        "tearing" => "Allow tearing for lower input lag. Recommended: off.",
        "direct_scanout" => "Send fullscreen apps straight to the display. Recommended: on.",
        "workspaces" => "Workspace names for this monitor; dynamic keeps them automatic.",
        _ => "",
    }
}

/// One output field as a row: the fixed vocabulary mapped onto editors
/// (toggle → checkbox, choice → dropdown, float → slider, the rest text).
fn output_row(
    shell: &Shell,
    name: &str,
    field: &outputs::Field,
    current: &BTreeMap<String, String>,
) -> FileRow {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let path = ["output", name, field.key];
    let key = format!("output.{name}.{}", field.key);
    let mut row = blank_output_row(shell, key.clone(), field.label);
    row.hint = output_field_hint(field.key).into();
    let default_text = match &field.default {
        Some(outputs::DefaultValue::Bool(value)) => value.to_string(),
        Some(outputs::DefaultValue::Float(value)) => value.to_string(),
        Some(outputs::DefaultValue::Text(value)) => (*value).to_owned(),
        None => String::new(),
    };
    match &field.kind {
        outputs::FieldKind::Toggle => {
            let value = doc.get_bool(&path).unwrap_or(matches!(
                field.default,
                Some(outputs::DefaultValue::Bool(true))
            ));
            row.kind = ValueKind::Boolean;
            row.checked = value;
            row.value = value.to_string().into();
        }
        outputs::FieldKind::Choice(vocab) => {
            row.kind = ValueKind::Choice;
            row.choices = Rc::new(VecModel::from(
                vocab
                    .iter()
                    .map(|value| SharedString::from(*value))
                    .collect::<Vec<_>>(),
            ))
            .into();
            row.value = doc
                .get_string(&path)
                .filter(|value| !value.is_empty())
                .unwrap_or(default_text)
                .into();
        }
        outputs::FieldKind::Position => {
            row.kind = ValueKind::Text;
            row.value = match doc.get_integers(&path).unwrap_or_default().as_slice() {
                [x, y] => format!("{x}, {y}").into(),
                _ => String::new().into(),
            };
        }
        outputs::FieldKind::Float { min, max } => {
            row.kind = ValueKind::Float;
            row.min = *min as f32;
            row.max = *max as f32;
            row.value = doc
                .get_float(&path)
                .unwrap_or_else(|| default_text.parse().unwrap_or(0.0))
                .to_string()
                .into();
        }
        outputs::FieldKind::Text | outputs::FieldKind::Workspaces => {
            row.value = doc
                .get_string(&path)
                .filter(|value| !value.is_empty())
                .unwrap_or(default_text)
                .into();
        }
    }
    row.changed = current.get(&key) != shell.saved.get(main).and_then(|values| values.get(&key));
    row
}

/// Format raw editor input for an output field (not schema-backed) into
/// its TOML text form.
fn format_output_value(shell: &Shell, key: &str, raw: &str) -> Result<String, String> {
    let field_key = key.rsplit('.').next().unwrap_or_default();
    let name = &key["output.".len()..key.len() - field_key.len() - 1];
    // The detected mode string splits into two dropdowns; both compose
    // the full mode this output saves.
    match field_key {
        "resolution" => {
            let Some(monitor) = shell
                .guide_monitors
                .iter()
                .find(|monitor| monitor.name == name)
            else {
                return Err("no detected monitor for this output".to_owned());
            };
            let matching: Vec<&live::LiveMode> = monitor
                .modes
                .iter()
                .filter(|mode| format!("{}x{}", mode.width, mode.height) == raw)
                .collect();
            let Some(mode) = matching
                .iter()
                .find(|mode| mode.preferred)
                .or_else(|| matching.first())
            else {
                return Err(format!("'{raw}' is not a mode this monitor reports"));
            };
            let refresh = (mode.refresh_mhz as f64 / 1000.0).to_string();
            return Ok(format!("{raw}@{refresh}"));
        }
        "refresh" => {
            let doc = doc_at(shell, shell.includes.docs.len());
            let (resolution, _) = mode_parts(doc, name);
            if resolution.is_empty() {
                return Err("pick a resolution first".to_owned());
            }
            return Ok(format!("{resolution}@{raw}"));
        }
        _ => {}
    }
    let Some(field) = outputs::FIELDS.iter().find(|field| field.key == field_key) else {
        return Err(format!("unknown output field in {key}"));
    };
    let raw = raw.trim();
    match &field.kind {
        outputs::FieldKind::Toggle => match raw {
            "true" | "false" => Ok(raw.to_owned()),
            _ => Err(format!("'{raw}' is not true or false")),
        },
        outputs::FieldKind::Float { min, max } => {
            let value: f64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a number"))?;
            Ok(value.clamp(*min, *max).to_string())
        }
        outputs::FieldKind::Position => {
            let mut parts = raw.split(',');
            let mut xy = [0_i32, 0];
            for slot in &mut xy {
                *slot = parts
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .map_err(|_| format!("'{raw}' is not an x, y position"))?;
            }
            Ok(format!("[{}, {}]", xy[0], xy[1]))
        }
        outputs::FieldKind::Choice(_)
        | outputs::FieldKind::Text
        | outputs::FieldKind::Workspaces => commit_value(Some(&schema::Kind::Text), raw),
    }
}

/// The four startup states (plan-onboarding.md): only the plain install
/// gets the first-run panel; a configured machine without umbriel gets
/// the quiet banner.
#[derive(Debug, PartialEq, Eq)]
enum SetupMode {
    Normal,
    FreshWithUmbriel,
    PlainInstall,
    MissingUmbriel,
}

/// "Umbriel present" is the packaged default being installed; "config
/// exists" is the main config file being on disk.
fn setup_mode(umbriel_present: bool, config_exists: bool) -> SetupMode {
    match (umbriel_present, config_exists) {
        (true, true) => SetupMode::Normal,
        (true, false) => SetupMode::FreshWithUmbriel,
        (false, true) => SetupMode::MissingUmbriel,
        (false, false) => SetupMode::PlainInstall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_mode_covers_the_four_states() {
        assert_eq!(setup_mode(true, true), SetupMode::Normal);
        assert_eq!(setup_mode(true, false), SetupMode::FreshWithUmbriel);
        assert_eq!(setup_mode(false, true), SetupMode::MissingUmbriel);
        assert_eq!(setup_mode(false, false), SetupMode::PlainInstall);
    }

    #[test]
    fn starter_config_loads_as_empty_healthy_document() {
        let doc = ConfigDocument::from_str(STARTER_CONFIG).expect("starter parses");
        assert!(doc.leaf_values().is_empty());
        assert!(!doc.is_modified());
    }
}
