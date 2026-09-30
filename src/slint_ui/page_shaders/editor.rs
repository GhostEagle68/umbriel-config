//! The shader editor page: opening, the code pane (keys, undo, settling),
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
    // Mid-typing code with no entry point keeps the kind it had.
    if let Some(kind) = shaders::entry_kind(&text) {
        app.set_shader_editor_kind(kind_index(kind));
    }
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

/// Open the editor pre-loaded with a shader file's content.
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
            alert(app, "Couldn't open the shader", format!("{path}: {err}"));
            return;
        }
    };
    shell.borrow_mut().shader_editing = editing.then(|| PathBuf::from(path));
    app.set_shader_editor_editing(editing);
    app.set_shader_editor_used_by(used_by(&shell.borrow(), Path::new(path)).into());
    app.set_shader_editor_name(name.into());
    sync_builder_from_code(app, shell, &text);
    app.set_shader_editor_kind(kind_index(shaders::kind_of(&text)));
    app.set_shader_editor_text(text.into());
    app.set_shader_editor_note(String::new().into());
    show_editor(app, shell);
    kick_shader_preview(app, shell);
}

/// Open the editor on a fresh, unsaved shader of `kind`: the builder's
/// starter stack for animations, that kind's template otherwise.
fn new_shader(app: &AppWindow, shell: &Rc<RefCell<Shell>>, kind: &str) {
    shell.borrow_mut().shader_editing = None;
    app.set_shader_editor_editing(false);
    app.set_shader_editor_kind(kind_index(kind));
    if kind == shaders::KINDS[0] {
        shell.borrow_mut().builder_steps = shaders::builder::default_steps();
        regen_builder(app, shell);
    } else {
        app.set_shader_editor_text(shaders::scaffold(kind).into());
    }
    app.set_shader_editor_note(String::new().into());
    show_editor(app, shell);
    kick_shader_preview(app, shell);
}

/// The [`shaders::EVENTS`] slots (with their index) that take shaders
/// of `kind`.
fn slots_of_kind(kind: &str) -> impl Iterator<Item = (usize, &'static str)> + '_ {
    shaders::EVENTS
        .iter()
        .copied()
        .enumerate()
        .filter(move |(_, event)| shaders::slot_kind(event) == kind)
}

/// Show the editor page over whatever code is loaded, remembering it
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

/// The "Use for" checklist: every slot that takes a `kind` shader, what
/// it uses now, ticked when it already uses the shader being saved. A
/// new or forked shader also has the previewed event ticked; editing
/// never pre-ticks an event the shader doesn't already have, so a plain
/// edit-and-save can't quietly take one over.
pub(super) fn use_for_rows(shell: &Shell, kind: &str) -> Vec<ShaderUse> {
    let this = shell.shader_editing.as_deref().map(shaders::file_key);
    let resolved = resolved_assignments(shell);
    slots_of_kind(kind)
        .map(|(index, event)| {
            let assigned = &resolved[index];
            let uses_this = match (assigned, &this) {
                (Some(assigned), Some(this)) => assigned.key == *this,
                _ => false,
            };
            let current = match assigned {
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
                label: slot_label(event).into(),
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
    let kind = shaders::kind_of(&app.get_shader_editor_text());
    let mut changes = Vec::new();
    for (row_index, (_, event)) in slots_of_kind(kind).enumerate() {
        let Some(row) = rows.row_data(row_index) else {
            continue;
        };
        let uses = using.contains(&event);
        if row.checked && !uses {
            match assign_event(shell, event, Some(path)) {
                Ok(()) => changes.push(format!("now used for {}", slot_label(event))),
                Err(err) => changes.push(format!("not used for {}: {err}", slot_label(event))),
            }
        } else if !row.checked && uses {
            // Clearing never fails.
            let _ = assign_event(shell, event, None);
            changes.push(format!("no longer used for {}", slot_label(event)));
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
            app.set_shader_editor_name(String::new().into());
            new_shader(&app, &shell, shaders::KINDS[0]);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // A new shader's kind picker: the kind's template replaces the
        // code, so only while the code is still the untouched template.
        app.on_shader_editor_kind_picked(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let text = app.get_shader_editor_text();
            let kind = usize::try_from(index)
                .ok()
                .and_then(|index| shaders::KINDS.get(index));
            match kind {
                Some(kind) if text.as_str() == shell.borrow().shader_editor_baseline => {
                    new_shader(&app, &shell, kind);
                }
                _ => {
                    app.set_shader_editor_kind(kind_index(shaders::kind_of(&text)));
                    app.set_shader_editor_note(
                        "The kind can only change before you edit the code.".into(),
                    );
                }
            }
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
            let kind = shaders::kind_of(&app.get_shader_editor_text());
            let rows = use_for_rows(&shell.borrow(), kind);
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
            // Events to repoint when an edited shader is renamed, and
            // the shader's old path.
            let mut renamed: Vec<(&'static str, usize)> = Vec::new();
            let mut renamed_from: Option<PathBuf> = None;
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
                                renamed_from = Some(path.clone());
                            }
                            shaders::sync_preset_kind(&new, shaders::kind_of(&text))?;
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
                        forget_thumbnail(&mut shell, &path);
                        shell.shader_editing = Some(path.clone());
                        shell.shader_editor_baseline = text.clone();
                        shell.shader_editor_baseline_name = name.clone();
                        // A renamed shader keeps its assignments: each
                        // selects the preset under its new name, and the
                        // include follows the renamed preset file.
                        scan_shaders(&mut shell);
                        let renamed_to = shell
                            .shaders
                            .iter()
                            .find(|entry| entry.path == path)
                            .map(|entry| (entry.preset.clone(), entry.preset_file.clone()));
                        if let (Some((preset, preset_file)), Some(old)) =
                            (renamed_to, renamed_from.as_deref())
                        {
                            let repoints: Vec<_> = renamed
                                .iter()
                                .map(|(event, doc)| (*event, *doc, Some(preset.clone())))
                                .collect();
                            if let Err(err) = write_assignments(&app, &mut shell, &repoints)
                                .and_then(|()| {
                                    repoint_include(
                                        &mut shell,
                                        &shaders::preset_file_for(old),
                                        Some(&preset_file),
                                    )
                                })
                            {
                                toast(&app, ToastKind::Error, err, "");
                            }
                        }
                        // An edit that changed the shader's kind strands the
                        // slots of the old kind: umbriel drops a preset of
                        // the wrong kind, so they stop using it.
                        let kind = shaders::kind_of(&text);
                        let mut changes = Vec::new();
                        for (event, _) in assignments_of(&shell, &path) {
                            if shaders::slot_kind(event) != kind {
                                let _ = assign_event(&mut shell, event, None);
                                changes.push(format!(
                                    "no longer used for {} (now a {kind} shader)",
                                    slot_label(event)
                                ));
                            }
                        }
                        if apply_use {
                            changes.extend(apply_use_for(&mut shell, &app, &path));
                        }
                        scan_shaders(&mut shell);
                        changes
                    };
                    request_thumbnails(&app, &shell);
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
                            renamed.iter().map(|(event, _)| slot_label(event)).collect();
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
                    toast(&app, ToastKind::Success, status, "");
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
        // Back lands here: unsaved code asks first.
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
