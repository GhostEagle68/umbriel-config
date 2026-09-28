//! The save flow: the popup's per-key destination picker, reset/discard,
//! and the write-everything + validate step shared with the guided setup.

use super::common::*;
use super::rows::refresh_row;
use super::*;

/// The current diff across the chain, in save-popup form. A deletion
/// shows as `(removed)` so a cleared key is auditable and savable.
pub(super) fn build_save_entries(shell: &Shell) -> Vec<SaveEntry> {
    let main = shell.includes.docs.len();
    let labels = setting_labels(shell);
    let destinations = save_destinations(shell);
    let mut entries: Vec<SaveEntry> = Vec::new();
    for i in 0..=main {
        let saved = shell.saved.get(i);
        let current: BTreeMap<String, String> =
            doc_at(shell, i).leaf_values().into_iter().collect();
        for (key, value) in diff_against_saved(&current, saved) {
            let label = shell
                .schema
                .iter()
                .find(|entry| entry.path.join(".") == key.as_str())
                .map(|entry| entry.label.clone())
                .unwrap_or_else(|| match key.strip_prefix("keybinds.") {
                    Some(chord) => format!("Keybind {chord}"),
                    // A top-level leaf is a rule list, like window_rule.
                    None if !key.contains('.') => prettify(&key),
                    None => key.clone(),
                });
            // The popup's ComboBox indexes the destinations model, not
            // the chain.
            let dest_label = labels.get(i).cloned().unwrap_or_default();
            let dest_index = destinations
                .iter()
                .position(|dest| *dest == dest_label)
                .map_or(-1, |index| index as i32);
            entries.push(SaveEntry {
                key: key.clone().into(),
                label: label.into(),
                value: value.unwrap_or_else(|| "(removed)".to_owned()).into(),
                dest_label,
                dest_index,
            });
        }
    }
    entries
}

/// The files a changed setting can be saved to, main first. Preset-only
/// files are left out: umbriel's bundled ones are read-only, and the
/// app rewrites its own.
pub(super) fn save_destinations(shell: &Shell) -> Vec<SharedString> {
    let labels = setting_labels(shell);
    let main = shell.includes.docs.len();
    let includes = shell
        .includes
        .docs
        .iter()
        .zip(&labels)
        .filter(|(inc, _)| {
            let leaves = inc.doc.leaf_values();
            leaves.is_empty()
                || !leaves
                    .iter()
                    .all(|(key, _)| key.starts_with("effects.preset."))
        })
        .map(|(_, label)| label.clone());
    std::iter::once(labels[main].clone())
        .chain(includes)
        .collect()
}

/// Write every modified doc, validate through umbriel, and surface the
/// verdict. Shared by the save popup and the guided setup's last step.
/// Returns false when a save failed (the status line already says so).
pub(super) fn save_all_and_validate(
    app: &AppWindow,
    shell: &mut Shell,
    env: &discovery::Env,
) -> bool {
    // Backup the on-disk chain before any of it is overwritten.
    if shell.doc.is_modified() || shell.includes.docs.iter().any(|inc| inc.doc.is_modified()) {
        super::page_backups::snapshot_before_save(app, shell, env, "save");
    }

    let mut saved_files = 0;
    for inc in &mut shell.includes.docs {
        if inc.doc.is_modified() {
            if let Err(err) = inc.doc.save(&inc.path) {
                alert(app, "Couldn't save", err.to_string());
                return false;
            }
            saved_files += 1;
        }
    }
    if shell.doc.is_modified() {
        let path = shell.path.clone();
        if let Err(err) = shell.doc.save(&path) {
            alert(app, "Couldn't save", err.to_string());
            return false;
        }
        saved_files += 1;
    }
    let report = validate::validate(&shell.path);
    shell.reset_saved();

    app.set_dirty(false);
    app.set_changed_count(0);
    let plural = if saved_files == 1 { "" } else { "s" };
    report_validation(app, report, &format!("Saved {saved_files} file{plural}"));
    true
}

/// Tell the user how umbriel took the config just written; `done` says
/// what happened ("Saved 2 files"). Errors mean umbriel kept something
/// out, so they get the popup; warnings apply with per-setting fallbacks.
pub(super) fn report_validation(
    app: &AppWindow,
    report: Result<validate::Report, validate::ValidateError>,
    done: &str,
) {
    match report {
        Ok(report) if report.diagnostics.is_empty() => toast(
            app,
            ToastKind::Success,
            format!("{done}. Umbriel accepted the config."),
            "",
        ),
        Ok(report) => {
            let messages: Vec<&str> = report.diagnostics.iter().map(|d| d.message()).collect();
            if report.is_ok() {
                toast(
                    app,
                    ToastKind::Warning,
                    format!("{done}. Umbriel warns: {}", messages.join("; ")),
                    "",
                );
            } else {
                alert(
                    app,
                    "Umbriel rejected part of the config",
                    format!("{done}, but umbriel reported:\n\n{}", messages.join("\n")),
                );
            }
        }
        Err(err) => toast(
            app,
            ToastKind::Warning,
            format!("{done} without validation: {err}."),
            "",
        ),
    }
}

pub(super) fn install_save(app: &AppWindow, shell: &Rc<RefCell<Shell>>, env: &discovery::Env) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_save_requested(move || {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            let entries = build_save_entries(&shell);
            if entries.is_empty() {
                toast(&app, ToastKind::Info, "Nothing to save.", "");
                return;
            }
            // Rebuilt each time: the include list changes as you edit.
            app.set_destinations(Rc::new(VecModel::from(save_destinations(&shell))).into());
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
        let shell = Rc::clone(shell);
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
            refresh_row(&app, &shell, &key);
            super::sections::refresh_shown_page(&app, &shell);
        });
    }
    {
        // Row-level reset (the slider "Reset" button): back to the
        // stored value, without the save popup.
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
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
        let shell = Rc::clone(shell);
        app.on_discard_all(move || {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut shell = shell.borrow_mut();
                shell.doc.discard();
                for inc in &mut shell.includes.docs {
                    inc.doc.discard();
                }
            }
            let shell = shell.borrow();
            super::sections::refresh_shown_page(&app, &shell);
            app.set_show_save_popup(false);
            toast(&app, ToastKind::Info, "Discarded all unsaved changes.", "");
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
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
                    // remove_leaf prunes the tables left empty, so the
                    // old file doesn't keep a bare section header.
                    source.remove_leaf(&parts);
                }
            }

            if save_all_and_validate(&app, &mut shell, &env) {
                super::sections::refresh_shown_page(&app, &shell);
                app.set_show_save_popup(false);
            }
        });
    }
}
