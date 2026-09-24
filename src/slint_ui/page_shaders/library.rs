//! The library and its assignments: scanning shader files, drawing the page's
//! cards and dropdowns, resolving and writing `animation.<event>.shader`, and
//! deleting shaders.

use super::super::common::*;
use super::super::*;

/// Rescan shader locations: the main config's directory (its
/// `shaders/` folder holds user copies and the community clone) and
/// umbriel's installed data directory (bundled effects).
pub(in crate::slint_ui) fn scan_shaders(shell: &mut Shell) {
    let config_dir = shell
        .path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    // Bundled shaders live at <data root>/umbriel/shaders; the
    // packaged default config sits at <data root>/umbriel/config.toml.
    let env = discovery::Env::from_process();
    let data_roots: Vec<PathBuf> = discovery::packaged_default(&env)
        .and_then(|path| path.parent().and_then(Path::parent).map(Path::to_path_buf))
        .into_iter()
        .collect();
    shell.shaders = shaders::scan(&config_dir, &data_roots);
    shell.shaders_installed = config_dir.join("shaders/community").is_dir();
}

pub(in crate::slint_ui) fn rebuild_shaders(app: &AppWindow, shell: &Shell) {
    // Resolve each event's assignment and each library file once; the
    // cards and rows below only compare these (no per-pair filesystem
    // lookups).
    let assigned = resolved_assignments(shell);
    let entry_keys: Vec<PathBuf> = shell
        .shaders
        .iter()
        .map(|entry| shaders::file_key(&entry.path))
        .collect();
    let infos: Vec<ShaderInfo> = shell
        .shaders
        .iter()
        .zip(&entry_keys)
        .map(|(entry, entry_key)| ShaderInfo {
            name: entry.name.clone().into(),
            value: entry.value.clone().into(),
            source: entry.source.label().into(),
            description: entry.description.clone().into(),
            invalid: entry.invalid.clone().unwrap_or_default().into(),
            path: entry.path.display().to_string().into(),
            is_own: entry.source == shaders::Source::ConfigDir,
            used_by: shaders::EVENTS
                .iter()
                .zip(&assigned)
                .filter(|(_, current)| {
                    current
                        .as_ref()
                        .is_some_and(|current| current.key == *entry_key)
                })
                .map(|(event, _)| prettify(event))
                .collect::<Vec<_>>()
                .join(", ")
                .into(),
        })
        .collect();
    app.set_shaders(Rc::new(VecModel::from(infos)).into());

    let rows: Vec<ShaderAssignment> = shaders::EVENTS
        .iter()
        .zip(assigned)
        .map(|(event, current)| {
            let mut choices: Vec<SharedString> = vec!["(no shader)".into()];
            choices.extend(shell.shaders.iter().map(|entry| entry.label.clone().into()));
            // Match by the file umbriel would actually read, so
            // "./shaders/x.glsl" and an absolute path both find x.glsl.
            let index = current.as_ref().and_then(|current| {
                entry_keys
                    .iter()
                    .position(|entry_key| *entry_key == current.key)
            });
            let warning = match &current {
                Some(current) => {
                    shaders::assignment_problem(&current.value, &current.path).unwrap_or_default()
                }
                None => "",
            };
            let current = current.map(|current| current.value);
            // A value outside the library gets its own trailing entry, so
            // the dropdown shows it and "(no shader)" is a real change.
            let current_index = match (index, &current) {
                (Some(position), _) => position + 1,
                (None, Some(value)) => {
                    let label = if warning.is_empty() {
                        format!("{value} (outside the library)")
                    } else {
                        format!("⚠ {value} (missing)")
                    };
                    choices.push(label.into());
                    choices.len() - 1
                }
                (None, None) => 0,
            };
            ShaderAssignment {
                key: format!("animation.{event}.shader").into(),
                label: prettify(event).into(),
                choices: Rc::new(VecModel::from(choices)).into(),
                current: current_index as i32,
                current_value: current.unwrap_or_default().into(),
                warning: warning.into(),
            }
        })
        .collect();
    app.set_shader_assignments(Rc::new(VecModel::from(rows)).into());
    app.set_shader_download_note(shell.shader_note.clone().into());
    app.set_shader_include_missing(shaders::missing_include(&shell.doc, &shell.path).is_some());
    app.set_shader_community_installed(shell.shaders_installed);
    app.set_changed_count(changed_count(shell));
}

/// One event's current assignment, resolved the way umbriel reads it.
pub(super) struct Resolved {
    /// The value as written.
    pub(super) value: String,
    /// Chain index of the document it comes from.
    pub(super) doc: usize,
    /// Where umbriel reads the shader from.
    pub(super) path: PathBuf,
    /// `path`'s identity for comparisons (`shaders::file_key`).
    pub(super) key: PathBuf,
}

/// Every event's assignment (in `shaders::EVENTS` order), resolved once.
/// The single place assignment paths are interpreted: the dropdowns,
/// the delete confirm and the Use-for checklist all read from it.
pub(super) fn resolved_assignments(shell: &Shell) -> Vec<Option<Resolved>> {
    let docs = chain_docs(shell);
    let paths = chain_paths(shell);
    shaders::EVENTS
        .iter()
        .map(|event| {
            shaders::current_assignment(&docs, event).map(|(value, doc)| {
                let path = shaders::resolve(&value, &paths[doc]);
                let key = shaders::file_key(&path);
                Resolved {
                    value,
                    doc,
                    path,
                    key,
                }
            })
        })
        .collect()
}

/// Every event whose assignment resolves to `shader`, with the chain
/// index of the document holding it.
pub(super) fn assignments_of(shell: &Shell, shader: &Path) -> Vec<(&'static str, usize)> {
    let key = shaders::file_key(shader);
    shaders::EVENTS
        .iter()
        .zip(resolved_assignments(shell))
        .filter_map(|(event, assigned)| {
            let assigned = assigned?;
            (assigned.key == key).then_some((*event, assigned.doc))
        })
        .collect()
}

/// "Windows out, Overview" for the events assigned `shader`.
pub(super) fn used_by(shell: &Shell, shader: &Path) -> String {
    assignments_of(shell, shader)
        .iter()
        .map(|(event, _)| prettify(event))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Point `event` at `shader`, or clear it with `None`, as an unsaved
/// change. An existing key is edited where it lives; a brand-new one
/// starts beside the other assignments (the save popup can move it).
/// The value is spelled for the file it lands in, since umbriel
/// resolves relative paths from there, and written as a typed string
/// so it is always quoted.
pub(super) fn assign_event(shell: &mut Shell, event: &str, shader: Option<&Path>) {
    let home = shaders::assignment_home(&chain_docs(shell), event);
    let key = ["animation", event, "shader"];
    match (shader, home) {
        (Some(shader), home) => {
            let target = home.unwrap_or_else(|| new_home(shell));
            let value = shaders::value_for(shader, &chain_paths(shell)[target]);
            doc_at_mut(shell, target).set_string(&key, &value);
        }
        (None, Some(home)) => {
            doc_at_mut(shell, home).remove_leaf(&key);
        }
        (None, None) => {}
    }
}

/// Chain index for a brand-new assignment; see
/// [`shaders::new_assignment_home`].
pub(super) fn new_home(shell: &Shell) -> usize {
    let names: Vec<String> = chain_paths(shell)
        .iter()
        .map(|path| file_name_of(path))
        .collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    shaders::new_assignment_home(&chain_docs(shell), &names)
}

/// Write assignment changes straight to their files (`None` clears).
/// Renaming or deleting a shader happens on disk at once, so the
/// assignments that follow it must too: left unsaved, a Discard would
/// point them at a file that no longer exists. Other unsaved edits in
/// those files stay unsaved.
pub(super) fn write_assignments(
    shell: &mut Shell,
    edits: &[(&'static str, usize, Option<String>)],
) -> Result<(), String> {
    if edits.is_empty() {
        return Ok(());
    }
    // Like every other write, take a backup run of the on-disk chain first.
    super::page_backups::snapshot_before_save(shell, &discovery::Env::from_process(), "shader");
    let paths = chain_paths(shell);
    for (event, doc, value) in edits {
        let key = ["animation", event, "shader"];
        doc_at_mut(shell, *doc)
            .write_through(&paths[*doc], |d| match value {
                Some(value) => d.set_string(&key, value),
                None => {
                    d.remove_leaf(&key);
                }
            })
            .map_err(|err| format!("could not update {}: {err}", paths[*doc].display()))?;
        // The written key's new on-disk value is its saved baseline. Only
        // that key: the rest of the baseline may hold values that aren't
        // on disk yet (the guided setup's suggestions) and must stay.
        let on_disk: ConfigDocument = doc_at(shell, *doc)
            .original_text()
            .parse()
            .map_err(|err| format!("{err}"))?;
        if let Some(slot) = shell.saved.get_mut(*doc) {
            rebase_saved_key(slot, &key.join("."), &on_disk);
        }
    }
    Ok(())
}

/// Set one key's saved baseline to its value in `on_disk` (dropping it
/// when the file no longer has it), leaving every other key alone.
pub(super) fn rebase_saved_key(
    saved: &mut BTreeMap<String, String>,
    dotted: &str,
    on_disk: &ConfigDocument,
) {
    let repr = on_disk
        .leaf_values()
        .into_iter()
        .find_map(|(path, repr)| (path == dotted).then_some(repr));
    match repr {
        Some(repr) => {
            saved.insert(dotted.to_owned(), repr);
        }
        None => {
            saved.remove(dotted);
        }
    }
}

/// Delete one of the user's own shaders, then rescan. An editor open on
/// that same file closes with it, and assignments pointing at it are
/// cleared (unsaved) so no event is left on a missing file.
pub(super) fn delete_shader(app: &AppWindow, shell: &Rc<RefCell<Shell>>, path: &Path) {
    // Found before the delete: matching needs the file on disk.
    let assigned = assignments_of(&shell.borrow(), path);
    let result = shell
        .borrow()
        .path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "could not determine the config directory.".to_owned())
        .and_then(|dir| shaders::delete_user_shader(&dir, path));
    match result {
        Ok(()) => {
            {
                let mut shell = shell.borrow_mut();
                if shell.shader_editing.as_deref() == Some(path) {
                    shell.shader_editing = None;
                    app.set_shader_editor_open(false);
                }
                let clears: Vec<_> = assigned
                    .iter()
                    .map(|(event, doc)| (*event, *doc, None))
                    .collect();
                if let Err(err) = write_assignments(&mut shell, &clears) {
                    app.set_status(err.into());
                }
                scan_shaders(&mut shell);
            }
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_shaders(app, &shell);
            let status = if assigned.is_empty() {
                format!("Deleted {}.", path.display())
            } else {
                let events: Vec<String> =
                    assigned.iter().map(|(event, _)| prettify(event)).collect();
                format!(
                    "Deleted {} and cleared it from {}.",
                    path.display(),
                    events.join(", ")
                )
            };
            app.set_status(status.into());
        }
        Err(err) if app.get_shader_editor_open() => app.set_shader_editor_note(err.into()),
        Err(err) => app.set_status(err.into()),
    }
}

pub(super) fn install(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_assign(move |key, index| {
            let Some(app) = weak.upgrade() else { return };
            let Some(event) = key.rsplit('.').nth(1).map(str::to_owned) else {
                return;
            };
            {
                let mut shell = shell.borrow_mut();
                // Resolve the pick against the list the dropdown was built
                // from; an index past it changes nothing.
                let shader = match usize::try_from(index) {
                    Ok(0) | Err(_) => None,
                    Ok(index) => shell.shaders.get(index - 1).map(|entry| entry.path.clone()),
                };
                match shader {
                    Some(shader) => assign_event(&mut shell, &event, Some(&shader)),
                    None if index == 0 => assign_event(&mut shell, &event, None),
                    None => {}
                }
            }
            // Always rebuild so the dropdowns mirror the documents, even
            // when the pick changed nothing.
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_shaders(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_fix_include(move || {
            let Some(app) = weak.upgrade() else { return };
            let Some(entry) = ({
                let shell = shell.borrow();
                shaders::missing_include(&shell.doc, &shell.path)
            }) else {
                return;
            };
            {
                let mut shell = shell.borrow_mut();
                let mut files = shell
                    .doc
                    .get_strings(&["include", "files"])
                    .unwrap_or_default();
                files.push(entry);
                shell.doc.set_strings(&["include", "files"], &files);
            }
            let shell = shell.borrow();
            app.set_status("Added shaders.toml to [include] — save to apply.".into());
            app.set_dirty(shell.any_modified());
            rebuild_shaders(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // A library card's Delete names its own file; it never goes
        // through the editor's remembered path, which may be stale.
        app.on_shader_delete_file(move |path| {
            let Some(app) = weak.upgrade() else { return };
            let path = PathBuf::from(path.as_str());
            let own = shell
                .borrow()
                .shaders
                .iter()
                .any(|entry| entry.source == shaders::Source::ConfigDir && entry.path == path);
            if own {
                delete_shader(&app, &shell, &path);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn rebasing_one_key_leaves_the_rest_of_the_baseline() {
        // A guide suggestion sits in the baseline but not on disk.
        let mut saved = BTreeMap::from([
            ("general.xwayland".to_owned(), "true".to_owned()),
            (
                "animation.windows_out.shader".to_owned(),
                "\"shaders/old.glsl\"".to_owned(),
            ),
        ]);
        let on_disk =
            ConfigDocument::from_str("[animation.windows_out]\nshader = \"shaders/new.glsl\"\n")
                .unwrap();
        rebase_saved_key(&mut saved, "animation.windows_out.shader", &on_disk);
        assert_eq!(
            saved["animation.windows_out.shader"],
            "\"shaders/new.glsl\""
        );
        assert_eq!(saved["general.xwayland"], "true", "untouched");
        // A key the file no longer has leaves the baseline.
        let cleared = ConfigDocument::from_str("").unwrap();
        rebase_saved_key(&mut saved, "animation.windows_out.shader", &cleared);
        assert!(!saved.contains_key("animation.windows_out.shader"));
        assert_eq!(saved.len(), 1);
    }
}
