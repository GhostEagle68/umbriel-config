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
                .unwrap_or_else(|| key.clone());
            // The popup's ComboBox indexes the main-first destinations
            // model, not the chain: 0 = main, include i = i + 1.
            let dest_index = if i == main { 0 } else { i + 1 };
            entries.push(SaveEntry {
                key: key.clone().into(),
                label: label.into(),
                value: value.unwrap_or_else(|| "(removed)".to_owned()).into(),
                dest_label: labels.get(i).cloned().unwrap_or_default(),
                dest_index: dest_index as i32,
            });
        }
    }
    entries
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
        super::page_backups::snapshot_before_save(shell, env, "save");
    }

    let mut saved_files = 0;
    for inc in &mut shell.includes.docs {
        if inc.doc.is_modified() {
            if let Err(err) = inc.doc.save(&inc.path) {
                app.set_status(format!("save failed: {err}").into());
                return false;
            }
            saved_files += 1;
        }
    }
    if shell.doc.is_modified() {
        let path = shell.path.clone();
        if let Err(err) = shell.doc.save(&path) {
            app.set_status(format!("save failed: {err}").into());
            return false;
        }
        saved_files += 1;
    }
    let report = validate::validate(&shell.path);
    shell.reset_saved();

    app.set_dirty(false);
    app.set_changed_count(0);
    match report {
        Ok(report) if report.diagnostics.is_empty() => {
            app.set_validate_note(String::new().into());
            app.set_status(
                format!("Saved {saved_files} file(s); umbriel has validated the config.").into(),
            );
        }
        Ok(report) => {
            let messages: Vec<String> = report
                .diagnostics
                .iter()
                .map(|d| d.message().to_owned())
                .collect();
            app.set_validate_note(messages.join("; ").into());
            // Warnings apply with per-setting fallbacks; only errors mean
            // umbriel kept something out.
            let verdict = if report.is_ok() {
                "umbriel noted warnings"
            } else {
                "umbriel has complaints"
            };
            app.set_status(
                format!("Saved {saved_files} file(s); {verdict} — see the banner.").into(),
            );
        }
        Err(err) => {
            app.set_validate_note(format!("umbriel could not be run: {err}").into());
            app.set_status(format!("Saved {saved_files} file(s) without validation.").into());
        }
    }
    true
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
                let main = shell.includes.docs.len();
                let mut changed: Vec<String> = Vec::new();
                for i in 0..=main {
                    let saved = shell.saved.get(i);
                    let current: BTreeMap<String, String> =
                        doc_at(&shell, i).leaf_values().into_iter().collect();
                    for (key, _) in diff_against_saved(&current, saved) {
                        changed.push(key);
                    }
                }
                for key in &changed {
                    reset_key(&mut shell, key);
                }
            }
            let shell = shell.borrow();
            super::sections::refresh_shown_page(&app, &shell);
            app.set_show_save_popup(false);
            app.set_status("Discarded all unsaved changes.".into());
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
                    source.remove_table(&parts);
                }
            }

            if save_all_and_validate(&app, &mut shell, &env) {
                super::sections::refresh_shown_page(&app, &shell);
            }
        });
    }
}
