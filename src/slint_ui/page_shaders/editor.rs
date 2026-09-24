//! The shader editor overlay: opening, the code pane (keys, undo, settling),
//! the unsaved-changes guard, and Save with its "Use for" step, rename and
//! delete.

use super::super::common::*;
use super::super::*;
use super::{builder::*, library::*, preview::*};

/// How long typing must pause before the code is re-checked.
pub(super) const CODE_SETTLE: std::time::Duration = std::time::Duration::from_millis(200);

/// The per-edit work, run once typing settles: lint note, builder sync
/// (the builder follows the code) and a preview compile.
pub(super) fn code_settled(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let text = app.get_shader_editor_text().to_string();
    let problems = shaders::lint_source(&text);
    let note = if problems.is_empty() {
        String::new()
    } else {
        format!("⚠ {}", problems.join("; "))
    };
    app.set_shader_editor_note(note.into());
    sync_builder_from_code(app, shell, &text);
    shell
        .borrow_mut()
        .preview_command(shader_preview::PreviewCommand::SetSource(text));
}

/// Run a pending settle now, so a builder action never acts on stale
/// code (typed a moment ago, not yet synced).
pub(super) fn settle_now(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let pending = shell.borrow().shader_code_settle.running();
    if pending {
        shell.borrow().shader_code_settle.stop();
        code_settled(app, shell);
    }
}

/// Open the overlay editor pre-loaded with a shader file's content.
/// `editing` reuses that exact file on save; forking leaves the source
/// untouched and saves under a new name.
pub(super) fn open_shader_editor(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    path: &str,
    editing: bool,
    name: String,
) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            app.set_status(format!("could not read {path}: {err}").into());
            return;
        }
    };
    shell.borrow_mut().shader_editing = editing.then(|| PathBuf::from(path));
    app.set_shader_editor_editing(editing);
    app.set_shader_editor_used_by(used_by(&shell.borrow(), Path::new(path)).into());
    app.set_shader_editor_name(name.into());
    sync_builder_from_code(app, shell, &text);
    app.set_shader_editor_text(text.into());
    app.set_shader_editor_note(String::new().into());
    show_editor(app, shell);
    kick_shader_preview(app, shell);
}

/// Show the editor overlay over whatever code is loaded, remembering it
/// as the unsaved-changes baseline.
pub(super) fn show_editor(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    // A settle left over from the last session would re-check old code.
    shell.borrow().shader_code_settle.stop();
    shell.borrow_mut().shader_editor_baseline = app.get_shader_editor_text().to_string();
    // A fresh undo history per opened shader.
    shell
        .borrow_mut()
        .shader_code_history
        .reset(app.get_shader_editor_text().as_str());
    shell.borrow_mut().shader_editor_baseline_name = app.get_shader_editor_name().to_string();
    app.set_shader_editor_confirm_close(false);
    app.set_shader_editor_open(true);
}

/// The "Use for" checklist: every event, what it uses now, ticked when
/// it already uses the shader being saved. A new or forked shader also
/// has the previewed event ticked; editing never pre-ticks an event the
/// shader doesn't already have, so a plain edit-and-save can't quietly
/// take one over.
pub(super) fn use_for_rows(shell: &Shell) -> Vec<ShaderUse> {
    let this = shell.shader_editing.as_deref().map(shaders::file_key);
    shaders::EVENTS
        .iter()
        .zip(resolved_assignments(shell))
        .enumerate()
        .map(|(index, (event, assigned))| {
            let uses_this = match (&assigned, &this) {
                (Some(assigned), Some(this)) => assigned.key == *this,
                _ => false,
            };
            let current = match &assigned {
                None => String::new(),
                Some(_) if uses_this => "uses this shader".to_owned(),
                Some(assigned) => {
                    let name = shell
                        .shaders
                        .iter()
                        .find(|entry| shaders::file_key(&entry.path) == assigned.key)
                        .map_or(assigned.value.as_str(), |entry| entry.name.as_str());
                    format!("uses {name}")
                }
            };
            ShaderUse {
                label: prettify(event).into(),
                current: current.into(),
                checked: uses_this || (this.is_none() && index == shell.shader_preview_event),
            }
        })
        .collect()
}

/// Apply the "Use for" checklist to the saved shader at `path`: ticked
/// events switch to it, unticked ones that used it are cleared, as
/// unsaved changes. Returns what changed, for the status line.
pub(super) fn apply_use_for(shell: &mut Shell, app: &AppWindow, path: &Path) -> Vec<String> {
    let rows = app.get_shader_use_for();
    let using: Vec<&str> = assignments_of(shell, path)
        .iter()
        .map(|(event, _)| *event)
        .collect();
    let mut changes = Vec::new();
    for (index, event) in shaders::EVENTS.iter().enumerate() {
        let Some(row) = rows.row_data(index) else {
            continue;
        };
        let uses = using.contains(event);
        if row.checked && !uses {
            assign_event(shell, event, Some(path));
            changes.push(format!("now used for {}", prettify(event)));
        } else if !row.checked && uses {
            assign_event(shell, event, None);
            changes.push(format!("no longer used for {}", prettify(event)));
        }
    }
    changes
}

pub(super) fn install(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_new(move || {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut shell = shell.borrow_mut();
                shell.shader_editing = None;
                shell.builder_steps = shaders::builder::default_steps();
            }
            app.set_shader_editor_editing(false);
            app.set_shader_editor_name(String::new().into());
            regen_builder(&app, &shell);
            app.set_shader_editor_note(String::new().into());
            show_editor(&app, &shell);
            kick_shader_preview(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // Editing loads one of the user's own shaders in place; forking
        // loads any shader's code into a fresh, unsaved editor.
        app.on_shader_editor_edit(move |path| {
            let Some(app) = weak.upgrade() else { return };
            let own = shell.borrow().shaders.iter().any(|entry| {
                entry.source == shaders::Source::ConfigDir
                    && entry.path.to_string_lossy() == path.as_str()
            });
            if !own {
                return;
            }
            let stem = Path::new(path.as_str())
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            open_shader_editor(&app, &shell, &path, true, stem);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_fork(move |path, name| {
            let Some(app) = weak.upgrade() else { return };
            // "reveal-fork", or "reveal-fork-2" when that's taken.
            let fork_name = match shell.borrow().path.parent() {
                Some(dir) => shaders::unused_shader_name(dir, &format!("{name}-fork")),
                None => format!("{name}-fork"),
            };
            open_shader_editor(&app, &shell, &path, false, fork_name);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // Save's first step: which events should use this shader.
        app.on_shader_editor_save_request(move || {
            let Some(app) = weak.upgrade() else { return };
            let rows = use_for_rows(&shell.borrow());
            app.set_shader_use_for(Rc::new(VecModel::from(rows)).into());
            app.set_shader_use_open(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_save(move |apply_use| {
            let Some(app) = weak.upgrade() else { return };
            let text = app.get_shader_editor_text().to_string();
            // Every successful save closes the editor; creating only
            // changes the status wording.
            let creating = shell.borrow().shader_editing.is_none();
            let name = app.get_shader_editor_name().to_string();
            // Events to repoint when an edited shader is renamed.
            let mut renamed: Vec<(&'static str, usize)> = Vec::new();
            let result = {
                let shell = shell.borrow();
                match shell.shader_editing.clone() {
                    // Save the code first, then rename: a failed rename
                    // still leaves the edits on disk under the old name.
                    Some(path) => match shell.path.parent().map(Path::to_path_buf) {
                        Some(dir) => shaders::write_user_shader(&path, &text).and_then(|()| {
                            let assigned = assignments_of(&shell, &path);
                            let new = shaders::rename_user_shader(&dir, &path, &name)?;
                            if new != path {
                                renamed = assigned;
                            }
                            Ok(new)
                        }),
                        None => Err("could not determine the config directory.".to_owned()),
                    },
                    None => match shell.path.parent().map(Path::to_path_buf) {
                        Some(dir) => shaders::save_user_shader(&dir, &name, &text),
                        None => Err("could not determine the config directory.".to_owned()),
                    },
                }
            };
            match result {
                Ok(path) => {
                    // What the "Use for" step switched on or off.
                    let use_changes = {
                        let mut shell = shell.borrow_mut();
                        shell.shader_editing = Some(path.clone());
                        shell.shader_editor_baseline = text.clone();
                        shell.shader_editor_baseline_name = name.clone();
                        // A renamed shader keeps its assignments: each is
                        // rewritten (unsaved) to the new file, spelled for
                        // the document it lives in.
                        let paths = chain_paths(&shell);
                        let repoints: Vec<_> = renamed
                            .iter()
                            .map(|(event, doc)| {
                                (*event, *doc, Some(shaders::value_for(&path, &paths[*doc])))
                            })
                            .collect();
                        if let Err(err) = write_assignments(&mut shell, &repoints) {
                            app.set_status(err.into());
                        }
                        let changes = if apply_use {
                            apply_use_for(&mut shell, &app, &path)
                        } else {
                            Vec::new()
                        };
                        scan_shaders(&mut shell);
                        changes
                    };
                    let shell = shell.borrow();
                    app.set_dirty(shell.any_modified());
                    rebuild_shaders(&app, &shell);
                    app.set_shader_editor_note(String::new().into());
                    app.set_shader_editor_used_by(used_by(&shell, &path).into());
                    app.set_shader_editor_open(false);
                    let mut status = if creating {
                        format!("Created {}.", path.display())
                    } else if !renamed.is_empty() {
                        let events: Vec<String> =
                            renamed.iter().map(|(event, _)| prettify(event)).collect();
                        format!(
                            "Saved as {} and pointed {} at it.",
                            path.display(),
                            events.join(", ")
                        )
                    } else {
                        format!("Saved {} — umbriel live-reloads it.", path.display())
                    };
                    if !use_changes.is_empty() {
                        status.push_str(&format!(
                            " {}: save your config to apply.",
                            use_changes.join(", ")
                        ));
                    }
                    app.set_status(status.into());
                }
                Err(err) => app.set_shader_editor_note(err.into()),
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_delete(move || {
            let Some(app) = weak.upgrade() else { return };
            let Some(path) = shell.borrow().shader_editing.clone() else {
                return;
            };
            delete_shader(&app, &shell, &path);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_editor_text_changed(move |text, anchor, cursor, kind| {
            let Some(app) = weak.upgrade() else { return };
            // Typing (0) and key edits (1) are undo steps; a restore (2)
            // came from the history itself.
            if kind != 2 {
                let offset = |value: i32| usize::try_from(value).unwrap_or(0);
                shell.borrow_mut().shader_code_history.record(
                    shaders::code_edit::Snapshot {
                        text: text.to_string(),
                        anchor: offset(anchor),
                        cursor: offset(cursor),
                    },
                    kind == 0,
                    std::time::Instant::now(),
                );
            }
            // Restart the settle timer; the work runs once typing pauses.
            let weak = app.as_weak();
            let settle_shell = Rc::downgrade(&shell);
            shell.borrow().shader_code_settle.start(
                slint::TimerMode::SingleShot,
                CODE_SETTLE,
                move || {
                    if let (Some(app), Some(shell)) = (weak.upgrade(), settle_shell.upgrade()) {
                        code_settled(&app, &shell);
                    }
                },
            );
        });
    }
    {
        let shell = Rc::clone(shell);
        app.on_shader_code_history(move |kind| {
            let mut shell = shell.borrow_mut();
            let history = &mut shell.shader_code_history;
            let step = if kind.as_str() == "redo" {
                history.redo()
            } else {
                history.undo()
            };
            match step {
                Some(snapshot) => CodeEdit {
                    text: snapshot.text.into(),
                    anchor: snapshot.anchor as i32,
                    cursor: snapshot.cursor as i32,
                },
                None => CodeEdit {
                    text: SharedString::new(),
                    anchor: -1,
                    cursor: -1,
                },
            }
        });
    }
    // Code-editor keys: pure text surgery, see shaders::code_edit.
    app.on_shader_code_key(|text, anchor, cursor, kind| {
        let key = match kind.as_str() {
            "outdent" => shaders::code_edit::Key::Outdent,
            "newline" => shaders::code_edit::Key::Newline,
            _ => shaders::code_edit::Key::Indent,
        };
        let offset = |value: i32| usize::try_from(value).unwrap_or(0);
        let (text, anchor, cursor) =
            shaders::code_edit::apply(&text, offset(anchor), offset(cursor), key);
        CodeEdit {
            text: text.into(),
            anchor: anchor as i32,
            cursor: cursor as i32,
        }
    });
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // Cancel and the scrim both land here: unsaved code asks first.
        app.on_shader_editor_close(move || {
            let Some(app) = weak.upgrade() else { return };
            let unsaved = {
                let shell = shell.borrow();
                app.get_shader_editor_text().as_str() != shell.shader_editor_baseline
                    || app.get_shader_editor_name().as_str() != shell.shader_editor_baseline_name
            };
            if unsaved {
                app.set_shader_editor_confirm_close(true);
            } else {
                app.set_shader_editor_open(false);
            }
        });
    }
}
