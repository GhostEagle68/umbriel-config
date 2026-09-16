//! The Settings page: app preferences, the update check, the upstream
//! commit line, the file viewer, and the changelog/update-notes overlays.

use super::common::*;
use super::*;

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
    app.set_update_note("Checking…".into());
    // Read the channel here: the worker can't touch the window.
    let prereleases = app.get_update_prereleases();
    std::thread::spawn(move || {
        let result = update::check(prereleases);
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
                    app.set_update_note(format!("Version {version} available.").into());
                    // Who owns this binary decides whether the app may
                    // install the update or only name the command.
                    let kind = update::current_install_kind();
                    app.set_update_can_install(kind == update::InstallKind::Tarball);
                    app.set_update_hint(update::update_hint(kind, &version).into());
                    app.set_update_version(version.into());
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
    let Some((sha, message, date)) = github_latest_commit(
        "https://api.github.com/repos/noctalia-dev/umbriel/commits?per_page=1",
    ) else {
        return Err("no commits found".to_owned());
    };
    let sha: String = sha.chars().take(7).collect();
    let date = date.get(..10).unwrap_or_default();
    Ok(format!("{sha} • {date} • {message}"))
}

pub(super) fn install_settings(app: &AppWindow, shell: &Rc<RefCell<Shell>>, env: &discovery::Env) {
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
        app.on_install_update(move || {
            let Some(app) = weak.upgrade() else { return };
            let version = app.get_update_version().to_string();
            if version.is_empty() {
                return;
            }
            app.set_update_note("Installing…".into());
            let weak = app.as_weak();
            std::thread::spawn(move || {
                let result = update::install(&version);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(app) = weak.upgrade() else { return };
                    match result {
                        Ok(()) => {
                            app.set_update_installed(true);
                            app.set_update_note(
                                format!("Installed {version}. Restart to use it.").into(),
                            );
                        }
                        Err(err) => app.set_update_note(format!("Install failed: {err}").into()),
                    }
                });
            });
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_restart_app(move || {
            let Some(app) = weak.upgrade() else { return };
            // The new binary is already on disk; unsaved edits would be
            // lost with the process, so they stop the restart.
            if app.get_dirty() {
                app.set_update_note("Save or discard your changes first.".into());
                return;
            }
            let Ok(exe) = std::env::current_exe() else {
                app.set_update_note("Restart manually to use the new version.".into());
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
                Err(err) => app.set_update_note(format!("Couldn't restart: {err}").into()),
            }
        });
    }
    {
        let weak = app.as_weak();
        let env = env.clone();
        app.on_update_channel_selected(move |prereleases| {
            let Some(app) = weak.upgrade() else { return };
            app.set_update_prereleases(prereleases);
            let mut settings = app_settings::load(&env);
            settings.prereleases = prereleases;
            let _ = app_settings::store(&env, &settings);
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
            let _ = app_settings::store(&env, &settings);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
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
        let shell = Rc::clone(shell);
        let env = env.clone();
        app.on_sync_schema_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            // Re-read the packaged default, diff old vs fresh, store the
            // fresh snapshot; the added keys get badges.
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
            app.set_sections(Rc::new(VecModel::from(super::sections::section_nav(&shell))).into());
            let section = app.get_current_section().to_string();
            super::sections::refill_page(&app, &shell, &section);
        });
    }
    {
        let weak = app.as_weak();
        app.on_view_changelog(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_whatsnew_title("Changelog".into());
            app.set_whatsnew_body(
                changelog::renderable(&changelog::full_text(&changelog::parse(
                    changelog::bundled(),
                )))
                .into(),
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
                    changelog::renderable(
                        &section
                            .map(|section| section.body.clone())
                            .unwrap_or_default(),
                    )
                    .into(),
                );
            } else {
                // The fetched body is the release page's markdown, emoji
                // headings and all.
                app.set_whatsnew_title("What's new in this release".into());
                app.set_whatsnew_body(changelog::renderable(&notes).into());
            }
            app.set_show_whatsnew(true);
        });
    }
    app.on_open_url(|url| {
        let _ = std::process::Command::new("xdg-open")
            .arg(url.as_str())
            .spawn();
    });
}
