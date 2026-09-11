//! Slint shell for umbriel-config; the migration plan lives in
//! `.design/ui-plan.md`. Phase 0: window chrome, sidebar navigation, and a
//! read-only status projection of the loaded config. The UI-agnostic library
//! does all real work — this module only presents it and forwards intents.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use slint::{
    CloseRequestResponse, ComponentHandle, LogicalSize, Model, SharedString, VecModel, WindowSize,
};
use umbriel_config::config::{
    backups, discovery, document::ConfigDocument, includes, keybinds, outputs, rules, schema,
    settings as app_settings, state, validate,
};
use umbriel_config::{changelog, live, update};

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
    // Last live-scan failure note; empty when detection is healthy.
    live_note: String,
    // Collapsed/expanded cards by stable key; absent = page default.
    card_expanded: BTreeMap<String, bool>,
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
            live_note: String::new(),
            card_expanded: BTreeMap::new(),
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
    // Merged keybinds backing the Keybinds page rows; rebuilt with them.
    let keybind_binds: Rc<RefCell<Vec<keybinds::SourcedBind>>> = Rc::new(RefCell::new(Vec::new()));
    // Keybind action vocabulary: starts from the committed snapshot, then
    // refreshed from the installed `umbriel msg --help` on a worker thread.
    let kb_actions = Arc::new(Mutex::new(keybinds::builtin_actions()));

    let app = AppWindow::new().map_err(|err| anyhow::anyhow!("window creation failed: {err}"))?;

    {
        let shell = shell.borrow();
        app.set_config_path(pretty_path(&shell.path, &env).into());
        refresh_include_files(&app, &shell, &env);
        app.set_include_note(shell.includes.notes.join("; ").into());
    }
    app.set_dirty(false);
    app.set_app_version(env!("CARGO_PKG_VERSION").into());
    app.set_check_updates_on_start(settings.check_updates_on_start);
    app.set_dark_mode(settings.dark);
    app.global::<Theme>().set_dark(settings.dark);
    app.set_backup_note(backup_note(&settings, &env));
    app.set_backup_count_text(settings.backup_count.to_string().into());
    app.set_backup_dir_text(settings.backup_dir.clone().unwrap_or_default().into());
    {
        let names: Vec<SharedString> = {
            let actions = kb_actions.lock().expect("kb actions");
            actions.iter().map(|a| a.name.clone().into()).collect()
        };
        app.set_action_choices(Rc::new(VecModel::from(names)).into());
    }
    {
        let weak = app.as_weak();
        let kb_actions = Arc::clone(&kb_actions);
        std::thread::spawn(move || {
            let live = std::process::Command::new("umbriel")
                .args(["msg", "--help"])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .map(|out| keybinds::actions_from_help(&String::from_utf8_lossy(&out.stdout)))
                .filter(|actions| !actions.is_empty());
            if let Some(actions) = live {
                let _ = slint::invoke_from_event_loop(move || {
                    let names: Vec<SharedString> =
                        actions.iter().map(|a| a.name.clone().into()).collect();
                    if let Some(app) = weak.upgrade() {
                        app.set_action_choices(Rc::new(VecModel::from(names)).into());
                        *kb_actions.lock().expect("kb actions") = actions;
                    }
                });
            }
        });
    }

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
        let labels = setting_labels(&shell);
        let main = labels.len() - 1;
        let mut destinations = vec![labels[main].clone()];
        destinations.extend(labels[..main].iter().cloned());
        app.set_destinations(Rc::new(VecModel::from(destinations)).into());
    }

    // What's new: the bundled changelog section for the running version,
    // once per version, and only where an update makes sense (a fresh
    // install has nothing "new" yet). The mode check above already
    // computed both inputs.
    if matches!(mode, SetupMode::Normal | SetupMode::MissingUmbriel) {
        let sections = changelog::parse(changelog::bundled());
        if changelog::should_show(&env, env!("CARGO_PKG_VERSION"))
            && let Some(section) = changelog::for_version(&sections, env!("CARGO_PKG_VERSION"))
        {
            app.set_whatsnew_title(format!("What's new in {}", env!("CARGO_PKG_VERSION")).into());
            app.set_whatsnew_body(section.body.clone().into());
            app.set_show_whatsnew(true);
            changelog::mark_shown(&env, env!("CARGO_PKG_VERSION"));
        }
    }

    app.window().set_size(WindowSize::Logical(LogicalSize::new(
        settings.window_width as f32,
        settings.window_height as f32,
    )));

    {
        let weak = app.as_weak();
        let keybind_binds = Rc::clone(&keybind_binds);
        let kb_actions = Arc::clone(&kb_actions);
        let shell = Rc::clone(&shell);
        app.on_section_selected(move |name| {
            let Some(app) = weak.upgrade() else { return };
            // Outputs page: its own surface; rescan on open —
            // fast-fails when no compositor is reachable and keeps the
            // last detection.
            if name.as_str() == catalog::OUTPUTS_ID {
                scan_outputs(&mut shell.borrow_mut());
                app.set_current_section(name.clone());
                let (title, description) = page_meta(&name);
                app.set_page_title(title.into());
                app.set_page_description(description.into());
                app.set_page(Page::Outputs);
                rebuild_outputs(&app, &shell.borrow());
                return;
            }
            // Rule pages: their own surface, one family per page.
            if rule_family(&name).is_some() {
                app.set_current_section(name.clone());
                let (title, description) = page_meta(&name);
                app.set_page_title(title.into());
                app.set_page_description(description.into());
                app.set_page(Page::Rules);
                rebuild_rule_page(&app, &shell.borrow());
                return;
            }
            // Keybinds page: its own surface, not a CategoryPage.
            if name.as_str() == "keybinds" {
                app.set_current_section(name.clone());
                app.set_page(Page::Keybinds);
                let shell = shell.borrow();
                rebuild_keybind_rows(
                    &app,
                    &shell,
                    &kb_actions,
                    &app.get_keybind_search(),
                    &keybind_binds,
                );
                return;
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
            let labels = setting_labels(&shell);
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
            let label = setting_labels(&shell)
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
        // Row-level reset (the slider "Reset" button): back to the
        // stored value, without the save popup.
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_reset_key(move |key| {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut shell = shell.borrow_mut();
                reset_key(&mut shell, &key);
            }
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            refresh_row(&app, &shell, &key);
            app.set_changed_count(changed_count(&shell));
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
        let env = env.clone();
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

            let labels = setting_labels(&shell);
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

            // Backup the on-disk chain before any of it is overwritten.
            if shell.doc.is_modified()
                || shell.includes.docs.iter().any(|inc| inc.doc.is_modified())
            {
                snapshot_before_save(&shell, &env, "save");
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
            let (note, clean) = if empty {
                (
                    "No packaged default found; install umbriel and sync again.".to_owned(),
                    false,
                )
            } else if drift.is_empty() {
                ("Schema is up to date.".to_owned(), true)
            } else {
                (format!("Synced from umbriel: {}.", drift.summary()), false)
            };
            shell.schema = fresh;
            app.set_sync_note(note.clone().into());
            app.set_sync_clean(clean);
            app.set_status(note.into());
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
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_theme_selected(move |dark| {
            let Some(app) = weak.upgrade() else { return };
            app.set_dark_mode(dark);
            app.global::<Theme>().set_dark(dark);
            let mut settings = app_settings::load(&env);
            settings.dark = dark;
            let _ = app_settings::store(&env, &settings);
        });
    }
    let backup_runs: Rc<RefCell<Vec<backups::RunInfo>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let env = env.clone();
        let backup_runs = Rc::clone(&backup_runs);
        app.on_backups_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_page(Page::Backups);
            let shell = shell.borrow();
            refresh_backup_runs(&app, &shell, &env, &backup_runs);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let env = env.clone();
        let backup_runs = Rc::clone(&backup_runs);
        app.on_backup_now(move || {
            let Some(app) = weak.upgrade() else { return };
            let result = {
                let shell = shell.borrow();
                write_backup_run(&shell, &env, "manual")
            };
            app.set_backup_action_note(
                match result {
                    Ok(Some(id)) => format!("Backed up as {id}."),
                    Ok(None) => "Nothing to back up yet, no config on disk.".to_owned(),
                    Err(err) => format!("Backup failed: {err}"),
                }
                .into(),
            );
            let shell = shell.borrow();
            refresh_backup_runs(&app, &shell, &env, &backup_runs);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let env = env.clone();
        let backup_runs = Rc::clone(&backup_runs);
        app.on_backup_chosen(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let run = backup_runs.borrow().get(index as usize).cloned();
            let Some(run) = run else { return };
            app.set_backup_selected(index);
            let settings = app_settings::load(&env);
            let base = backup_base(&settings, &env);
            let shell = shell.borrow();
            app.set_restore_diff(backup_diff(&shell, &base, &run.id).into());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let env = env.clone();
        let backup_runs = Rc::clone(&backup_runs);
        app.on_restore_backup(move || {
            let Some(app) = weak.upgrade() else { return };
            let run_id = match backup_runs.borrow().get(app.get_backup_selected() as usize) {
                Some(run) => run.id.clone(),
                None => return,
            };
            let restored = {
                let mut shell = shell.borrow_mut();
                // A restore is itself reversible: snapshot first.
                snapshot_before_save(&shell, &env, "restore");
                let base = backup_base(&app_settings::load(&env), &env);
                let backup = match backups::read_run(&base, &run_id) {
                    Ok(backup) => backup,
                    Err(err) => {
                        app.set_backup_action_note(format!("Restore failed: {err}").into());
                        return;
                    }
                };
                let mut chain: Vec<(String, PathBuf)> =
                    vec![(file_name_of(&shell.path), shell.path.clone())];
                chain.extend(
                    shell
                        .includes
                        .docs
                        .iter()
                        .map(|inc| (file_name_of(&inc.path), inc.path.clone())),
                );
                let mut restored = 0;
                for (name, content) in &backup {
                    if let Some((_, path)) = chain.iter().find(|(n, _)| n == name)
                        && std::fs::write(path, content).is_ok()
                    {
                        restored += 1;
                    }
                }
                let path = shell.path.clone();
                *shell = Shell::load(&path, &env);
                restored
            };
            {
                let shell = shell.borrow();
                app.set_config_path(pretty_path(&shell.path, &env).into());
                refresh_include_files(&app, &shell, &env);
                app.set_include_note(shell.includes.notes.join("; ").into());
                app.set_dirty(false);
                app.set_changed_count(0);
                app.set_sections(Rc::new(VecModel::from(section_nav(&shell))).into());
                let section = app.get_current_section().to_string();
                refill_page(&app, &shell, &section);
                refresh_backup_runs(&app, &shell, &env, &backup_runs);
            }
            match validate::validate(&shell.borrow().path) {
                Ok(report) if report.is_ok() => app.set_validate_note(String::new().into()),
                Ok(report) => app.set_validate_note(
                    format!(
                        "umbriel: {}.",
                        report
                            .diagnostics
                            .iter()
                            .map(|d| d.message())
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                    .into(),
                ),
                Err(err) => app.set_validate_note(format!("validation skipped ({err})").into()),
            }
            app.set_backup_action_note(
                format!("Restored {restored} file(s) from {run_id}.").into(),
            );
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_backup_count_chosen(move |text| {
            let Some(app) = weak.upgrade() else { return };
            let parsed = text.trim().parse::<u32>().map(|count| count.max(1)).ok();
            let mut settings = app_settings::load(&env);
            match parsed {
                Some(count) => {
                    settings.backup_count = count;
                    let _ = app_settings::store(&env, &settings);
                    app.set_backup_count_text(count.to_string().into());
                    app.set_backup_note(backup_note(&settings, &env));
                    app.set_backup_action_note(format!("Keeping {count} backups.").into());
                }
                None => {
                    app.set_backup_count_text(settings.backup_count.to_string().into());
                    app.set_backup_action_note("Keep count must be a number of 1 or more.".into());
                }
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let env = env.clone();
        let backup_runs = Rc::clone(&backup_runs);
        app.on_backup_dir_chosen(move |text| {
            let Some(app) = weak.upgrade() else { return };
            let trimmed = text.trim().to_owned();
            let shell = shell.borrow();
            apply_backup_dir(&app, &shell, &env, &backup_runs, &trimmed);
        });
    }
    {
        let weak = app.as_weak();
        app.on_browse_location(move || {
            let weak = weak.clone();
            // The portal dialog runs outside our process; picking happens
            // off-thread so the UI loop never blocks. Only the weak handle
            // crosses the thread — applying the picked folder is delegated
            // to the main-thread dir-chosen handler.
            std::thread::spawn(move || {
                let picked = rfd::FileDialog::new().pick_folder();
                let _ = slint::invoke_from_event_loop(move || {
                    let (Some(app), Some(path)) = (weak.upgrade(), picked) else {
                        return;
                    };
                    app.invoke_backup_dir_chosen(path.to_string_lossy().to_string().into());
                });
            });
        });
    }
    {
        let weak = app.as_weak();
        app.on_view_changelog(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_whatsnew_title("Changelog".into());
            app.set_whatsnew_body(
                changelog::full_text(&changelog::parse(changelog::bundled())).into(),
            );
            app.set_show_whatsnew(true);
        });
    }
    {
        let weak = app.as_weak();
        app.on_view_update_notes(move || {
            let Some(app) = weak.upgrade() else { return };
            // The fetched notes are the new release's body; fall back to
            // the bundled section when the check hasn't run.
            let notes = app.get_update_notes().to_string();
            if notes.is_empty() {
                let sections = changelog::parse(changelog::bundled());
                let section = changelog::for_version(&sections, env!("CARGO_PKG_VERSION"));
                app.set_whatsnew_title(
                    format!("What's new in {}", env!("CARGO_PKG_VERSION")).into(),
                );
                app.set_whatsnew_body(
                    section
                        .map(|section| section.body.clone())
                        .unwrap_or_default()
                        .into(),
                );
            } else {
                app.set_whatsnew_title("What's new in this release".into());
                app.set_whatsnew_body(notes.into());
            }
            app.set_show_whatsnew(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_toggle_card(move |key| {
            let Some(app) = weak.upgrade() else { return };
            let open = {
                let shell = shell.borrow();
                // The guide renders through the same cards; its step
                // must survive the rebuild.
                let in_guide = shell.guide.is_some();
                (in_guide, !card_expanded(&shell, &key, true))
            };
            shell
                .borrow_mut()
                .card_expanded
                .insert(key.to_string(), open.1);
            let shell = shell.borrow();
            if open.0 {
                guide_show_step(&app, &shell);
            } else {
                refill_page(&app, &shell, &app.get_current_section());
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_toggle_monitor(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let key = format!("monitor:{name}");
            let open = !card_expanded(&shell.borrow(), &key, true);
            shell.borrow_mut().card_expanded.insert(key, open);
            let shell = shell.borrow();
            rebuild_outputs(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_toggle_rule(move |file, index| {
            let Some(app) = weak.upgrade() else { return };
            let Some(family) = rule_family(&app.get_current_section()) else {
                return;
            };
            let key = format!("rule:{family}:{file}:{index}");
            // The default (open when alone) depends on the card count,
            // so flip whatever the model shows right now.
            let open = {
                let shell = shell.borrow();
                rule_cards(&shell, family)
                    .into_iter()
                    .find(|card| card.file == file && card.index == index)
                    .map(|card| !card.expanded)
            };
            let Some(open) = open else { return };
            shell.borrow_mut().card_expanded.insert(key, open);
            let shell = shell.borrow();
            rebuild_rule_page(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_outputs_refresh(move || {
            let Some(app) = weak.upgrade() else { return };
            scan_outputs(&mut shell.borrow_mut());
            let shell = shell.borrow();
            rebuild_outputs(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_output_add(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let name = name.trim().to_owned();
            let taken = {
                let shell = shell.borrow();
                let mut names = outputs::configured(&shell.doc);
                for monitor in &shell.guide_monitors {
                    if !names.contains(&monitor.name) {
                        names.push(monitor.name.clone());
                    }
                }
                names.contains(&name) || name.is_empty()
            };
            if taken {
                app.set_status(if name.is_empty() {
                    "Enter a connector name first, e.g. DP-3.".into()
                } else {
                    format!("{name} is already listed").into()
                });
                return;
            }
            shell
                .borrow_mut()
                .doc
                .set_bool(&["output", &name, "enabled"], true);
            shell
                .borrow_mut()
                .card_expanded
                .insert(format!("monitor:{name}"), true);
            app.set_outputs_add_name(String::new().into());
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_outputs(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_output_remove(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let removed = {
                let mut shell = shell.borrow_mut();
                let only = outputs::configured(&shell.doc).len() <= 1;
                if only {
                    None
                } else {
                    shell.doc.remove_table(&["output", &name]).then_some(())
                }
            };
            if removed.is_none() {
                app.set_status("The only configured output cannot be removed.".into());
                return;
            }
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_outputs(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_rule_add(move || {
            let Some(app) = weak.upgrade() else { return };
            let section = app.get_current_section().to_string();
            let Some(family) = rule_family(&section) else {
                return;
            };
            let target = rule_add_target(&shell.borrow(), family);
            doc_at_mut(&mut shell.borrow_mut(), target).add_rule(family);
            // The new rule sits at the end of its file; open it so the
            // fields are right there to fill in.
            let new_index = doc_at(&shell.borrow(), target).rule_count(family) - 1;
            shell
                .borrow_mut()
                .card_expanded
                .insert(format!("rule:{family}:{target}:{new_index}"), true);
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_rule_page(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        app.on_rule_remove(move |file, index| {
            let Some(app) = weak.upgrade() else { return };
            let section = app.get_current_section().to_string();
            let Some(family) = rule_family(&section) else {
                return;
            };
            doc_at_mut(&mut shell.borrow_mut(), file as usize).remove_rule(family, index as usize);
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_rule_page(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let keybind_binds = Rc::clone(&keybind_binds);
        let kb_actions = Arc::clone(&kb_actions);
        app.on_keybind_search_edited(move |text| {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            rebuild_keybind_rows(&app, &shell, &kb_actions, &text, &keybind_binds);
        });
    }
    {
        let weak = app.as_weak();
        app.on_keybind_add(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_keybind_editing(false);
            app.set_kb_original_index(-1);
            app.set_kb_conflict_note("".into());
            app.set_kb_draft_chord("".into());
            app.set_kb_draft_use_mod(true);
            app.set_kb_draft_scope("".into());
            app.set_kb_draft_action(0);
            app.set_kb_draft_param("".into());
            app.set_kb_draft_no_repeat(false);
            app.set_kb_draft_locked(false);
            app.set_kb_draft_submap("".into());
            app.set_kb_editor_open(true);
        });
    }
    {
        let weak = app.as_weak();
        let keybind_binds = Rc::clone(&keybind_binds);
        app.on_keybind_edit(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let bind = keybind_binds.borrow().get(index as usize).cloned();
            let Some(bind) = bind else { return };
            let draft = keybinds::BindDraft::from_parts(
                &bind.chord,
                &bind.action,
                bind.repeat,
                bind.allow_when_locked,
                bind.submap.clone(),
            );
            let (name, param) = bind.action.split_once(':').unwrap_or((&bind.action, ""));
            let choices = app.get_action_choices();
            let mut names: Vec<String> = (0..choices.row_count())
                .filter_map(|i| choices.row_data(i).map(|s| s.to_string()))
                .collect();
            let action_index = match names.iter().position(|candidate| candidate == name) {
                Some(position) => position,
                None => {
                    // An action outside the vocabulary (a newer umbriel):
                    // surface it so the edit still round-trips.
                    names.push(name.to_owned());
                    let model: Vec<SharedString> = names
                        .iter()
                        .map(|n| SharedString::from(n.clone()))
                        .collect();
                    app.set_action_choices(Rc::new(VecModel::from(model)).into());
                    names.len() - 1
                }
            };
            app.set_keybind_editing(true);
            app.set_kb_original_index(index);
            app.set_kb_original_chord(bind.chord.clone().into());
            app.set_kb_draft_chord(draft.chord.into());
            app.set_kb_draft_use_mod(draft.use_mod);
            app.set_kb_draft_scope(draft.scope.into());
            app.set_kb_draft_action(action_index as i32);
            app.set_kb_draft_param(param.into());
            app.set_kb_draft_no_repeat(bind.repeat == Some(false));
            app.set_kb_draft_locked(bind.allow_when_locked == Some(true));
            app.set_kb_draft_submap(bind.submap.clone().unwrap_or_default().into());
            app.set_kb_conflict_note("".into());
            app.set_kb_editor_open(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let keybind_binds = Rc::clone(&keybind_binds);
        let kb_actions = Arc::clone(&kb_actions);
        app.on_keybind_remove(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let bind = keybind_binds.borrow().get(index as usize).cloned();
            let Some(bind) = bind else { return };
            let mut shell = shell.borrow_mut();
            let removed = if bind.source_file >= shell.includes.docs.len() {
                shell.doc.remove_keybind(&bind.chord)
            } else {
                shell.includes.docs[bind.source_file]
                    .doc
                    .remove_keybind(&bind.chord)
            };
            if removed {
                rebuild_keybind_rows(
                    &app,
                    &shell,
                    &kb_actions,
                    &app.get_keybind_search(),
                    &keybind_binds,
                );
                app.set_dirty(shell.any_modified());
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        let keybind_binds = Rc::clone(&keybind_binds);
        let kb_actions = Arc::clone(&kb_actions);
        app.on_keybind_apply(move || {
            let Some(app) = weak.upgrade() else { return };
            let chord_body = app.get_kb_draft_chord().trim().to_owned();
            if chord_body.is_empty() {
                app.set_kb_conflict_note("Enter a chord, e.g. Return with Mod on.".into());
                return;
            }
            let action_index = app.get_kb_draft_action().max(0) as usize;
            let choices = app.get_action_choices();
            let Some(action_name) = choices.row_data(action_index) else {
                app.set_kb_conflict_note("Pick an action.".into());
                return;
            };
            let param = app.get_kb_draft_param().trim().to_owned();
            let action = if param.is_empty() {
                action_name.to_string()
            } else {
                format!("{action_name}:{param}")
            };
            let submap = app.get_kb_draft_submap().trim().to_owned();
            let draft = keybinds::BindDraft {
                chord: chord_body,
                use_mod: app.get_kb_draft_use_mod(),
                scope: app.get_kb_draft_scope().trim().to_owned(),
                action: action.clone(),
                repeat: if app.get_kb_draft_no_repeat() {
                    Some(false)
                } else {
                    None
                },
                allow_when_locked: if app.get_kb_draft_locked() {
                    Some(true)
                } else {
                    None
                },
                submap: (!submap.is_empty()).then_some(submap),
            };
            let chord = draft.composed_chord();
            let editing = app.get_keybind_editing();
            let original = app.get_kb_original_chord().to_string();
            {
                let shell = shell.borrow();
                let docs = keybind_docs(&shell);
                if let Some((_file, other)) =
                    keybinds::find_conflict(&docs, &chord, editing.then_some(original.as_str()))
                {
                    app.set_kb_conflict_note(
                        format!("Already bound to {other} — pick another chord.").into(),
                    );
                    return;
                }
            }
            let mut shell = shell.borrow_mut();
            let write = |doc: &mut ConfigDocument| {
                if !original.eq_ignore_ascii_case(&chord) {
                    doc.remove_keybind(&original);
                }
                doc.set_keybind(
                    &chord,
                    &action,
                    draft.repeat,
                    draft.allow_when_locked,
                    draft.submap.as_deref(),
                );
            };
            if editing {
                let index = app.get_kb_original_index().max(0) as usize;
                let source = keybind_binds
                    .borrow()
                    .get(index)
                    .map(|bind| bind.source_file);
                match source {
                    Some(source) if source >= shell.includes.docs.len() => {
                        write(&mut shell.doc);
                    }
                    Some(source) => {
                        write(&mut shell.includes.docs[source].doc);
                    }
                    None => return,
                }
            } else {
                // New binds land where [keybinds] already lives: the main
                // file if it has binds, else the first include that does,
                // else the main file (created on save).
                let main_owns = !shell.doc.keybinds().is_empty();
                let include_owns = (!main_owns).then(|| {
                    shell
                        .includes
                        .docs
                        .iter()
                        .position(|inc| !inc.doc.keybinds().is_empty())
                });
                match include_owns {
                    Some(Some(index)) => write(&mut shell.includes.docs[index].doc),
                    _ => write(&mut shell.doc),
                }
            }
            app.set_kb_editor_open(false);
            rebuild_keybind_rows(
                &app,
                &shell,
                &kb_actions,
                &app.get_keybind_search(),
                &keybind_binds,
            );
            app.set_dirty(shell.any_modified());
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
        let env = env.clone();
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
            if shell.doc.is_modified()
                || shell.includes.docs.iter().any(|inc| inc.doc.is_modified())
            {
                snapshot_before_save(&shell, &env, "save");
            }
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
            // Rule fields ("window_rule[0].match.app_id") write straight
            // to the owning document — no schema formatting, no
            // save-popup destination.
            if let Some((family, doc, index, field_key)) = rules::parse_rule_key(key) {
                let field = rules::fields(family)
                    .0
                    .iter()
                    .chain(rules::fields(family).1)
                    .find(|field| field.key == field_key);
                let Some(field) = field else { return };
                let target = {
                    let shell = shell.borrow();
                    doc.unwrap_or_else(|| rule_target(&shell, family))
                };
                let result = {
                    let mut shell = shell.borrow_mut();
                    rules::apply_field_text(
                        doc_at_mut(&mut shell, target),
                        family,
                        index,
                        field,
                        raw,
                    )
                };
                if let Err(err) = result {
                    app.set_status(err.into());
                    return;
                }
                let shell = shell.borrow();
                app.set_dirty(shell.any_modified());
                rebuild_rule_page(app, &shell);
                return;
            }
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
fn backup_base(settings: &app_settings::Settings, env: &discovery::Env) -> PathBuf {
    match settings.backup_dir.as_deref() {
        Some(dir) => PathBuf::from(dir),
        None => backups::default_base(env),
    }
}

fn backup_note(settings: &app_settings::Settings, env: &discovery::Env) -> SharedString {
    format!(
        "Saving to {} · keeping {} backups",
        backup_base(settings, env).display(),
        settings.backup_count
    )
    .into()
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Dropdown label for one backup run, e.g.
/// `2026-09-09 18:04:31 UTC · save · 2 files`.
fn backup_choice(run: &backups::RunInfo) -> SharedString {
    let (date, time) = run.id.split_once('T').unwrap_or((run.id.as_str(), ""));
    let clock = match time.split('-').collect::<Vec<_>>().as_slice() {
        [h, m, s] => format!("{h}:{m}:{s}"),
        [h, m, s, n] => format!("{h}:{m}:{s} (#{n})"),
        _ => time.to_owned(),
    };
    format!(
        "{date} {clock} UTC · {} · {}",
        run.trigger,
        if run.files.len() == 1 {
            "1 file".to_owned()
        } else {
            format!("{} files", run.files.len())
        }
    )
    .into()
}

/// Unified diff (backup vs current content) for every file in a run.
fn backup_diff(shell: &Shell, base: &Path, id: &str) -> String {
    let files = match backups::read_run(base, id) {
        Ok(files) => files,
        Err(err) => return format!("Backup unreadable: {err}"),
    };
    let mut chain: Vec<(String, PathBuf)> = vec![(file_name_of(&shell.path), shell.path.clone())];
    chain.extend(
        shell
            .includes
            .docs
            .iter()
            .map(|inc| (file_name_of(&inc.path), inc.path.clone())),
    );
    let mut out = String::new();
    for (name, old) in &files {
        out += &format!("──── {name} ────\n");
        let current = chain
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, path)| std::fs::read_to_string(path).ok());
        match current {
            Some(current) => {
                let diff = similar::TextDiff::from_lines(old, &current);
                let text = diff.unified_diff().context_radius(3).to_string();
                out += if text.is_empty() {
                    "(identical to the current files)\n"
                } else {
                    &text
                };
            }
            None => out += "(not part of the current config chain)\n",
        }
        out.push('\n');
    }
    out
}

/// Snapshot the on-disk chain into a backup run before a save overwrites
/// it. Best effort: a backup failure is never allowed to block a save.
fn snapshot_before_save(shell: &Shell, env: &discovery::Env, trigger: &str) {
    let _ = write_backup_run(shell, env, trigger);
}

/// Write one backup run from the on-disk chain; `Ok(None)` when there is
/// nothing on disk to back up yet.
fn write_backup_run(
    shell: &Shell,
    env: &discovery::Env,
    trigger: &str,
) -> std::io::Result<Option<String>> {
    let settings = app_settings::load(env);
    let base = backup_base(&settings, env);
    let mut chain = vec![shell.path.clone()];
    chain.extend(shell.includes.docs.iter().map(|inc| inc.path.clone()));
    let files: Vec<(PathBuf, String)> = chain
        .into_iter()
        .filter_map(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            Some((path, content))
        })
        .collect();
    if files.is_empty() {
        return Ok(None);
    }
    let id = backups::snapshot_run(&base, &shell.path, trigger, &files)?;
    backups::prune_runs(&base, settings.backup_count.max(1) as usize);
    Ok(Some(id))
}

/// Reload the runs list and the diff for its first (newest) entry.
fn refresh_backup_runs(
    app: &AppWindow,
    shell: &Shell,
    env: &discovery::Env,
    runs_out: &Rc<RefCell<Vec<backups::RunInfo>>>,
) {
    let base = backup_base(&app_settings::load(env), env);
    let found = backups::list_runs(&base);
    let choices: Vec<SharedString> = found.iter().map(backup_choice).collect();
    let possible = !found.is_empty();
    let diff = possible.then(|| backup_diff(shell, &base, &found[0].id));
    app.set_backup_choices(Rc::new(VecModel::from(choices)).into());
    app.set_backup_selected(0);
    app.set_restore_possible(possible);
    app.set_restore_diff(
        diff.unwrap_or_else(|| {
            "No backups yet — they appear here after your first save.".to_owned()
        })
        .into(),
    );
    *runs_out.borrow_mut() = found;
}

/// Mirror the include chain into the Settings page's file list.
fn refresh_include_files(app: &AppWindow, shell: &Shell, env: &discovery::Env) {
    let files: Vec<SharedString> = shell
        .includes
        .docs
        .iter()
        .map(|inc| pretty_path(&inc.path, env).into())
        .collect();
    app.set_include_files(Rc::new(VecModel::from(files)).into());
}

/// Home-collapsed path for display: `$HOME/…` becomes `~/…`.
fn pretty_path(path: &Path, env: &discovery::Env) -> String {
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

/// Docs in umbriel's precedence: includes first, the main file last (it
/// wins). Same indexing as `doc_at` — keybind source_file indices refer
/// to it.
fn keybind_docs(shell: &Shell) -> Vec<&ConfigDocument> {
    let mut docs: Vec<&ConfigDocument> = shell.includes.docs.iter().map(|inc| &inc.doc).collect();
    docs.push(&shell.doc);
    docs
}

/// The file a merged bind lives in, for the list's source label.
fn keybind_source(shell: &Shell, file: usize) -> String {
    if file >= shell.includes.docs.len() {
        file_name_of(&shell.path)
    } else {
        file_name_of(&shell.includes.docs[file].path)
    }
}

/// Non-default extras worth showing on a row.
fn keybind_extras(bind: &keybinds::SourcedBind) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if bind.repeat == Some(false) {
        parts.push("once");
    }
    if bind.allow_when_locked == Some(true) {
        parts.push("works when locked");
    }
    if bind.submap.is_some() {
        parts.push("submap");
    }
    parts.join(" · ")
}

/// Rebuild the Keybinds page rows (filtered by `filter`) and keep the
/// merged bind list in sync for the edit/remove handlers.
fn rebuild_keybind_rows(
    app: &AppWindow,
    shell: &Shell,
    kb_actions: &Arc<Mutex<Vec<keybinds::LiveAction>>>,
    filter: &str,
    binds_out: &Rc<RefCell<Vec<keybinds::SourcedBind>>>,
) {
    let actions = kb_actions.lock().expect("kb actions").clone();
    let docs = keybind_docs(shell);
    let merged = keybinds::merged_binds(&docs);
    let lowered = filter.trim().to_lowercase();

    struct KbEntry {
        family: &'static str,
        chord: String,
        summary: String,
        extras: String,
        source: String,
        is_user: bool,
        shadowing: bool,
        bind: usize,
    }
    let mut entries: Vec<KbEntry> = Vec::new();
    for (bind_index, bind) in merged.iter().enumerate() {
        let shadowing = keybinds::DEFAULT_BINDS
            .iter()
            .any(|(chord, _)| chord.eq_ignore_ascii_case(&bind.chord));
        entries.push(KbEntry {
            family: keybinds::action_group(&bind.action),
            chord: bind.chord.clone(),
            summary: keybinds::describe(&bind.action, &actions),
            extras: keybind_extras(bind),
            source: keybind_source(shell, bind.source_file),
            is_user: true,
            shadowing,
            bind: bind_index,
        });
    }
    for (chord, action) in keybinds::DEFAULT_BINDS {
        if merged
            .iter()
            .any(|bind| bind.chord.eq_ignore_ascii_case(chord))
        {
            continue; // a user bind shadows this default and is shown instead
        }
        entries.push(KbEntry {
            family: keybinds::action_group(action),
            chord: (*chord).to_owned(),
            summary: keybinds::describe(action, &actions),
            extras: String::new(),
            source: String::new(),
            is_user: false,
            shadowing: false,
            bind: usize::MAX,
        });
    }
    entries.retain(|entry| {
        lowered.is_empty()
            || entry.chord.to_lowercase().contains(&lowered)
            || entry.summary.to_lowercase().contains(&lowered)
    });
    entries.sort_by(|a, b| {
        let rank = |family: &str| {
            keybinds::GROUP_ORDER
                .iter()
                .position(|known| *known == family)
                .unwrap_or(keybinds::GROUP_ORDER.len())
        };
        rank(a.family)
            .cmp(&rank(b.family))
            .then_with(|| a.chord.cmp(&b.chord))
    });

    let mut rows: Vec<KeybindRow> = Vec::new();
    let mut last_family = String::new();
    for entry in entries {
        if entry.family != last_family {
            last_family = entry.family.to_owned();
            rows.push(KeybindRow {
                chord: entry.family.to_uppercase().into(),
                summary: String::new().into(),
                extras: String::new().into(),
                source: String::new().into(),
                is_header: true,
                is_user: false,
                shadowing: false,
                index: -1,
            });
        }
        rows.push(KeybindRow {
            chord: entry.chord.into(),
            summary: entry.summary.into(),
            extras: entry.extras.into(),
            source: entry.source.into(),
            is_header: false,
            is_user: entry.is_user,
            shadowing: entry.shadowing,
            index: entry.bind as i32,
        });
    }
    app.set_keybind_rows(Rc::new(VecModel::from(rows)).into());
    *binds_out.borrow_mut() = merged;
}

/// Store a new backup location (empty = default), refresh the list, and
/// say so inline. Shared by the text field and the Browse dialog.
fn apply_backup_dir(
    app: &AppWindow,
    shell: &Shell,
    env: &discovery::Env,
    runs_out: &Rc<RefCell<Vec<backups::RunInfo>>>,
    dir: &str,
) {
    let mut settings = app_settings::load(env);
    settings.backup_dir = (!dir.is_empty()).then(|| dir.to_owned());
    if let Some(path) = &settings.backup_dir {
        let _ = std::fs::create_dir_all(path);
    }
    let _ = app_settings::store(env, &settings);
    app.set_backup_dir_text(dir.to_owned().into());
    app.set_backup_note(backup_note(&settings, env));
    refresh_backup_runs(app, shell, env, runs_out);
    app.set_backup_action_note(
        if dir.is_empty() {
            "Backups will be saved to the default location.".to_owned()
        } else {
            format!("Backups will be saved to {dir}.").to_owned()
        }
        .into(),
    );
}

/// The toggle is read from the live property so a change made on the
/// Settings page survives exit.
fn store_window_settings(app: &AppWindow, env: &discovery::Env) {
    let scale = app.window().scale_factor();
    let size = app.window().size();
    // Load-mutate-store: fields without a live property (backup prefs)
    // must survive a window-settings save untouched.
    let mut settings = app_settings::load(env);
    settings.check_updates_on_start = app.get_check_updates_on_start();
    settings.window_width = (size.width as f32 / scale) as u32;
    settings.window_height = (size.height as f32 / scale) as u32;
    settings.dark = app.get_dark_mode();
    let _ = app_settings::store(env, &settings);
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
                Ok(update::Verdict::UpdateAvailable { version, notes }) => {
                    app.set_update_available(true);
                    app.set_update_note(format!("Version {version} available.").into());
                    if let Some(notes) = notes {
                        app.set_update_notes(notes.into());
                    }
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
            .header("User-Agent", "umbriel-config")
            .config()
            .timeout_global(Some(std::time::Duration::from_secs(10)))
            .build()
            .call()
            .map_err(|err| err.to_string())?
            .body_mut()
            .read_json()
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
fn setting_labels(shell: &Shell) -> Vec<SharedString> {
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
    let labels = setting_labels(shell);
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
    app.set_cards(Rc::new(VecModel::from(page_cards(shell, page_id))).into());
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
            label: catalog::group_title(group).to_uppercase().into(),
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
            label: catalog::group_title(catalog::MORE_GROUP)
                .to_uppercase()
                .into(),
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
fn page_cards(shell: &Shell, page_id: &str) -> Vec<SettingsCard> {
    if page_id == catalog::OUTPUTS_ID {
        return output_cards(shell);
    }
    if let Some(page) = catalog::page(page_id) {
        return catalog_page_cards(shell, page);
    }
    fallback_page_cards(shell, page_id)
}

/// A catalog page: one card per curated sub-section, then any
/// sub-sections of the same areas the catalog doesn't claim yet, then
/// the uncovered-keys card.
fn catalog_page_cards(shell: &Shell, page: &catalog::Page) -> Vec<SettingsCard> {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();

    let mut cards: Vec<SettingsCard> = Vec::new();
    let mut claimed: Vec<&str> = Vec::new();
    for card in page.cards {
        claimed.push(card.section);
        let rows: Vec<SettingRow> = shell
            .schema
            .iter()
            .filter(|entry| entry.section == card.section)
            .map(|entry| schema_row(shell, &sets, &labels, &current, entry))
            .collect();
        if !rows.is_empty() {
            let key = format!("card:{}:{}", page.id, card.title);
            cards.push(SettingsCard {
                title: card.title.into(),
                key: key.clone().into(),
                expanded: card_expanded(shell, &key, true),
                rows: Rc::new(VecModel::from(rows)).into(),
            });
        }
    }

    let tops = catalog::page_top_levels(page);
    let mut auto: BTreeMap<String, Vec<SettingRow>> = BTreeMap::new();
    for entry in &shell.schema {
        let top = entry.section.split('.').next().unwrap_or("");
        if !tops.contains(&top) || claimed.contains(&entry.section.as_str()) {
            continue;
        }
        let row = schema_row(shell, &sets, &labels, &current, entry);
        auto.entry(prettify(&entry.section)).or_default().push(row);
    }
    for (title, rows) in auto {
        let key = format!("card:{}:{title}", page.id);
        cards.push(SettingsCard {
            key: key.clone().into(),
            expanded: card_expanded(shell, &key, true),
            title: title.into(),
            rows: Rc::new(VecModel::from(rows)).into(),
        });
    }
    if let Some(other) = other_card(shell, &sets, &labels, &current, &tops) {
        cards.push(other);
    }
    cards
}

/// A MORE fallback page: every sub-section of one top-level area.
fn fallback_page_cards(shell: &Shell, top: &str) -> Vec<SettingsCard> {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();

    let mut cards: BTreeMap<String, Vec<SettingRow>> = BTreeMap::new();
    for entry in &shell.schema {
        if entry.section.split('.').next() != Some(top) {
            continue;
        }
        let row = schema_row(shell, &sets, &labels, &current, entry);
        cards.entry(prettify(&entry.section)).or_default().push(row);
    }
    let mut out: Vec<SettingsCard> = cards
        .into_iter()
        .map(|(title, rows)| SettingsCard {
            key: title.clone().into(),
            expanded: true,
            title: title.into(),
            rows: Rc::new(VecModel::from(rows)).into(),
        })
        .collect();
    if let Some(other) = other_card(shell, &sets, &labels, &current, &[top]) {
        out.push(other);
    }
    out
}

/// Keys beyond the schema fold onto the page that owns their area (the
/// old Other-settings sweep): one read-only row per key, owned like any
/// other row. Only keys whose top-level matches one of `tops` are shown.
fn other_card(
    shell: &Shell,
    sets: &[BTreeSet<String>],
    labels: &[SharedString],
    current: &[BTreeMap<String, String>],
    tops: &[&str],
) -> Option<SettingsCard> {
    let docs: Vec<&ConfigDocument> = shell.includes.docs.iter().map(|inc| &inc.doc).collect();
    let claims = schema::managed_claims(&docs);
    let schema_keys = schema::key_set(&shell.schema);
    let mut other: BTreeMap<String, SettingRow> = BTreeMap::new();
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
                SettingRow {
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
                    preview: String::new().into(),
                },
            );
        }
    }
    if other.is_empty() {
        return None;
    }
    Some(SettingsCard {
        key: "other".into(),
        expanded: true,
        title: "other".into(),
        rows: Rc::new(VecModel::from(other.into_values().collect::<Vec<_>>())).into(),
    })
}

/// One schema row, editor-ready: typed value, checked state, swatch, hint.
fn setting_row(doc: &ConfigDocument, entry: &schema::Entry) -> SettingRow {
    let value = typed_value(doc, entry).unwrap_or_else(|| "—".to_owned());
    let parts: Vec<&str> = entry.path.iter().map(String::as_str).collect();
    let (min, max) = kind_bounds(&entry.kind);
    SettingRow {
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
        preview: String::new().into(),
    }
}

fn refresh_row(app: &AppWindow, shell: &Shell, key: &str) {
    rebuild_row(app, shell, key, false);
}

/// Rebuild a row and overwrite it even when the text is unchanged — for
/// when a sibling edit changes this row's options (resolution → refresh).
fn rebuild_row(app: &AppWindow, shell: &Shell, key: &str, force: bool) {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let row = if let Some(entry) = shell
        .schema
        .iter()
        .find(|entry| entry.path.join(".") == key)
    {
        let home = entry_home(&sets, key).unwrap_or(shell.includes.docs.len());
        let current: BTreeMap<String, String> =
            doc_at(shell, home).leaf_values().into_iter().collect();
        let mut row = setting_row(doc_at(shell, home), entry);
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

    let cards = app.get_cards();
    let Some(cards) = cards.as_any().downcast_ref::<VecModel<SettingsCard>>() else {
        return;
    };
    for gi in 0..cards.row_count() {
        let Some(card) = cards.row_data(gi) else {
            continue;
        };
        let Some(rows) = card.rows.as_any().downcast_ref::<VecModel<SettingRow>>() else {
            continue;
        };
        for ri in 0..rows.row_count() {
            let Some(old) = rows.row_data(ri) else {
                continue;
            };
            if old.key.as_str() == key {
                // Same text = nothing to re-render; keeps the editor's
                // focus. A forced rebuild also swaps changed options, and
                // a lingering drag preview always clears.
                if force || old.value != row.value || !old.preview.is_empty() {
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
    let cards = app.get_cards();
    let Some(cards) = cards.as_any().downcast_ref::<VecModel<SettingsCard>>() else {
        return;
    };
    for gi in 0..cards.row_count() {
        let Some(card) = cards.row_data(gi) else {
            continue;
        };
        let Some(rows) = card.rows.as_any().downcast_ref::<VecModel<SettingRow>>() else {
            continue;
        };
        for ri in 0..rows.row_count() {
            let Some(mut old) = rows.row_data(ri) else {
                continue;
            };
            if old.key.as_str() == key {
                // Patch the preview text only: touching `value` would
                // re-evaluate the Slider's binding mid-drag and kill the
                // gesture (the thumb fights the user at the extremes).
                if old.preview.as_str() != value_text {
                    old.preview = value_text.into();
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

/// Render the guide's current step into the cards model and update
/// the card heading/controls.
fn guide_show_step(app: &AppWindow, shell: &Shell) {
    let Some(guide) = shell.guide.as_ref() else {
        return;
    };
    let (title, metas) = &guide.steps[guide.index];
    if title == "output" {
        app.set_cards(Rc::new(VecModel::from(output_cards(shell))).into());
    } else {
        app.set_cards(Rc::new(VecModel::from(guide_cards(shell, metas))).into());
    }
    app.set_guide_title(guide_title(title, guide.index + 1, guide.steps.len()).into());
    app.set_guide_first(guide.index == 0);
    app.set_guide_last(guide.index == guide.steps.len() - 1);
}

/// One guided-setup card: the step's curated keys in order, resolved to
/// schema rows (keys this umbriel's schema lacks are skipped). The
/// curated label and description replace the mined ones — the guide
/// speaks human, not config-file.
fn guide_cards(shell: &Shell, metas: &[&'static GuideKey]) -> Vec<SettingsCard> {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();
    let rows: Vec<SettingRow> = metas
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
    vec![SettingsCard {
        title: String::new().into(),
        key: String::new().into(),
        expanded: true,
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
) -> SettingRow {
    let main = shell.includes.docs.len();
    let dotted = entry.dotted();
    let home = entry_home(sets, &dotted);
    let mut row = setting_row(doc_at(shell, home.unwrap_or(main)), entry);
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
fn output_cards(shell: &Shell) -> Vec<SettingsCard> {
    let current: BTreeMap<String, String> = doc_at(shell, shell.includes.docs.len())
        .leaf_values()
        .into_iter()
        .collect();
    output_names(shell)
        .iter()
        .map(|name| {
            let key = format!("card:output:{name}");
            SettingsCard {
                title: name.clone().into(),
                key: key.clone().into(),
                expanded: card_expanded(shell, &key, true),
                rows: Rc::new(VecModel::from(output_monitor_rows(shell, name, &current))).into(),
            }
        })
        .collect()
}

/// Detected-then-configured output names, deduplicated.
fn output_names(shell: &Shell) -> Vec<String> {
    let main = shell.includes.docs.len();
    let mut names = outputs::configured(doc_at(shell, main));
    for monitor in &shell.guide_monitors {
        if !names.contains(&monitor.name) {
            names.push(monitor.name.clone());
        }
    }
    names
}

/// One monitor's field rows; with detected modes the single mode string
/// splits into resolution and refresh dropdowns.
fn output_monitor_rows(
    shell: &Shell,
    name: &str,
    current: &BTreeMap<String, String>,
) -> Vec<SettingRow> {
    let monitor = shell
        .guide_monitors
        .iter()
        .find(|monitor| monitor.name == name);
    let mut rows: Vec<SettingRow> = Vec::new();
    for field in outputs::FIELDS {
        if field.key == "mode"
            && let Some(detected) = monitor.filter(|monitor| !monitor.modes.is_empty())
        {
            rows.push(resolution_choice_row(shell, name, detected, current));
            rows.push(refresh_choice_row(shell, name, detected, current));
            continue;
        }
        rows.push(output_row(shell, name, field, current));
    }
    let home_label = setting_labels(shell)
        .get(shell.includes.docs.len())
        .cloned()
        .unwrap_or_default();
    for row in &mut rows {
        row.home_label = home_label.clone();
    }
    rows
}

/// The outputs page model: one card per monitor with its live info.
fn monitor_cards(shell: &Shell) -> Vec<MonitorCard> {
    let main = shell.includes.docs.len();
    let current: BTreeMap<String, String> = doc_at(shell, main).leaf_values().into_iter().collect();
    let configured = outputs::configured(doc_at(shell, main));
    let removable = configured.len() > 1;
    output_names(shell)
        .iter()
        .map(|name| {
            let monitor = shell
                .guide_monitors
                .iter()
                .find(|monitor| monitor.name == *name);
            let mut info: Vec<String> = Vec::new();
            if let Some(monitor) = monitor {
                if !monitor.description.is_empty() {
                    info.push(monitor.description.clone());
                }
                if let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index)) {
                    info.push(format!("Currently {}", mode.label()));
                }
            }
            MonitorCard {
                name: name.clone().into(),
                info: info.join(" · ").into(),
                connected: monitor.is_some(),
                configured: configured.contains(name),
                expanded: card_expanded(shell, &format!("monitor:{name}"), true),
                removable,
                rows: Rc::new(VecModel::from(output_monitor_rows(shell, name, &current))).into(),
            }
        })
        .collect()
}

/// A card's expansion: the user's choice wins, else the default
/// (settings and monitor cards open, rule cards open only when alone).
fn card_expanded(shell: &Shell, key: &str, default: bool) -> bool {
    shell.card_expanded.get(key).copied().unwrap_or(default)
}

/// Rescan the compositor's monitors; failures become the page's note.
fn scan_outputs(shell: &mut Shell) {
    match live::outputs() {
        Ok(list) => {
            shell.guide_monitors = list;
            shell.live_note = String::new();
        }
        Err(err) => shell.live_note = format!("live state unavailable: {err}"),
    }
}

fn rebuild_outputs(app: &AppWindow, shell: &Shell) {
    app.set_monitors(Rc::new(VecModel::from(monitor_cards(shell))).into());
    app.set_outputs_live_note(shell.live_note.clone().into());
    app.set_changed_count(changed_count(shell));
}

/// Which rule family a section page edits, if any.
fn rule_family(section: &str) -> Option<&'static str> {
    match section {
        "window-rules" => Some("window_rule"),
        "layer-rules" => Some("layer_rule"),
        "security-contexts" => Some("security_context_rule"),
        _ => None,
    }
}

/// The chain document that owns a rule family: the first include with
/// rules of that family in it, else the main config.
fn rule_target(shell: &Shell, family: &str) -> usize {
    let main = shell.includes.docs.len();
    shell
        .includes
        .docs
        .iter()
        .position(|inc| inc.doc.rule_count(family) > 0)
        .unwrap_or(main)
}

fn doc_at_mut(shell: &mut Shell, file_index: usize) -> &mut ConfigDocument {
    if file_index == shell.includes.docs.len() {
        &mut shell.doc
    } else {
        &mut shell.includes.docs[file_index].doc
    }
}

/// One rule field as a settings row. Rules have no defaults: unset
/// fields show blank, choices offer "(unset)", and only what the user
/// fills in is written. The key carries the chain document so edits
/// land where the rule lives; the badge opens that file.
fn rule_row(
    shell: &Shell,
    doc_index: usize,
    family: &str,
    index: usize,
    field: &rules::Field,
) -> SettingRow {
    let doc = doc_at(shell, doc_index);
    let mut row = blank_row(
        rules::rule_key(family, doc_index, index, field.key),
        field.label,
        doc_index as i32,
    );
    row.home_label = setting_labels(shell)
        .get(doc_index)
        .cloned()
        .unwrap_or_default();
    let text = rules::field_text(doc, family, index, field);
    match &field.kind {
        rules::FieldKind::Text
        | rules::FieldKind::List
        | rules::FieldKind::Size
        | rules::FieldKind::Position => {
            row.value = text.into();
        }
        rules::FieldKind::Toggle => {
            row.kind = ValueKind::Boolean;
            row.checked = text == "true";
            row.value = text.into();
        }
        rules::FieldKind::Choice(options) => {
            row.kind = ValueKind::Choice;
            let mut choices: Vec<SharedString> = vec!["(unset)".into()];
            choices.extend(options.iter().map(|option| (*option).into()));
            row.choices = Rc::new(VecModel::from(choices)).into();
            row.value = if text.is_empty() {
                "(unset)".into()
            } else {
                text.into()
            };
        }
        rules::FieldKind::Float { min, max } => {
            row.kind = ValueKind::Float;
            row.min = *min as f32;
            row.max = *max as f32;
            row.value = if text.is_empty() {
                min.to_string().into()
            } else {
                text.into()
            };
        }
        rules::FieldKind::Integer { min, max } => {
            row.kind = ValueKind::Integer;
            row.min = *min as f32;
            row.max = *max as f32;
            row.value = if text.is_empty() {
                min.to_string().into()
            } else {
                text.into()
            };
        }
    }
    row
}

/// The rule page model: every chain document's rules of the family,
/// includes first then the main config — all editable in place.
fn rule_cards(shell: &Shell, family: &str) -> Vec<RuleCard> {
    let main = shell.includes.docs.len();
    let labels = setting_labels(shell);
    let (match_fields, setting_fields) = rules::fields(family);
    let mut cards: Vec<RuleCard> = Vec::new();
    for doc_index in 0..=main {
        let doc = doc_at(shell, doc_index);
        for index in 0..doc.rule_count(family) {
            let rows = |fields: &[rules::Field]| {
                fields
                    .iter()
                    .map(|field| rule_row(shell, doc_index, family, index, field))
                    .collect::<Vec<_>>()
            };
            cards.push(RuleCard {
                title: rules::rule_title(doc, family, index, match_fields).into(),
                index: index as i32,
                file: doc_index as i32,
                file_label: labels.get(doc_index).cloned().unwrap_or_default(),
                expanded: false,
                match_rows: Rc::new(VecModel::from(rows(match_fields))).into(),
                setting_rows: Rc::new(VecModel::from(rows(setting_fields))).into(),
            });
        }
    }
    // A lone rule opens by default; otherwise the titles read like a
    // collapsed list until a card is expanded.
    let default = cards.len() == 1;
    let family_key = family.to_owned();
    for (position, card) in cards.iter_mut().enumerate() {
        let key = format!("rule:{family_key}:{}:{}", card.file, card.index);
        card.expanded = card_expanded(shell, &key, default && position == 0);
    }
    cards
}

/// New rules join the file where that family already has the most
/// rules — where the user keeps them; a tie (or no rules anywhere)
/// goes to the main config.
fn rule_add_target(shell: &Shell, family: &str) -> usize {
    let main = shell.includes.docs.len();
    (0..=main)
        .max_by_key(|doc_index| doc_at(shell, *doc_index).rule_count(family))
        .unwrap_or(main)
}

fn rebuild_rule_page(app: &AppWindow, shell: &Shell) {
    let section = app.get_current_section().to_string();
    let Some(family) = rule_family(&section) else {
        return;
    };
    app.set_rule_cards(Rc::new(VecModel::from(rule_cards(shell, family))).into());
    app.set_changed_count(changed_count(shell));
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
) -> SettingRow {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let (mut resolution, _) = mode_parts(doc, name);
    if resolution.is_empty()
        && let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index))
    {
        resolution = format!("{}x{}", mode.width, mode.height);
    }
    let key = format!("output.{name}.resolution");
    let mut row = blank_row(key.clone(), "Resolution", shell.includes.docs.len() as i32);
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
) -> SettingRow {
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
    let mut row = blank_row(
        key.clone(),
        "Refresh rate",
        shell.includes.docs.len() as i32,
    );
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
fn blank_row(key: String, label: &str, home: i32) -> SettingRow {
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
) -> SettingRow {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let path = ["output", name, field.key];
    let key = format!("output.{name}.{}", field.key);
    let mut row = blank_row(key.clone(), field.label, shell.includes.docs.len() as i32);
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
        outputs::FieldKind::Text => {
            row.value = doc
                .get_string(&path)
                .filter(|value| !value.is_empty())
                .unwrap_or(default_text)
                .into();
        }
        outputs::FieldKind::Workspaces => {
            // The config accepts a count, a name list, or "dynamic" —
            // a plain string read misses the arrays people actually
            // have, so read all three shapes.
            row.value = workspaces_text(doc, &path)
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
        outputs::FieldKind::Choice(_) | outputs::FieldKind::Text => {
            commit_value(Some(&schema::Kind::Text), raw)
        }
        // A count stays a number, "dynamic" stays literal, names become
        // a TOML array — set_leaf_text parses the text as TOML, so the
        // array form keeps umbriel's list shape.
        outputs::FieldKind::Workspaces => match workspaces_parse(raw)? {
            Some(text) => Ok(text),
            None => {
                Err("workspaces: use comma-separated names, a count, or \"dynamic\"".to_owned())
            }
        },
    }
}

/// Editor text for an output's workspaces: a bare count, comma-joined
/// names, or the literal string.
fn workspaces_text(doc: &ConfigDocument, path: &[&str]) -> Option<String> {
    if let Some(count) = doc.get_integer(path) {
        return Some(count.to_string());
    }
    if let Some(names) = doc.get_strings(path) {
        return Some(names.join(", "));
    }
    doc.get_string(path)
}

/// Parse editor input into the TOML text `set_leaf_text` writes; `None`
/// when nothing sensible remains (empty input).
fn workspaces_parse(raw: &str) -> Result<Option<String>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("workspaces can't be empty — use names, a count, or \"dynamic\"".to_owned());
    }
    if let Ok(count) = trimmed.parse::<i64>() {
        return Ok(Some(count.to_string()));
    }
    if trimmed == "dynamic" {
        return Ok(Some("\"dynamic\"".to_owned()));
    }
    let names: Vec<&str> = trimmed
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return Err("workspaces can't be empty — use names, a count, or \"dynamic\"".to_owned());
    }
    let listed: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
    Ok(Some(format!("[{}]", listed.join(", "))))
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
    fn workspaces_text_reads_count_list_and_literal() {
        let doc = ConfigDocument::from_str(
            "[output.\"DP-3\"]\nworkspaces = [\"Games\", \"Steam\"]\n\n[output.\"eDP-1\"]\nworkspaces = 4\n\n[output.\"HDMI-1\"]\nworkspaces = \"dynamic\"\n",
        )
        .unwrap();
        assert_eq!(
            workspaces_text(&doc, &["output", "DP-3", "workspaces"]).unwrap(),
            "Games, Steam"
        );
        assert_eq!(
            workspaces_text(&doc, &["output", "eDP-1", "workspaces"]).unwrap(),
            "4"
        );
        assert_eq!(
            workspaces_text(&doc, &["output", "HDMI-1", "workspaces"]).unwrap(),
            "dynamic"
        );
        assert_eq!(
            workspaces_text(&doc, &["output", "none", "workspaces"]),
            None
        );
    }

    #[test]
    fn workspaces_parse_emits_toml_set_leaf_text_accepts() {
        // Names become a TOML array the document can parse back.
        let formatted = workspaces_parse("Games, Steam , Util").unwrap().unwrap();
        assert_eq!(formatted, "[\"Games\", \"Steam\", \"Util\"]");
        let mut doc = ConfigDocument::from_str("[output.\"DP-3\"]\n").unwrap();
        assert!(doc.set_leaf_text("output.DP-3.workspaces", &formatted));
        assert_eq!(
            workspaces_text(&doc, &["output", "DP-3", "workspaces"]).unwrap(),
            "Games, Steam, Util"
        );
        // A count stays a number, the literal stays a string.
        assert_eq!(workspaces_parse("4").unwrap().unwrap(), "4");
        assert_eq!(workspaces_parse("dynamic").unwrap().unwrap(), "\"dynamic\"");
        // Empty and comma-only input are rejected, not written.
        assert!(workspaces_parse("").is_err());
        assert!(workspaces_parse(" , ").is_err());
    }

    #[test]
    fn rule_card_expansion_defaults_and_persists() {
        let dir = std::env::temp_dir().join(format!("umbriel-expand-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("config.toml");
        std::fs::write(&main_path, "[[window_rule]]\nmatch.app_id = \"kitty\"\n").unwrap();
        let env = discovery::Env::from_process();

        // A lone rule card opens by default.
        let mut shell = Shell::load(&main_path, &env);
        assert!(rule_cards(&shell, "window_rule")[0].expanded);

        // A second rule collapses the list — and the user's choice
        // survives the rebuild that every edit triggers.
        shell.doc.add_rule("window_rule");
        let cards = rule_cards(&shell, "window_rule");
        assert!(cards.iter().all(|card| !card.expanded));
        let key = format!("rule:window_rule:{}:{}", cards[0].file, cards[0].index);
        shell.card_expanded.insert(key, true);
        assert!(rule_cards(&shell, "window_rule")[0].expanded);
        assert!(!rule_cards(&shell, "window_rule")[1].expanded);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rule_cards_collect_every_chain_document() {
        let dir = std::env::temp_dir().join(format!("umbriel-rules-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("config.toml");
        let inc_path = dir.join("windowrules.toml");
        std::fs::write(
            &inc_path,
            "[[window_rule]]\nmatch.app_id = \"steam\"\n\n[[window_rule]]\nmatch.app_id = \"discord\"\n",
        )
        .unwrap();
        std::fs::write(
            &main_path,
            "[include]\nfiles = [\"windowrules.toml\"]\n\n[[window_rule]]\ndefault_floating = true\n",
        )
        .unwrap();
        let env = discovery::Env::from_process();
        let shell = Shell::load(&main_path, &env);
        let cards = rule_cards(&shell, "window_rule");
        // Include rules first, then the main config's own rule.
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].title, "app_id = steam");
        assert_eq!(cards[0].file, 0);
        assert_eq!(cards[2].file, 1);
        assert!(cards[2].file_label.contains("config.toml"));
        // Editing a card's row writes through the doc-qualified key.
        let key = rules::rule_key("window_rule", 0, 0, "default_floating");
        assert_eq!(key, "window_rule[0:0].default_floating");
        assert_eq!(
            rules::parse_rule_key(&key),
            Some(("window_rule", Some(0), 0, "default_floating"))
        );
        // New rules join the file holding the most rules of the family.
        assert_eq!(rule_add_target(&shell, "window_rule"), 0);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn starter_config_loads_as_empty_healthy_document() {
        let doc = ConfigDocument::from_str(STARTER_CONFIG).expect("starter parses");
        assert!(doc.leaf_values().is_empty());
        assert!(!doc.is_modified());
    }
}
