//! The Backups page: run history, restore diffs, and the backup
//! location/count preferences.

use super::common::*;
use super::*;

/// The backups base directory: the user's chosen folder, else the default.
pub(super) fn backup_base(settings: &app_settings::Settings, env: &discovery::Env) -> PathBuf {
    match settings.backup_dir.as_deref() {
        Some(dir) => PathBuf::from(dir),
        None => backups::default_base(env),
    }
}

pub(super) fn backup_note(settings: &app_settings::Settings, env: &discovery::Env) -> SharedString {
    format!(
        "Saving to {} · keeping {} backups",
        backup_base(settings, env).display(),
        settings.backup_count
    )
    .into()
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

/// (file name, path) for every file in the chain, main first: matches
/// a run's stored entries back onto the live files.
fn chain_files(shell: &Shell) -> Vec<(String, PathBuf)> {
    let mut chain: Vec<(String, PathBuf)> = vec![(file_name_of(&shell.path), shell.path.clone())];
    chain.extend(
        shell
            .includes
            .docs
            .iter()
            .map(|inc| (file_name_of(&inc.path), inc.path.clone())),
    );
    chain
}

/// Unified diff (backup vs current content) for every file in a run.
fn backup_diff(shell: &Shell, base: &Path, id: &str) -> String {
    let files = match backups::read_run(base, id) {
        Ok(files) => files,
        Err(err) => return format!("Backup unreadable: {err}"),
    };
    let chain = chain_files(shell);
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
pub(super) fn snapshot_before_save(shell: &Shell, env: &discovery::Env, trigger: &str) {
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

pub(super) fn install_backups(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    env: &discovery::Env,
    backup_runs: &Rc<RefCell<Vec<backups::RunInfo>>>,
) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let env = env.clone();
        let backup_runs = Rc::clone(backup_runs);
        app.on_backups_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_page(Page::Backups);
            let shell = shell.borrow();
            refresh_backup_runs(&app, &shell, &env, &backup_runs);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let env = env.clone();
        let backup_runs = Rc::clone(backup_runs);
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
        let shell = Rc::clone(shell);
        let env = env.clone();
        let backup_runs = Rc::clone(backup_runs);
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
        let shell = Rc::clone(shell);
        let env = env.clone();
        let backup_runs = Rc::clone(backup_runs);
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
                let chain = chain_files(&shell);
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
                app.set_sections(
                    Rc::new(VecModel::from(super::sections::section_nav(&shell))).into(),
                );
                let section = app.get_current_section().to_string();
                super::sections::refill_page(&app, &shell, &section);
                refresh_backup_runs(&app, &shell, &env, &backup_runs);
            }
            match validate::validate(&shell.borrow().path) {
                Ok(report) if report.diagnostics.is_empty() => {
                    app.set_validate_note(String::new().into())
                }
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
        let shell = Rc::clone(shell);
        let env = env.clone();
        let backup_runs = Rc::clone(backup_runs);
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
}
