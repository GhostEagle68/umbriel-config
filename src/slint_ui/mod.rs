//! Slint shell for umbriel-config. The UI-agnostic library does all real
//! work — this tree only presents it and forwards intents.
//!
//! Module map:
//! - `mod.rs` (this file): the shared [`Shell`] state and `run()`, which
//!   wires every feature's `install_*` callbacks and starts the loop.
//! - `common`: chain-document access, saved-state diffing, ownership
//!   lookups, small display utilities.
//! - `rows`: schema entries → `SettingRow` rendering and the shared
//!   value-commit write path.
//! - `sections`: catalog-driven settings pages, sidebar, navigation.
//! - `save`: the save popup and the write-all + validate step.
//! - `search`: the header search across settings, pages, and keybinds.
//! - `page_*`: one module per dedicated page (keybinds, outputs, rules,
//!   shaders, backups, settings) — its callbacks and its rebuild fns.
//! - `guide`: first-run onboarding and the guided setup walk.
//! - `catalog`: the curated page/card metadata; `shader_preview`: the
//!   offscreen GLSL preview worker.

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
    settings as app_settings, shaders, state, validate,
};

use umbriel_config::{changelog, live, update};

mod catalog;
mod common;
mod guide;
mod page_backups;
mod page_keybinds;
mod page_outputs;
mod page_rules;
mod page_settings;
mod page_shaders;
mod rows;
mod save;
mod search;
mod sections;
mod shader_preview;

slint::include_modules!();

/// Central UI state shared by every page module: the loaded chain, the
/// schema projection, per-page caches, and the editor's worker handle.
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
    guide: Option<guide::Guide>,
    // Monitors detected when the guide was armed: feeds the outputs
    // card's resolution/refresh dropdowns and its recommendation hints.
    guide_monitors: Vec<live::LiveOutput>,
    // Last live-scan failure note; empty when detection is healthy.
    live_note: String,
    // Collapsed/expanded cards by stable key; absent = page default.
    card_expanded: BTreeMap<String, bool>,
    // Discovered GLSL shaders + the last download/scan note.
    shaders: Vec<shaders::ShaderEntry>,
    shader_note: String,
    // The community collection exists on disk (button flips to Update).
    shaders_installed: bool,
    // The upstream commit check ran this session (once per launch).
    shaders_update_checked: bool,
    // The shader being edited in the overlay editor; None = creating new.
    shader_editing: Option<PathBuf>,
    // The effect builder's step stack; mirrors the code pane whenever
    // the builder can read it (the window's builder-locked flag).
    builder_steps: Vec<shaders::builder::BuilderStep>,
    // Offscreen preview worker; started on first editor use. After an
    // init failure it stays off until the next app run (best-effort).
    shader_preview: Option<shader_preview::PreviewHandle>,
    shader_preview_failed: bool,
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
        // Startup drift: keys added since the last snapshot get NEW badges.
        // No snapshot yet (first run) flags nothing.
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
            shaders: Vec::new(),
            shader_note: String::new(),
            shaders_installed: false,
            shaders_update_checked: false,
            shader_editing: None,
            builder_steps: Vec::new(),
            shader_preview: None,
            shader_preview_failed: false,
        };
        shell.reset_saved();
        shell
    }

    /// Re-capture the on-disk baselines; a save resets the "changed" marks.
    fn reset_saved(&mut self) {
        let main = self.includes.docs.len();
        self.saved = (0..=main)
            .map(|i| common::doc_at(self, i).leaf_values().into_iter().collect())
            .collect();
    }

    fn any_modified(&self) -> bool {
        self.doc.is_modified() || self.includes.docs.iter().any(|inc| inc.doc.is_modified())
    }

    /// Send a command to the preview worker, starting it on first use.
    fn preview_command(&mut self, cmd: shader_preview::PreviewCommand) {
        if self.shader_preview_failed {
            return;
        }
        if self.shader_preview.is_none() {
            self.shader_preview = Some(shader_preview::PreviewHandle::spawn());
        }
        if let Some(handle) = &self.shader_preview {
            handle.send(cmd);
        }
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
    // Backup run history backing the Backups page dropdown.
    let backup_runs: Rc<RefCell<Vec<backups::RunInfo>>> = Rc::new(RefCell::new(Vec::new()));

    let app = AppWindow::new().map_err(|err| anyhow::anyhow!("window creation failed: {err}"))?;

    {
        let shell = shell.borrow();
        app.set_config_path(common::pretty_path(&shell.path, &env).into());
        common::refresh_include_files(&app, &shell, &env);
        app.set_include_note(shell.includes.notes.join("; ").into());
    }
    app.set_dirty(false);
    app.set_app_version(env!("CARGO_PKG_VERSION").into());
    app.set_check_updates_on_start(settings.check_updates_on_start);
    app.set_update_prereleases(settings.prereleases);
    app.set_dark_mode(settings.dark);
    app.global::<Theme>().set_dark(settings.dark);
    app.set_backup_note(page_backups::backup_note(&settings, &env));
    app.set_backup_count_text(settings.backup_count.to_string().into());
    app.set_backup_dir_text(settings.backup_dir.clone().unwrap_or_default().into());
    {
        let names: Vec<slint::SharedString> = {
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
                    let names: Vec<slint::SharedString> =
                        actions.iter().map(|a| a.name.clone().into()).collect();
                    if let Some(app) = weak.upgrade() {
                        app.set_action_choices(Rc::new(VecModel::from(names)).into());
                        *kb_actions.lock().expect("kb actions") = actions;
                    }
                });
            }
        });
    }

    // First-run state: a machine without a config gets the onboarding
    // panel — with the guided walk when umbriel is present. Umbriel
    // missing additionally arms the quiet banner + empty state.
    let umbriel_present = discovery::packaged_default(&env).is_some();
    let mode = guide::setup_mode(umbriel_present, path.exists());
    app.set_show_onboarding(matches!(
        mode,
        guide::SetupMode::PlainInstall | guide::SetupMode::FreshWithUmbriel
    ));
    app.set_umbriel_missing(!umbriel_present);
    app.set_schema_empty(shell.borrow().schema.is_empty());

    let nav = sections::section_nav(&shell.borrow());
    let configured_outputs = !outputs::configured(&shell.borrow().doc).is_empty();
    let landing = nav
        .iter()
        .find(|entry| !entry.is_header && (configured_outputs || entry.id != catalog::OUTPUTS_ID));
    if let Some(first) = landing {
        let (title, description) = sections::page_meta(&first.id);
        app.set_current_section(first.id.clone());
        app.set_page_title(title.into());
        app.set_page_description(description.into());
        sections::refill_page(&app, &shell.borrow(), &first.id);
    }
    app.set_sections(Rc::new(VecModel::from(nav)).into());
    {
        // Destination picker model, main first: index 0 = main, i = include i-1.
        let shell = shell.borrow();
        let labels = common::setting_labels(&shell);
        let main = labels.len() - 1;
        let mut destinations = vec![labels[main].clone()];
        destinations.extend(labels[..main].iter().cloned());
        app.set_destinations(Rc::new(VecModel::from(destinations)).into());
    }
    {
        // Shader builder: the palette of step kinds the Add-an-effect
        // dropdown offers.
        let kinds: Vec<slint::SharedString> = shaders::builder::STEP_DEFS
            .iter()
            .map(|def| slint::SharedString::from(def.label))
            .collect();
        app.set_shader_step_kinds(Rc::new(VecModel::from(kinds)).into());
    }

    // What's new: the bundled changelog section for the running version,
    // once per version, and only where an update makes sense (a fresh
    // install has nothing "new" yet).
    if matches!(
        mode,
        guide::SetupMode::Normal | guide::SetupMode::MissingUmbriel
    ) {
        let sections = changelog::parse(changelog::bundled());
        // The channels/in-app-update notice rides along once, ever — and
        // on its own if this version's changelog was already seen.
        let notice = update::notice_should_show(&env);
        if notice {
            app.set_whatsnew_notice(
                "Updates work differently now. You're on the Pre-release channel, \
                 which gets new builds from the dev branch first. Prefer tested \
                 releases? Switch to Stable in Settings → Updates. If you installed \
                 from the release tarball, updates can now be installed from the app; \
                 cargo, AUR and source builds show the command to run instead."
                    .into(),
            );
            update::notice_mark_shown(&env);
        }
        if changelog::should_show(&env, env!("CARGO_PKG_VERSION"))
            && let Some(section) = changelog::for_version(&sections, env!("CARGO_PKG_VERSION"))
        {
            app.set_whatsnew_title(format!("What's new in {}", env!("CARGO_PKG_VERSION")).into());
            app.set_whatsnew_body(changelog::renderable(&section.body).into());
            app.set_show_whatsnew(true);
            changelog::mark_shown(&env, env!("CARGO_PKG_VERSION"));
        } else if notice {
            app.set_whatsnew_title("Updates work differently now".into());
            app.set_show_whatsnew(true);
        }
    }

    app.window().set_size(WindowSize::Logical(LogicalSize::new(
        settings.window_width as f32,
        settings.window_height as f32,
    )));

    // Every feature registers its own callbacks.
    sections::install_navigation(&app, &shell, &keybind_binds, &kb_actions);
    search::install_search(&app, &shell, &kb_actions, &keybind_binds);
    save::install_save(&app, &shell, &env);
    page_settings::install_settings(&app, &shell, &env);
    page_backups::install_backups(&app, &shell, &env, &backup_runs);
    page_outputs::install_outputs(&app, &shell);
    page_rules::install_rules(&app, &shell);
    page_shaders::install_shaders(&app, &shell);
    page_keybinds::install_keybinds(&app, &shell, &kb_actions, &keybind_binds);
    guide::install_guide(&app, &shell, &env);
    rows::install_value_editing(&app, &shell);
    rows::install_color_math(&app);

    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_exit_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            page_settings::store_window_settings(&app, &env);
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
                page_settings::store_window_settings(&app, &env);
                CloseRequestResponse::HideWindow
            }
        });
    }

    // Pump preview events off the worker at scrub-friendly latency; an
    // idle tick is one failed try_recv, so running forever is free.
    let preview_timer = slint::Timer::default();
    {
        let weak = app.as_weak();
        let shell = Rc::clone(&shell);
        preview_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(33),
            move || {
                if let Some(app) = weak.upgrade() {
                    page_shaders::poll_shader_preview(&app, &shell);
                }
            },
        );
    }

    if settings.check_updates_on_start && update::should_auto_check(&env) {
        page_settings::start_update_check(app.as_weak(), Some(env.clone()));
    }

    app.run()
        .map_err(|err| anyhow::anyhow!("event loop failed: {err}"))?;
    Ok(())
}
