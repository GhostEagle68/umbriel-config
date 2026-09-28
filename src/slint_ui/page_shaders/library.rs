//! The library and its assignments: scanning shader files, drawing the page's
//! library and assignment rows, resolving and writing `animation.<event>.effect`
//! (with the preset file and its include), and deleting shaders.

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
    shell.shaders = shaders::scan(&config_dir, &data_roots());
    shell.shaders_installed = config_dir.join("shaders/community").is_dir();
}

/// Where umbriel's bundled presets live: `<data root>/umbriel/effects`,
/// beside the packaged default config at `<data root>/umbriel/config.toml`.
pub(in crate::slint_ui) fn data_roots() -> Vec<PathBuf> {
    discovery::packaged_default(&discovery::Env::from_process())
        .and_then(|path| path.parent().and_then(Path::parent).map(Path::to_path_buf))
        .into_iter()
        .collect()
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
    // Each section numbers its own shaders, for the grid's positions.
    let (mut own_count, mut other_count) = (0, 0);
    let infos: Vec<ShaderInfo> = shell
        .shaders
        .iter()
        .zip(&entry_keys)
        .map(|(entry, entry_key)| {
            let is_own = entry.source == shaders::Source::ConfigDir;
            let count = if is_own {
                &mut own_count
            } else {
                &mut other_count
            };
            let slot = *count;
            *count += 1;
            let thumb = shell.shader_thumbs.get(&entry.path);
            ShaderInfo {
                name: entry.name.clone().into(),
                value: entry.preset.clone().into(),
                source: entry.source.label().into(),
                description: entry.description.clone().into(),
                invalid: entry.invalid.clone().unwrap_or_default().into(),
                path: entry.path.display().to_string().into(),
                is_own,
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
                thumb: thumb.cloned().unwrap_or_default(),
                has_thumb: thumb.is_some(),
                slot,
            }
        })
        .collect();
    app.set_shaders(Rc::new(VecModel::from(infos)).into());
    app.set_shader_own_count(own_count);
    app.set_shader_other_count(other_count);

    let rows: Vec<ShaderAssignment> = shaders::EVENTS
        .iter()
        .zip(assigned)
        .map(|(event, current)| {
            // Match by the file umbriel would actually read, so
            // "./shaders/x.glsl" and an absolute path both find x.glsl.
            let index = current.as_ref().and_then(|current| {
                entry_keys
                    .iter()
                    .position(|entry_key| *entry_key == current.key)
            });
            let warning = match &current {
                Some(current) => {
                    shaders::assignment_problem(current.path.as_deref()).unwrap_or_default()
                }
                None => "",
            };
            let current = current.map(|current| current.value);
            // 0 is no shader, 1.. the library; a value outside the
            // library sits just past it, so picking "No shader" is a
            // real change.
            let (current_index, current_name) = match (index, &current) {
                (Some(position), _) => (position + 1, shell.shaders[position].name.clone()),
                (None, Some(value)) => (shell.shaders.len() + 1, value.clone()),
                (None, None) => (0, "No shader".to_owned()),
            };
            ShaderAssignment {
                key: format!("animation.{event}.effect").into(),
                label: prettify(event).into(),
                current: current_index as i32,
                current_name: current_name.into(),
                current_value: current.unwrap_or_default().into(),
                warning: warning.into(),
            }
        })
        .collect();
    app.set_shader_assignments(Rc::new(VecModel::from(rows)).into());
    app.set_shader_download_note(shell.shader_note.clone().into());
    app.set_shader_include_missing(shaders::missing_include(&shell.doc, &shell.path).is_some());
    let legacy = shaders::legacy_assignments(&chain_docs(shell));
    app.set_shader_legacy_note(
        match legacy.len() {
            0 => String::new(),
            1 => "Umbriel no longer reads shader paths, so 1 animation runs its built-in effect \
                  instead of its shader. Convert it to an effect preset."
                .to_owned(),
            n => format!(
                "Umbriel no longer reads shader paths, so {n} animations run their built-in \
                 effect instead of their shaders. Convert them to effect presets."
            ),
        }
        .into(),
    );
    app.set_shader_community_installed(shell.shaders_installed);
    app.set_changed_count(changed_count(shell));
}

/// One event's current assignment, resolved the way umbriel reads it.
pub(super) struct Resolved {
    /// The preset name as written.
    pub(super) value: String,
    /// Chain index of the document it comes from.
    pub(super) doc: usize,
    /// The shader file the preset runs; `None` when nothing defines it.
    pub(super) path: Option<PathBuf>,
    /// `path`'s identity for comparisons (`shaders::file_key`); empty,
    /// so it matches no file, when the preset is unknown.
    pub(super) key: PathBuf,
}

/// Every event's assignment (in `shaders::EVENTS` order), resolved once.
/// The single place assignment paths are interpreted: the assignment rows,
/// the delete confirm and the Use-for checklist all read from it.
pub(super) fn resolved_assignments(shell: &Shell) -> Vec<Option<Resolved>> {
    let docs = chain_docs(shell);
    let paths = chain_paths(shell);
    shaders::EVENTS
        .iter()
        .map(|event| {
            shaders::current_assignment(&docs, event).map(|(value, doc)| {
                // Defined in the chain, or a library preset whose include
                // isn't saved (so not loaded) yet.
                let path = shaders::preset_shader(&docs, &paths, &value).or_else(|| {
                    shell
                        .shaders
                        .iter()
                        .find(|entry| entry.preset == value)
                        .map(|entry| entry.path.clone())
                });
                let key = path.as_deref().map(shaders::file_key).unwrap_or_default();
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

/// Point `event` at `shader`'s preset, or clear it with `None`, as an
/// unsaved change. An existing key is edited where it lives; a brand-new
/// one starts beside the other assignments (the save popup can move it).
/// Assigning also writes the preset file when it's missing and lists it
/// under `[include] files` (unsaved), since umbriel only knows presets
/// from files it reads.
pub(super) fn assign_event(
    shell: &mut Shell,
    event: &str,
    shader: Option<&Path>,
) -> Result<(), String> {
    let home = shaders::assignment_home(&chain_docs(shell), event);
    let key = ["animation", event, "effect"];
    let Some(shader) = shader else {
        if let Some(home) = home {
            doc_at_mut(shell, home).remove_leaf(&key);
        }
        return Ok(());
    };
    let Some(entry) = shell.shaders.iter().find(|entry| entry.path == shader) else {
        return Err(format!(
            "{} is not in the shader library.",
            shader.display()
        ));
    };
    // Umbriel refuses a preset name defined in two files: the whole
    // config would fall back to its defaults.
    let paths = chain_paths(shell);
    let listed = includes::listed_paths(&shell.doc, &shell.path);
    let clash = shaders::preset_clash(
        &chain_docs(shell),
        &paths,
        &entry.preset,
        &entry.preset_file,
    )
    .map(|other| paths[other].clone())
    .or_else(|| {
        // A library preset of the same name whose include isn't saved
        // yet (so not loaded) clashes just the same once it is.
        shell
            .shaders
            .iter()
            .find(|other| {
                other.preset == entry.preset
                    && !shaders::same_file(&other.preset_file, &entry.preset_file)
                    && listed
                        .iter()
                        .any(|path| shaders::same_file(path, &other.preset_file))
            })
            .map(|other| other.preset_file.clone())
    });
    if let Some(other) = clash {
        return Err(format!(
            "{} already defines a preset named \"{}\". Rename this shader to use it.",
            other.display(),
            entry.preset
        ));
    }
    shaders::ensure_preset_file(entry)?;
    let (preset, preset_file) = (entry.preset.clone(), entry.preset_file.clone());
    include_preset(shell, &preset_file);
    let target = home.unwrap_or_else(|| new_home(shell));
    doc_at_mut(shell, target).set_string(&key, &preset);
    Ok(())
}

/// List `preset_file` under the main config's `[include] files`, as an
/// unsaved change, unless an include already names it.
pub(in crate::slint_ui) fn include_preset(shell: &mut Shell, preset_file: &Path) {
    let listed = includes::listed_paths(&shell.doc, &shell.path);
    if listed
        .iter()
        .any(|path| shaders::same_file(path, preset_file))
    {
        return;
    }
    let mut files = shell
        .doc
        .get_strings(&["include", "files"])
        .unwrap_or_default();
    files.push(shaders::value_for(preset_file, &shell.path));
    shell.doc.set_strings(&["include", "files"], &files);
}

/// Rewrite the main config's include of `old` (a preset file that was
/// just renamed or deleted on disk) to `new`, or drop it with `None`.
/// Written straight to disk like the assignments that follow a rename:
/// an include of a missing file stops umbriel's config from loading.
pub(super) fn repoint_include(
    shell: &mut Shell,
    old: &Path,
    new: Option<&Path>,
) -> Result<(), String> {
    let main_path = shell.path.clone();
    let replacement = new.map(|new| shaders::value_for(new, &main_path));
    // Run on the saved text and on the edited document alike, so other
    // unsaved includes stay unsaved.
    let repoint = |doc: &mut ConfigDocument| {
        let files = doc.get_strings(&["include", "files"]).unwrap_or_default();
        // listed_paths expands `files` first, in order.
        let listed = includes::listed_paths(doc, &main_path);
        let updated: Vec<String> = files
            .iter()
            .zip(&listed)
            .filter_map(|(raw, path)| {
                if shaders::same_file(path, old) {
                    replacement.clone()
                } else {
                    Some(raw.clone())
                }
            })
            .collect();
        if updated != files {
            doc.set_strings(&["include", "files"], &updated);
        }
    };
    let listed = includes::listed_paths(&shell.doc, &main_path);
    if !listed.iter().any(|path| shaders::same_file(path, old)) {
        return Ok(());
    }
    shell
        .doc
        .write_through(&main_path, repoint)
        .map_err(|err| format!("could not update {}: {err}", main_path.display()))
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
    app: &AppWindow,
    shell: &mut Shell,
    edits: &[(&'static str, usize, Option<String>)],
) -> Result<(), String> {
    if edits.is_empty() {
        return Ok(());
    }
    // Like every other write, take a backup run of the on-disk chain first.
    super::page_backups::snapshot_before_save(
        app,
        shell,
        &discovery::Env::from_process(),
        "shader",
    );
    let paths = chain_paths(shell);
    for (event, doc, value) in edits {
        let key = ["animation", event, "effect"];
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

/// Replace every old `shader = "<path>"` assignment with the matching
/// preset's `effect`, as unsaved changes to review before saving. One
/// whose file isn't in the library stays, named in the returned status.
fn convert_legacy(shell: &mut Shell) -> String {
    let paths = chain_paths(shell);
    let mut converted = Vec::new();
    let mut failed = Vec::new();
    for (event, value, doc) in shaders::legacy_assignments(&chain_docs(shell)) {
        let old = shaders::resolve(&value, &paths[doc]);
        let target = shaders::legacy_target(&shell.shaders, &old).map(|entry| entry.path.clone());
        let result = match target {
            Some(shader) => assign_event(shell, event, Some(&shader)),
            None => Err(format!("{value} isn't in the shader library")),
        };
        match result {
            Ok(()) => {
                doc_at_mut(shell, doc).remove_leaf(&["animation", event, "shader"]);
                converted.push(prettify(event));
            }
            Err(err) => failed.push(format!("{}: {err}", prettify(event))),
        }
    }
    let mut status = match converted.len() {
        0 => String::new(),
        _ => format!(
            "Converted {}. Review and save to apply.",
            converted.join(", ")
        ),
    };
    if !failed.is_empty() {
        status.push_str(&format!(" Couldn't convert {}.", failed.join("; ")));
    }
    status.trim().to_owned()
}

/// Delete one of the user's own shaders, then rescan. An editor open on
/// that same file closes with it, and assignments pointing at it are
/// cleared (unsaved) so no event is left on a missing file.
pub(super) fn delete_shader(app: &AppWindow, shell: &Rc<RefCell<Shell>>, path: &Path) {
    // Found before the delete: matching needs the file on disk.
    let assigned = assignments_of(&shell.borrow(), path);
    let preset_file = shaders::preset_file_for(path);
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
                if let Err(err) = write_assignments(app, &mut shell, &clears)
                    .and_then(|()| repoint_include(&mut shell, &preset_file, None))
                {
                    toast(app, ToastKind::Error, err, "");
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
            toast(app, ToastKind::Success, status, "");
        }
        Err(err) if app.get_shader_editor_open() => app.set_shader_editor_note(err.into()),
        Err(err) => toast(app, ToastKind::Error, err, ""),
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
                // Resolve the pick against the list the picker was built
                // from; an index past it changes nothing.
                let shader = match usize::try_from(index) {
                    Ok(0) | Err(_) => None,
                    Ok(index) => shell.shaders.get(index - 1).map(|entry| entry.path.clone()),
                };
                let result = match shader {
                    Some(shader) => assign_event(&mut shell, &event, Some(&shader)),
                    None if index == 0 => assign_event(&mut shell, &event, None),
                    None => Ok(()),
                };
                if let Err(err) = result {
                    toast(&app, ToastKind::Error, err, "");
                }
            }
            // Always rebuild so the assignment rows mirror the documents,
            // even when the pick changed nothing.
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
            toast(
                &app,
                ToastKind::Info,
                "Added shaders.toml to [include] — save to apply.",
                "",
            );
            app.set_dirty(shell.any_modified());
            rebuild_shaders(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_convert_legacy(move || {
            let Some(app) = weak.upgrade() else { return };
            let status = convert_legacy(&mut shell.borrow_mut());
            let shell = shell.borrow();
            toast(&app, ToastKind::Info, status, "");
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

    /// Your config as umbriel 11f6b72 found it: an included shaders.toml
    /// with two `shader = "<path>"` keys, one on umbriel's old bundled
    /// reveal.glsl. Converting leaves effects and includes umbriel reads.
    fn convert_fixture(base: &Path, data_dir: &Path) -> Shell {
        let config_dir = base.join("config");
        let write = |path: &Path, text: &str| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write(
            &config_dir.join("config.toml"),
            "[include]\nfiles = [\"shaders.toml\"]\n",
        );
        write(
            &config_dir.join("shaders.toml"),
            &format!(
                "[animation.windows_out]\nshader = \"shaders/test.glsl\"\n\n\
                 [animation.windows_in]\nshader = \"{}\"\n",
                data_dir.join("umbriel/shaders/reveal.glsl").display()
            ),
        );
        write(
            &config_dir.join("shaders/test.glsl"),
            "vec4 animation(vec2 uv) { return umbriel_sample(uv); }\n",
        );
        let mut shell = Shell::load(
            &config_dir.join("config.toml"),
            &discovery::Env::from_process(),
        );
        shell.shaders = shaders::scan(&config_dir, &[data_dir.to_path_buf()]);
        shell
    }

    #[test]
    fn converting_old_shader_paths_selects_presets_and_includes_them() {
        let base = std::env::temp_dir().join(format!("umbriel-convert-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let data_dir = base.join("data");
        let bundled = data_dir.join("umbriel/effects/animation/reveal");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::write(
            bundled.join("shader.glsl"),
            "vec4 animation(vec2 uv) { return vec4(0.0); }\n",
        )
        .unwrap();
        std::fs::write(
            bundled.join("effect.toml"),
            "[effects.preset.reveal]\nkind = \"animation\"\nshader = \"shader.glsl\"\n",
        )
        .unwrap();
        let mut shell = convert_fixture(&base, &data_dir);

        let status = convert_legacy(&mut shell);
        assert!(status.starts_with("Converted"), "{status}");
        assert!(shaders::legacy_assignments(&chain_docs(&shell)).is_empty());
        // The effects land where the old keys were (shaders.toml).
        let docs = chain_docs(&shell);
        assert_eq!(
            shaders::current_assignment(&docs, "windows_out"),
            Some(("test".to_owned(), 0))
        );
        assert_eq!(
            shaders::current_assignment(&docs, "windows_in"),
            Some(("reveal".to_owned(), 0))
        );
        // Your shader got a preset file; umbriel's is included as it is.
        let includes = shell.doc.get_strings(&["include", "files"]).unwrap();
        assert_eq!(
            includes,
            vec![
                "shaders.toml".to_owned(),
                bundled.join("effect.toml").display().to_string(),
                "shaders/test.effect.toml".to_owned(),
            ]
        );
        assert!(base.join("config/shaders/test.effect.toml").is_file());
        // Both assignments resolve to the shader files.
        let resolved = resolved_assignments(&shell);
        let out = shaders::EVENTS
            .iter()
            .position(|e| *e == "windows_out")
            .unwrap();
        assert_eq!(
            resolved[out].as_ref().unwrap().path.as_deref(),
            Some(base.join("config/shaders/test.glsl").as_path())
        );

        // A community shader also named "test" can't be assigned: the
        // first one's include is unsaved, but would clash once saved.
        let community = base.join("config/shaders/community/animation/test/shader.glsl");
        std::fs::create_dir_all(community.parent().unwrap()).unwrap();
        std::fs::write(
            &community,
            "vec4 animation(vec2 uv) { return vec4(0.0); }\n",
        )
        .unwrap();
        shell.shaders = shaders::scan(&base.join("config"), std::slice::from_ref(&data_dir));
        let err = assign_event(&mut shell, "layers", Some(&community)).unwrap_err();
        assert!(
            err.contains("already defines a preset named \"test\""),
            "{err}"
        );

        // Once saved, the preset files join the chain but aren't places
        // to save settings.
        for inc in &mut shell.includes.docs {
            inc.doc.save(&inc.path).unwrap();
        }
        let main = shell.path.clone();
        shell.doc.save(&main).unwrap();
        let shell = Shell::load(&main, &discovery::Env::from_process());
        assert_eq!(shell.includes.docs.len(), 3);
        assert_eq!(
            crate::slint_ui::save::save_destinations(&shell),
            vec!["config.toml (main)", "shaders.toml"]
        );
        std::fs::remove_dir_all(&base).ok();
    }
}
