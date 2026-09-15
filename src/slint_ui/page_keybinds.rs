//! The Keybinds page: merged binds across the chain plus umbriel's
//! defaults, with the add/edit/remove editor.

use super::common::*;
use super::*;

/// The file a merged bind lives in, for the list's source label.
fn keybind_source(shell: &Shell, file: usize) -> String {
    if file >= shell.includes.docs.len() {
        file_name_of(&shell.path)
    } else {
        file_name_of(&shell.includes.docs[file].path)
    }
}

/// Non-default extras worth showing on a row.
pub(super) fn keybind_extras(bind: &keybinds::SourcedBind) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if bind.repeat == Some(false) {
        parts.push("once");
    }
    if bind.allow_when_locked == Some(true) {
        parts.push("works when locked");
    }
    if bind.submap.is_some() {
        parts.push("submap");
    }
    parts.join(" · ")
}

/// Rebuild the Keybinds page rows (filtered by `filter`) and keep the
/// merged bind list in sync for the edit/remove handlers.
pub(super) fn rebuild_keybind_rows(
    app: &AppWindow,
    shell: &Shell,
    kb_actions: &Arc<Mutex<Vec<keybinds::LiveAction>>>,
    filter: &str,
    binds_out: &Rc<RefCell<Vec<keybinds::SourcedBind>>>,
) {
    let actions = kb_actions.lock().expect("kb actions").clone();
    let docs = chain_docs(shell);
    let merged = keybinds::merged_binds(&docs);
    let lowered = filter.trim().to_lowercase();

    struct KbEntry {
        family: &'static str,
        chord: String,
        summary: String,
        extras: String,
        source: String,
        is_user: bool,
        shadowing: bool,
        bind: usize,
    }
    let mut entries: Vec<KbEntry> = Vec::new();
    for (bind_index, bind) in merged.iter().enumerate() {
        let shadowing = keybinds::DEFAULT_BINDS
            .iter()
            .any(|(chord, _)| chord.eq_ignore_ascii_case(&bind.chord));
        entries.push(KbEntry {
            family: keybinds::action_group(&bind.action),
            chord: bind.chord.clone(),
            summary: keybinds::describe(&bind.action, &actions),
            extras: keybind_extras(bind),
            source: keybind_source(shell, bind.source_file),
            is_user: true,
            shadowing,
            bind: bind_index,
        });
    }
    for (chord, action) in keybinds::DEFAULT_BINDS {
        if merged
            .iter()
            .any(|bind| bind.chord.eq_ignore_ascii_case(chord))
        {
            continue; // a user bind shadows this default and is shown instead
        }
        entries.push(KbEntry {
            family: keybinds::action_group(action),
            chord: (*chord).to_owned(),
            summary: keybinds::describe(action, &actions),
            extras: String::new(),
            source: String::new(),
            is_user: false,
            shadowing: false,
            bind: usize::MAX,
        });
    }
    entries.retain(|entry| {
        lowered.is_empty()
            || entry.chord.to_lowercase().contains(&lowered)
            || entry.summary.to_lowercase().contains(&lowered)
    });
    entries.sort_by(|a, b| {
        let rank = |family: &str| {
            keybinds::GROUP_ORDER
                .iter()
                .position(|known| *known == family)
                .unwrap_or(keybinds::GROUP_ORDER.len())
        };
        rank(a.family)
            .cmp(&rank(b.family))
            .then_with(|| a.chord.cmp(&b.chord))
    });

    let mut rows: Vec<KeybindRow> = Vec::new();
    let mut last_family = String::new();
    for entry in entries {
        if entry.family != last_family {
            last_family = entry.family.to_owned();
            rows.push(KeybindRow {
                chord: entry.family.to_uppercase().into(),
                summary: String::new().into(),
                extras: String::new().into(),
                source: String::new().into(),
                is_header: true,
                is_user: false,
                shadowing: false,
                index: -1,
            });
        }
        rows.push(KeybindRow {
            chord: entry.chord.into(),
            summary: entry.summary.into(),
            extras: entry.extras.into(),
            source: entry.source.into(),
            is_header: false,
            is_user: entry.is_user,
            shadowing: entry.shadowing,
            index: entry.bind as i32,
        });
    }
    app.set_keybind_rows(Rc::new(VecModel::from(rows)).into());
    *binds_out.borrow_mut() = merged;
}

pub(super) fn install_keybinds(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    kb_actions: &Arc<Mutex<Vec<keybinds::LiveAction>>>,
    keybind_binds: &Rc<RefCell<Vec<keybinds::SourcedBind>>>,
) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let keybind_binds = Rc::clone(keybind_binds);
        let kb_actions = Arc::clone(kb_actions);
        app.on_keybind_search_edited(move |text| {
            let Some(app) = weak.upgrade() else { return };
            let shell = shell.borrow();
            rebuild_keybind_rows(&app, &shell, &kb_actions, &text, &keybind_binds);
        });
    }
    {
        let weak = app.as_weak();
        app.on_keybind_add(move || {
            let Some(app) = weak.upgrade() else { return };
            app.set_keybind_editing(false);
            app.set_kb_original_index(-1);
            app.set_kb_conflict_note("".into());
            app.set_kb_draft_chord("".into());
            app.set_kb_draft_use_mod(true);
            app.set_kb_draft_scope("".into());
            app.set_kb_draft_action(0);
            app.set_kb_draft_param("".into());
            app.set_kb_draft_no_repeat(false);
            app.set_kb_draft_locked(false);
            app.set_kb_draft_submap("".into());
            app.set_kb_editor_open(true);
        });
    }
    {
        let weak = app.as_weak();
        let keybind_binds = Rc::clone(keybind_binds);
        app.on_keybind_edit(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let bind = keybind_binds.borrow().get(index as usize).cloned();
            let Some(bind) = bind else { return };
            let draft = keybinds::BindDraft::from_parts(
                &bind.chord,
                &bind.action,
                bind.repeat,
                bind.allow_when_locked,
                bind.submap.clone(),
            );
            let (name, param) = bind.action.split_once(':').unwrap_or((&bind.action, ""));
            let choices = app.get_action_choices();
            let mut names: Vec<String> = (0..choices.row_count())
                .filter_map(|i| choices.row_data(i).map(|s| s.to_string()))
                .collect();
            let action_index = match names.iter().position(|candidate| candidate == name) {
                Some(position) => position,
                None => {
                    // An action outside the vocabulary (a newer umbriel):
                    // surface it so the edit still round-trips.
                    names.push(name.to_owned());
                    let model: Vec<SharedString> = names
                        .iter()
                        .map(|n| SharedString::from(n.clone()))
                        .collect();
                    app.set_action_choices(Rc::new(VecModel::from(model)).into());
                    names.len() - 1
                }
            };
            app.set_keybind_editing(true);
            app.set_kb_original_index(index);
            app.set_kb_original_chord(bind.chord.clone().into());
            app.set_kb_draft_chord(draft.chord.into());
            app.set_kb_draft_use_mod(draft.use_mod);
            app.set_kb_draft_scope(draft.scope.into());
            app.set_kb_draft_action(action_index as i32);
            app.set_kb_draft_param(param.into());
            app.set_kb_draft_no_repeat(bind.repeat == Some(false));
            app.set_kb_draft_locked(bind.allow_when_locked == Some(true));
            app.set_kb_draft_submap(bind.submap.clone().unwrap_or_default().into());
            app.set_kb_conflict_note("".into());
            app.set_kb_editor_open(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let keybind_binds = Rc::clone(keybind_binds);
        let kb_actions = Arc::clone(kb_actions);
        app.on_keybind_remove(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let bind = keybind_binds.borrow().get(index as usize).cloned();
            let Some(bind) = bind else { return };
            let mut shell = shell.borrow_mut();
            let removed = if bind.source_file >= shell.includes.docs.len() {
                shell.doc.remove_keybind(&bind.chord)
            } else {
                shell.includes.docs[bind.source_file]
                    .doc
                    .remove_keybind(&bind.chord)
            };
            if removed {
                rebuild_keybind_rows(
                    &app,
                    &shell,
                    &kb_actions,
                    &app.get_keybind_search(),
                    &keybind_binds,
                );
                app.set_dirty(shell.any_modified());
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let keybind_binds = Rc::clone(keybind_binds);
        let kb_actions = Arc::clone(kb_actions);
        app.on_keybind_apply(move || {
            let Some(app) = weak.upgrade() else { return };
            let chord_body = app.get_kb_draft_chord().trim().to_owned();
            if chord_body.is_empty() {
                app.set_kb_conflict_note("Enter a chord, e.g. Return with Mod on.".into());
                return;
            }
            let action_index = app.get_kb_draft_action().max(0) as usize;
            let choices = app.get_action_choices();
            let Some(action_name) = choices.row_data(action_index) else {
                app.set_kb_conflict_note("Pick an action.".into());
                return;
            };
            let param = app.get_kb_draft_param().trim().to_owned();
            let action = if param.is_empty() {
                action_name.to_string()
            } else {
                format!("{action_name}:{param}")
            };
            let submap = app.get_kb_draft_submap().trim().to_owned();
            let draft = keybinds::BindDraft {
                chord: chord_body,
                use_mod: app.get_kb_draft_use_mod(),
                scope: app.get_kb_draft_scope().trim().to_owned(),
                action: action.clone(),
                repeat: if app.get_kb_draft_no_repeat() {
                    Some(false)
                } else {
                    None
                },
                allow_when_locked: if app.get_kb_draft_locked() {
                    Some(true)
                } else {
                    None
                },
                submap: (!submap.is_empty()).then_some(submap),
            };
            let chord = draft.composed_chord();
            let editing = app.get_keybind_editing();
            let original = app.get_kb_original_chord().to_string();
            {
                let shell = shell.borrow();
                let docs = chain_docs(&shell);
                if let Some((_file, other)) =
                    keybinds::find_conflict(&docs, &chord, editing.then_some(original.as_str()))
                {
                    app.set_kb_conflict_note(
                        format!("Already bound to {other} — pick another chord.").into(),
                    );
                    return;
                }
            }
            let mut shell = shell.borrow_mut();
            let write = |doc: &mut ConfigDocument| {
                if !original.eq_ignore_ascii_case(&chord) {
                    doc.remove_keybind(&original);
                }
                doc.set_keybind(
                    &chord,
                    &action,
                    draft.repeat,
                    draft.allow_when_locked,
                    draft.submap.as_deref(),
                );
            };
            if editing {
                let index = app.get_kb_original_index().max(0) as usize;
                let source = keybind_binds
                    .borrow()
                    .get(index)
                    .map(|bind| bind.source_file);
                match source {
                    Some(source) if source >= shell.includes.docs.len() => {
                        write(&mut shell.doc);
                    }
                    Some(source) => {
                        write(&mut shell.includes.docs[source].doc);
                    }
                    None => return,
                }
            } else {
                // New binds land where [keybinds] already lives: the main
                // file if it has binds, else the first include that does,
                // else the main file (created on save).
                let main_owns = !shell.doc.keybinds().is_empty();
                let include_owns = (!main_owns).then(|| {
                    shell
                        .includes
                        .docs
                        .iter()
                        .position(|inc| !inc.doc.keybinds().is_empty())
                });
                match include_owns {
                    Some(Some(index)) => write(&mut shell.includes.docs[index].doc),
                    _ => write(&mut shell.doc),
                }
            }
            app.set_kb_editor_open(false);
            rebuild_keybind_rows(
                &app,
                &shell,
                &kb_actions,
                &app.get_keybind_search(),
                &keybind_binds,
            );
            app.set_dirty(shell.any_modified());
        });
    }
}
