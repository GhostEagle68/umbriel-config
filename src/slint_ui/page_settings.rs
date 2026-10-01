//! The Settings page: app preferences, the update check, the upstream
//! commit line, the file viewer, and the changelog/update-notes overlays.

use super::common::*;
use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Download umbriel's docs on a worker thread, then hand the result to
/// `schema-source-updated` (an empty string means it worked).
pub(super) fn start_docs_download(weak: slint::Weak<AppWindow>, env: discovery::Env) {
    std::thread::spawn(move || {
        let error = umbriel_docs::refresh(&env).err().unwrap_or_default();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                app.invoke_schema_source_updated(error.into());
            }
        });
    });
}

/// The toggle is read from the live property so a change made on the
/// Settings page survives exit.
pub(super) fn store_window_settings(app: &AppWindow, env: &discovery::Env) {
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
/// once-a-day stamp.
pub(super) fn start_update_check(weak: slint::Weak<AppWindow>, env: Option<discovery::Env>) {
    let Some(app) = weak.upgrade() else { return };
    // A build from a checkout carries no release or canary commit to
    // compare, so every release would look new.
    if update::current_install_kind() == update::InstallKind::Source {
        app.set_update_note(
            "Development build, running from the source tree: no update checks.".into(),
        );
        return;
    }
    app.set_update_note("Checking…".into());
    // Read the channel here: the worker can't touch the window.
    let channel = app_settings::Channel::at(app.get_update_channel());
    // Only the startup check may install on its own.
    let automatic = env.is_some();
    std::thread::spawn(move || {
        let result = update::check(channel);
        if result.is_ok()
            && let Some(env) = env.as_ref()
        {
            update::mark_checked(env);
        }
        let _ = slint::invoke_from_event_loop(move || {
            let Some(app) = weak.upgrade() else { return };
            match result {
                Ok(update::Verdict::UpToDate) => app.set_update_note("Up to date.".into()),
                Ok(update::Verdict::NoRelease) => app.set_update_note(
                    "No stable release yet. You'll get the first one when it ships.".into(),
                ),
                Ok(update::Verdict::UpdateAvailable { version, notes }) => {
                    app.set_update_available(true);
                    let note = match channel {
                        app_settings::Channel::Stable => format!("Version {version} available."),
                        app_settings::Channel::Canary => format!("New build: {version}."),
                    };
                    app.set_update_note(note.into());
                    // Who owns this binary decides whether the app may
                    // install the update or only name the command.
                    let kind = update::current_install_kind();
                    app.set_update_can_install(kind == update::InstallKind::Tarball);
                    app.set_update_hint(update::update_hint(kind, channel, &version).into());
                    app.set_update_version(version.into());
                    if let Some(notes) = notes {
                        app.set_update_notes(notes.into());
                    }
                    if automatic
                        && channel == app_settings::Channel::Canary
                        && app.get_canary_auto_install()
                        && kind == update::InstallKind::Tarball
                    {
                        app.invoke_install_update();
                    } else if automatic {
                        let action = if kind == update::InstallKind::Tarball {
                            "Install"
                        } else {
                            "Details"
                        };
                        toast(&app, ToastKind::Info, app.get_update_note(), action);
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
    let Some((sha, message, date)) = github_latest_commit(
        "https://api.github.com/repos/noctalia-dev/umbriel/commits?per_page=1",
    ) else {
        return Err("no commits found".to_owned());
    };
    let sha: String = sha.chars().take(7).collect();
    let date = date.get(..10).unwrap_or_default();
    Ok(format!("{sha} • {date} • {message}"))
}

/// Fetch the compositor's latest commit into the Settings and Home
/// pages, once per run; the "Checking…" note doubles as the in-flight
/// guard.
pub(super) fn fetch_upstream_note(app: &AppWindow) {
    if !app.get_upstream_note().is_empty() {
        return;
    }
    let weak = app.as_weak();
    app.set_upstream_note("Checking…".into());
    std::thread::spawn(move || {
        let result = fetch_latest_commit();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                let note = match result {
                    Ok(note) => note,
                    Err(err) => format!("Couldn't fetch the latest commit: {err}"),
                };
                app.set_upstream_note(note.into());
            }
        });
    });
}

pub(super) fn install_settings(app: &AppWindow, shell: &Rc<RefCell<Shell>>, env: &discovery::Env) {
    {
        let weak = app.as_weak();
        app.on_settings_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_page(Page::Settings);
            fetch_upstream_note(&app);
        });
    }
    {
        let weak = app.as_weak();
        app.on_check_updates_requested(move || {
            start_update_check(weak.clone(), None);
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_check_updates_toggled(move |checked| {
            let Some(app) = weak.upgrade() else { return };
            let mut settings = app_settings::load(&env);
            settings.check_updates_on_start = checked;
            store_settings(&app, &env, &settings);
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        // Two installs at once would race on the same staged file.
        let installing = Arc::new(AtomicBool::new(false));
        app.on_install_update(move || {
            let Some(app) = weak.upgrade() else { return };
            let version = app.get_update_version().to_string();
            if version.is_empty() || installing.swap(true, Ordering::Relaxed) {
                return;
            }
            let tag = match app_settings::Channel::at(app.get_update_channel()) {
                app_settings::Channel::Canary => "canary".to_owned(),
                _ => format!("v{version}"),
            };
            app.set_update_note("Installing…".into());
            let weak = app.as_weak();
            let installing = Arc::clone(&installing);
            let env = env.clone();
            std::thread::spawn(move || {
                let result = update::install(&tag);
                let _ = slint::invoke_from_event_loop(move || {
                    installing.store(false, Ordering::Relaxed);
                    let Some(app) = weak.upgrade() else { return };
                    match result {
                        Ok(()) => {
                            app.set_update_installed(true);
                            let note = format!("Installed {version}. Restart to use it.");
                            app.set_update_note(note.clone().into());
                            toast(&app, ToastKind::Success, note, "Restart");
                            if let Some(sha) = version.strip_prefix("canary ") {
                                // Every canary shares a version, so the new
                                // build shows these instead of the changelog.
                                update::save_canary_notes(&env, sha, &app.get_update_notes());
                            }
                        }
                        Err(err) => {
                            app.set_update_note(format!("Install failed: {err}").into());
                            alert(&app, "Update failed", err.to_string());
                        }
                    }
                });
            });
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        // Read before an update replaces the binary: afterwards /proc/self/exe
        // points at the "(deleted)" old inode, which can't be launched.
        let exe = std::env::current_exe();
        app.on_restart_app(move || {
            let Some(app) = weak.upgrade() else { return };
            // The new binary is already on disk; unsaved edits would be
            // lost with the process, so they stop the restart.
            if app.get_dirty() {
                toast(
                    &app,
                    ToastKind::Warning,
                    "Save or discard your changes before restarting.",
                    "",
                );
                return;
            }
            let Ok(exe) = &exe else {
                toast(
                    &app,
                    ToastKind::Warning,
                    "Restart manually to use the new version.",
                    "",
                );
                return;
            };
            match std::process::Command::new(exe)
                .args(std::env::args().skip(1))
                .spawn()
            {
                Ok(_) => {
                    // Same order as the Exit path: persist, then quit.
                    store_window_settings(&app, &env);
                    let _ = slint::quit_event_loop();
                }
                Err(err) => alert(&app, "Couldn't restart", err.to_string()),
            }
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_canary_auto_install_toggled({
            let weak = weak.clone();
            let env = env.clone();
            move |checked| {
                let Some(app) = weak.upgrade() else { return };
                let mut settings = app_settings::load(&env);
                settings.canary_auto_install = checked;
                store_settings(&app, &env, &settings);
            }
        });
        app.on_update_channel_selected(move |index| {
            let Some(app) = weak.upgrade() else { return };
            app.set_update_channel(index);
            let mut settings = app_settings::load(&env);
            settings.channel = app_settings::Channel::at(index);
            store_settings(&app, &env, &settings);
            start_update_check(app.as_weak(), None);
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
            store_settings(&app, &env, &settings);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_view_file(move |index, key| {
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
            let text = doc.text();
            let lines: Vec<CodeLine> = text
                .lines()
                .enumerate()
                .map(|(number, line)| {
                    let spans: Vec<CodeSpan> = toml_lines::highlight(line)
                        .into_iter()
                        .map(|(text, token)| CodeSpan {
                            text: text.into(),
                            kind: token as i32,
                        })
                        .collect();
                    CodeLine {
                        number: number as i32 + 1,
                        spans: Rc::new(VecModel::from(spans)).into(),
                    }
                })
                .collect();
            // A rule row's key names its document too; the viewer
            // already shows that file.
            let key = match rules::parse_rule_key(&key) {
                Some((family, _, index, field)) => format!("{family}[{index}].{field}"),
                None => key.to_string(),
            };
            let focus = (!key.is_empty())
                .then(|| toml_lines::key_line(&text, &key))
                .flatten();
            app.set_popup_file_lines(Rc::new(VecModel::from(lines)).into());
            app.set_popup_focus_line(focus.map_or(-1, |line| line as i32));
            app.set_file_stats(status_line(&shell));
            app.set_show_file_popup(true);
        });
    }
    {
        // Sync: download umbriel's newest docs, then rebuild from them.
        let weak = app.as_weak();
        let env = env.clone();
        app.on_sync_schema_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_sync_note("Downloading umbriel's docs…".into());
            app.set_sync_clean(true);
            start_docs_download(app.as_weak(), env.clone());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let env = env.clone();
        app.on_schema_source_updated(move |error| {
            let Some(app) = weak.upgrade() else { return };
            // Rebuild from the docs, diff old vs fresh, store the fresh
            // snapshot; the added keys get badges.
            let super::Loaded {
                schema: fresh,
                rule_families,
                output_fields,
                schema_source: source,
                shader_api,
            } = super::load_schema(&env);
            let fresh_set = schema::key_set(&fresh);
            let mut shell = shell.borrow_mut();
            let drift = schema::diff(&schema::key_set(&shell.schema), &fresh_set);
            store_snapshot(&app, &env, &fresh_set);
            shell.new_keys.extend(drift.added.iter().cloned());
            store_new_keys(&app, &shell, &env);
            let (note, clean) = match (&source, error.is_empty(), drift.is_empty()) {
                // umbriel's schema decides the keys; the docs only add
                // labels and units, so their refresh reads as that.
                (Some(_), false, _) => (
                    format!(
                        "{} Couldn't refresh labels from umbriel's docs ({error}).",
                        super::schema_note(source.as_deref())
                    ),
                    false,
                ),
                (Some(_), true, true) => (
                    format!(
                        "{} Labels are up to date with umbriel's docs.",
                        super::schema_note(source.as_deref())
                    ),
                    true,
                ),
                (Some(_), true, false) => (
                    format!(
                        "{} Changed: {}.",
                        super::schema_note(source.as_deref()),
                        drift.summary()
                    ),
                    false,
                ),
                (None, false, _) => (
                    format!("Couldn't download umbriel's docs ({error}); using the saved copy."),
                    false,
                ),
                (None, true, true) => (
                    "Settings are up to date with umbriel's docs.".to_owned(),
                    true,
                ),
                (None, true, false) => (
                    format!("Synced from umbriel's docs: {}.", drift.summary()),
                    false,
                ),
            };
            shell.schema = fresh;
            shell.rule_families = rule_families;
            shell.output_fields = output_fields;
            shell.schema_source = source;
            shell.shader_api = shader_api;
            app.set_sync_note(note.clone().into());
            app.set_sync_clean(clean);
            if !drift.added.is_empty() {
                let count = drift.added.len();
                let plural = if count == 1 { "" } else { "s" };
                toast(
                    &app,
                    ToastKind::Info,
                    format!("{count} new setting{plural} synced from umbriel's docs."),
                    "Review",
                );
            } else if !drift.is_empty() || !error.is_empty() {
                toast(&app, ToastKind::Info, note, "");
            }
            app.set_schema_empty(shell.schema.is_empty());
            app.set_sections(
                Rc::new(VecModel::from(super::sections::section_nav(
                    &shell,
                    &app.get_current_section(),
                )))
                .into(),
            );
            super::sections::refresh_shown_page(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_view_changelog(move || {
            let Some(app) = weak.upgrade() else { return };
            // The canary updates you installed, each with what it added,
            // above the releases.
            let canaries = update::canary_notes(&env)
                .into_iter()
                .map(|(sha, notes)| notes_card(format!("Canary {sha}"), "", "Canary", &notes));
            let sections = changelog::parse(changelog::bundled());
            // A canary build's changes since the last release are its
            // commits, which GitHub lists.
            if let (Some(sha), Some(release)) = (update::BUILD_SHA, sections.first()) {
                app.set_whatsnew_canary_url(
                    format!(
                        "https://github.com/GhostEagle68/umbriel-config/compare/v{}...{sha}",
                        release.version
                    )
                    .into(),
                );
            }
            show_notes(
                &app,
                "Changelog",
                canaries.chain(sections.iter().map(release_card)).collect(),
                true,
            );
        });
    }
    {
        let weak = app.as_weak();
        app.on_view_update_notes(move || {
            let Some(app) = weak.upgrade() else { return };
            // The fetched notes are the new release's body; fall back to
            // the bundled section when the check hasn't run.
            let notes = app.get_update_notes().to_string();
            let card = if notes.is_empty() {
                let sections = changelog::parse(changelog::bundled());
                changelog::for_version(&sections, env!("CARGO_PKG_VERSION")).map(release_card)
            } else {
                let version = app.get_update_version().to_string();
                Some(match version.strip_prefix("canary ") {
                    Some(sha) => notes_card(format!("Canary {sha}"), "", "Canary", &notes),
                    None => notes_card(
                        format!("Version {version}"),
                        "",
                        version_badge(&version),
                        &notes,
                    ),
                })
            };
            show_notes(&app, "What's new", card.into_iter().collect(), false);
        });
    }
    let weak = app.as_weak();
    app.on_open_url(move |url| {
        let Some(app) = weak.upgrade() else { return };
        if let Err(err) = std::process::Command::new("xdg-open")
            .arg(url.as_str())
            .spawn()
        {
            toast(
                &app,
                ToastKind::Error,
                format!("Couldn't open {url}: {err}"),
                "",
            );
        }
    });
}

/// Open the What's new view on `cards`, newest first. `full` means every
/// release is shown, which hides its "Show full changelog" button.
pub(super) fn show_notes(app: &AppWindow, title: &str, cards: Vec<NotesCard>, full: bool) {
    app.set_whatsnew_title(title.into());
    app.set_whatsnew_cards(Rc::new(VecModel::from(cards)).into());
    app.set_whatsnew_full(full);
    app.set_show_whatsnew(true);
}

/// One release's notes as a card of the What's new view.
pub(super) fn notes_card(title: String, date: &str, badge: &str, notes: &str) -> NotesCard {
    let lines: Vec<NoteLine> = changelog::blocks(&changelog::renderable(notes))
        .into_iter()
        .map(|block| NoteLine {
            kind: block.kind as i32,
            tone: block.tone,
            text: block.text.into(),
            meta: block.meta.into(),
        })
        .collect();
    NotesCard {
        title: title.into(),
        date: date.into(),
        badge: badge.into(),
        lines: Rc::new(VecModel::from(lines)).into(),
    }
}

/// A bundled changelog section as a card.
pub(super) fn release_card(section: &changelog::Section) -> NotesCard {
    notes_card(
        format!("Version {}", section.version),
        &section.date,
        version_badge(&section.version),
        &section.body,
    )
}

/// A pre-release (`0.3.0-beta.4`) is a beta; a plain version is stable.
fn version_badge(version: &str) -> &'static str {
    if version.contains('-') {
        "Beta"
    } else {
        "Stable"
    }
}
