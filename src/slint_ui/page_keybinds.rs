//! The Keybinds page: merged binds across the chain plus umbriel's
//! defaults, with the inline add/edit/override editor. Drives the
//! `KeybindsState` global.

use super::common::*;
use super::*;
use std::collections::HashSet;
use umbriel_config::config::apps;
use umbriel_config::config::document::KeybindEntry;

/// Page state that doesn't belong in the window: the merged binds behind
/// the rows (for edit/remove), which groups are collapsed, and the bind
/// the editor is changing.
#[derive(Default)]
pub(super) struct KeybindView {
    binds: Vec<keybinds::SourcedBind>,
    collapsed: HashSet<String>,
    /// The search came from recording keys: match by key, not text.
    pub exact: bool,
    /// Index into `binds` and file-exact chord of the bind being edited;
    /// None for a new bind or an override of a default.
    target: Option<(usize, String)>,
    /// Installed apps, scanned the first time the picker opens.
    apps: Option<Vec<apps::App>>,
}

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
    if bind.allow_when_inhibited == Some(true) {
        parts.push("works when inhibited");
    }
    let cooldown = bind.cooldown_ms.map(|ms| format!("{ms} ms cooldown"));
    parts.extend(cooldown.as_deref());
    if bind.submap.is_some() {
        parts.push("submap");
    }
    parts.join(" · ")
}

/// A row's text: a spawn bind shows its command (the group already says
/// it launches something), everything else the action's summary.
fn row_summary(action: &str, actions: &[keybinds::LiveAction]) -> (String, bool) {
    match action.split_once(':') {
        Some(("spawn", command)) => (command.to_owned(), true),
        _ => (keybinds::describe(action, actions), false),
    }
}

/// Rebuild the Keybinds page rows from the search, filter, and collapsed
/// groups, and keep the merged bind list in sync for the handlers.
pub(super) fn rebuild_keybind_rows(
    app: &AppWindow,
    shell: &Shell,
    kb_actions: &Arc<Mutex<Vec<keybinds::LiveAction>>>,
    view: &Rc<RefCell<KeybindView>>,
) {
    let state = app.global::<KeybindsState>();
    let actions = kb_actions.lock().expect("kb actions").clone();
    let docs = chain_docs(shell);
    let merged = keybinds::merged_binds(&docs);
    let mut view = view.borrow_mut();
    let lowered = state.get_search().trim().to_lowercase();
    let filter = state.get_filter();
    // Name the file only when binds come from more than one.
    let files: HashSet<usize> = merged.iter().map(|bind| bind.source_file).collect();
    let multi_file = files.len() > 1;

    struct KbEntry {
        family: &'static str,
        chord: String,
        summary: String,
        is_command: bool,
        extras: String,
        source: String,
        is_user: bool,
        shadowing: bool,
        bind: i32,
    }
    let mut entries: Vec<KbEntry> = Vec::new();
    if filter != 2 {
        for (bind_index, bind) in merged.iter().enumerate() {
            let shadowing = keybinds::DEFAULT_BINDS
                .iter()
                .any(|(chord, _)| chord.eq_ignore_ascii_case(&bind.chord));
            let (summary, is_command) = row_summary(&bind.action, &actions);
            let mut extras = keybind_extras(bind);
            if shadowing {
                if !extras.is_empty() {
                    extras.push_str(" · ");
                }
                extras.push_str("replaces default");
            }
            entries.push(KbEntry {
                family: keybinds::bind_group(&bind.chord, &bind.action),
                chord: bind.chord.clone(),
                summary,
                is_command,
                extras,
                source: if multi_file {
                    keybind_source(shell, bind.source_file)
                } else {
                    String::new()
                },
                is_user: true,
                shadowing,
                bind: bind_index as i32,
            });
        }
    }
    if filter != 1 {
        for (chord, action) in keybinds::DEFAULT_BINDS {
            if merged
                .iter()
                .any(|bind| bind.chord.eq_ignore_ascii_case(chord))
            {
                continue; // a user bind shadows this default and is shown instead
            }
            let (summary, is_command) = row_summary(action, &actions);
            entries.push(KbEntry {
                family: keybinds::bind_group(chord, action),
                chord: (*chord).to_owned(),
                summary,
                is_command,
                extras: String::new(),
                source: String::new(),
                is_user: false,
                shadowing: false,
                bind: -1,
            });
        }
    }
    entries.retain(|entry| {
        if view.exact {
            keybinds::chord_matches(&entry.chord, &lowered)
        } else {
            lowered.is_empty()
                || entry.chord.to_lowercase().contains(&lowered)
                || entry.summary.to_lowercase().contains(&lowered)
        }
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

    // A search shows every match; collapsing only tidies the full list.
    let searching = !lowered.is_empty();
    let mut rows: Vec<KeybindRow> = Vec::new();
    for (position, entry) in entries.iter().enumerate() {
        let first_of_group = position == 0 || entries[position - 1].family != entry.family;
        let expanded = searching || !view.collapsed.contains(entry.family);
        if first_of_group {
            let count = entries.iter().filter(|e| e.family == entry.family).count();
            rows.push(KeybindRow {
                chord: entry.family.to_uppercase().into(),
                is_header: true,
                expanded,
                count: count as i32,
                index: -1,
                ..Default::default()
            });
        }
        if expanded {
            rows.push(KeybindRow {
                chord: entry.chord.clone().into(),
                summary: entry.summary.clone().into(),
                extras: entry.extras.clone().into(),
                source: entry.source.clone().into(),
                is_user: entry.is_user,
                shadowing: entry.shadowing,
                is_command: entry.is_command,
                index: entry.bind,
                ..Default::default()
            });
        }
    }
    let note = match files.iter().next() {
        Some(file) if !multi_file => format!(
            "Bold keys are yours (in {}); grey ones are Umbriel's defaults. Click a row to change it.",
            keybind_source(shell, *file)
        ),
        _ => "Bold keys are yours; grey ones are Umbriel's defaults. Click a row to change it."
            .to_owned(),
    };
    state.set_list_note(note.into());
    state.set_rows(Rc::new(VecModel::from(rows)).into());
    view.binds = merged;
}

/// The action picker's value for spawn used as "launch an app"; plain
/// "spawn" is "run a shell command". Both save as `spawn:<command>`.
const LAUNCH_APP: &str = "spawn:app";

/// Installed apps, scanned on first use.
fn apps_of(view: &mut KeybindView) -> &[apps::App] {
    view.apps
        .get_or_insert_with(|| apps::installed_apps(&discovery::Env::from_process()))
}

/// Point the editor at `name` (an action, or LAUNCH_APP) and its
/// parameter, shaping the parameter field from the action's spec.
fn set_draft_action(
    state: &KeybindsState,
    actions: &[keybinds::LiveAction],
    name: &str,
    param: &str,
) {
    match name {
        LAUNCH_APP => {
            state.set_draft_action("spawn".into());
            state.set_draft_action_summary("Launch an app".into());
            state.set_draft_param(param.into());
            state.set_param_input(ParamInput::App);
            return;
        }
        "spawn" => {
            state.set_draft_action("spawn".into());
            state.set_draft_action_summary("Run a shell command".into());
            state.set_draft_param(param.into());
            state.set_param_input(ParamInput::Command);
            return;
        }
        _ => {}
    }
    let live = actions.iter().find(|action| action.name == name);
    state.set_draft_action(name.into());
    state.set_draft_action_summary(
        live.map(|action| action.summary.clone())
            .unwrap_or_default()
            .into(),
    );
    state.set_draft_param(param.into());
    // An action this umbriel doesn't list still round-trips as free text.
    let kind = live.map_or(keybinds::ParamKind::Text(String::new()), |action| {
        keybinds::param_kind(&action.param)
    });
    let input = match kind {
        keybinds::ParamKind::None => ParamInput::None,
        keybinds::ParamKind::Command => ParamInput::Command,
        keybinds::ParamKind::Text(hint) => {
            state.set_param_hint(hint.into());
            ParamInput::Text
        }
        keybinds::ParamKind::Choice(choices) => {
            // The combo can't show "nothing selected": start on the first
            // choice so what's shown is what saves.
            let index = choices.iter().position(|c| c == param).unwrap_or(0);
            state.set_draft_param(choices[index].clone().into());
            let labels: Vec<SharedString> = choices
                .iter()
                .map(|c| if c.is_empty() { "none" } else { c.as_str() }.into())
                .collect();
            state.set_param_choices(Rc::new(VecModel::from(labels)).into());
            state.set_param_choice_index(index as i32);
            ParamInput::Choice
        }
    };
    state.set_param_input(input);
}

/// A spawn bind whose command is an installed app's (or still empty)
/// edits as "launch an app"; any other command stays a shell command.
fn prefer_app_mode(
    state: &KeybindsState,
    actions: &[keybinds::LiveAction],
    apps: &[apps::App],
    action: &str,
) {
    let Some(command) = action
        .strip_prefix("spawn:")
        .or((action == "spawn").then_some(""))
    else {
        return;
    };
    let app = apps.iter().find(|app| app.command == command);
    if command.is_empty() || app.is_some() {
        set_draft_action(state, actions, LAUNCH_APP, command);
        state.set_draft_app_name(app.map(|app| app.name.clone()).unwrap_or_default().into());
    }
}

/// Open the editor under `anchor` with a fresh draft.
fn open_editor(
    state: &KeybindsState,
    actions: &[keybinds::LiveAction],
    title: &str,
    anchor: &str,
    chord: &str,
    action: &str,
    bind: Option<&keybinds::SourcedBind>,
) {
    let (name, param) = action.split_once(':').unwrap_or((action, ""));
    let (scope, body) = keybinds::split_scope(chord);
    state.set_editor_title(title.into());
    state.set_editor_anchor(anchor.into());
    state.set_draft_chord(body.into());
    state.set_draft_scope(scope.into());
    set_draft_action(state, actions, name, param);
    state.set_draft_no_repeat(bind.is_some_and(|bind| bind.repeat == Some(false)));
    state.set_draft_locked(bind.is_some_and(|bind| bind.allow_when_locked == Some(true)));
    state.set_draft_inhibited(bind.is_some_and(|bind| bind.allow_when_inhibited == Some(true)));
    let cooldown = bind
        .and_then(|bind| bind.cooldown_ms)
        .map(|ms| ms.to_string())
        .unwrap_or_default();
    let submap = bind
        .and_then(|bind| bind.submap.clone())
        .unwrap_or_default();
    state.set_advanced_open(!scope.is_empty() || !submap.is_empty() || !cooldown.is_empty());
    state.set_draft_cooldown(cooldown.into());
    state.set_draft_submap(submap.into());
    state.set_conflict_note("".into());
    state.set_recording(0);
    state.set_typing_chord(false);
    state.set_action_picker_open(false);
    state.set_app_picker_open(false);
    state.set_editor_open(true);
}

/// A key event as chord text. Pressing a modifier alone yields "" (keep
/// listening); releasing one alone yields a modifier-only chord.
fn key_chord(text: &str, meta: bool, ctrl: bool, alt: bool, shift: bool, released: bool) -> String {
    use slint::platform::Key;
    let is = |key: Key| text == SharedString::from(key).as_str();
    let modifier = [
        (Key::Meta, "Mod"),
        (Key::MetaR, "Mod"),
        (Key::Control, "Ctrl"),
        (Key::ControlR, "Ctrl"),
        (Key::Alt, "Alt"),
        (Key::AltGr, "Alt"),
        (Key::Shift, "Shift"),
        (Key::ShiftR, "Shift"),
    ]
    .into_iter()
    .find(|(key, _)| is(*key))
    .map(|(_, name)| name);
    let mut parts: Vec<&str> = [
        (meta, "Mod"),
        (ctrl, "Ctrl"),
        (alt, "Alt"),
        (shift, "Shift"),
    ]
    .into_iter()
    .filter_map(|(held, name)| held.then_some(name))
    .collect();
    if let Some(modifier) = modifier {
        if !released {
            return String::new();
        }
        if !parts.contains(&modifier) {
            parts.push(modifier);
        }
        return parts.join("+");
    }
    if released || text.is_empty() {
        return String::new();
    }
    let named = [
        (Key::Return, "Return"),
        // Reaches here only with a modifier held; plain Esc cancels.
        (Key::Escape, "Escape"),
        (Key::Tab, "Tab"),
        (Key::Backspace, "BackSpace"),
        (Key::Delete, "Delete"),
        (Key::Insert, "Insert"),
        (Key::Space, "space"),
        (Key::LeftArrow, "Left"),
        (Key::RightArrow, "Right"),
        (Key::UpArrow, "Up"),
        (Key::DownArrow, "Down"),
        (Key::Home, "Home"),
        (Key::End, "End"),
        (Key::PageUp, "Page_Up"),
        (Key::PageDown, "Page_Down"),
        (Key::F1, "F1"),
        (Key::F2, "F2"),
        (Key::F3, "F3"),
        (Key::F4, "F4"),
        (Key::F5, "F5"),
        (Key::F6, "F6"),
        (Key::F7, "F7"),
        (Key::F8, "F8"),
        (Key::F9, "F9"),
        (Key::F10, "F10"),
        (Key::F11, "F11"),
        (Key::F12, "F12"),
    ]
    .into_iter()
    .find(|(key, _)| is(*key))
    .map(|(_, name)| name.to_owned());
    let key = match named {
        Some(name) => name,
        // Other keys Slint names (Caps Lock, Escape, media keys…) are
        // control or private-use characters with no chord spelling; ignore
        // them rather than write garbage.
        None if text
            .chars()
            .any(|ch| ch.is_control() || ('\u{f700}'..='\u{f8ff}').contains(&ch)) =>
        {
            return String::new();
        }
        None => {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) => keybinds::key_name(ch),
                _ => text.to_uppercase(),
            }
        }
    };
    parts.push(&key);
    parts.join("+")
}

pub(super) fn install_keybinds(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    kb_actions: &Arc<Mutex<Vec<keybinds::LiveAction>>>,
    view: &Rc<RefCell<KeybindView>>,
) {
    let state = app.global::<KeybindsState>();
    state.set_chord_hint(keybinds::CHORD_HINT.into());
    let special: Vec<SharedString> = std::iter::once("Add a key…".to_owned())
        .chain(keybinds::key_choices().into_iter().map(|(key, label)| {
            if key == label {
                label
            } else {
                format!("{label} ({key})")
            }
        }))
        .map(SharedString::from)
        .collect();
    state.set_special_keys(Rc::new(VecModel::from(special)).into());
    state.on_chord_keys(|chord| {
        let (scope, body) = keybinds::split_scope(&chord);
        let keys: Vec<SharedString> = (!scope.is_empty())
            .then(|| format!("{scope}:"))
            .into_iter()
            .chain(
                body.split('+')
                    .filter(|key| !key.is_empty())
                    .map(str::to_owned),
            )
            .map(SharedString::from)
            .collect();
        Rc::new(VecModel::from(keys)).into()
    });
    state.on_key_chord(|text, meta, ctrl, alt, shift, released| {
        key_chord(&text, meta, ctrl, alt, shift, released).into()
    });
    state.on_command_found(|command| apps::command_found(&command));
    {
        let weak = app.as_weak();
        let active = RefCell::new(None);
        state.on_inhibit_shortcuts(move |on| {
            let Some(app) = weak.upgrade() else { return };
            // Dropping the old inhibitor hands shortcuts back.
            *active.borrow_mut() = on
                .then(|| super::shortcuts_inhibit::inhibit(&app))
                .flatten();
        });
    }
    state.on_command_presets(|chord| {
        let labels: Vec<SharedString> = std::iter::once("Common commands…")
            .chain(
                keybinds::command_presets(&chord)
                    .into_iter()
                    .map(|(label, _)| label),
            )
            .map(SharedString::from)
            .collect();
        Rc::new(VecModel::from(labels)).into()
    });
    state.on_preset_command(|chord, index| {
        keybinds::command_presets(&chord)
            .get(index as usize)
            .map(|(_, command)| SharedString::from(*command))
            .unwrap_or_default()
    });

    // Everything that only needs the list redrawn.
    let rebuild = {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let kb_actions = Arc::clone(kb_actions);
        let view = Rc::clone(view);
        move || {
            if let Some(app) = weak.upgrade() {
                rebuild_keybind_rows(&app, &shell.borrow(), &kb_actions, &view);
            }
        }
    };
    {
        let rebuild = rebuild.clone();
        let view = Rc::clone(view);
        state.on_search_edited(move |_| {
            view.borrow_mut().exact = false;
            rebuild();
        });
    }
    {
        let rebuild = rebuild.clone();
        let view = Rc::clone(view);
        state.on_search_chord(move |_| {
            view.borrow_mut().exact = true;
            rebuild();
        });
    }
    state.on_filter_changed(rebuild.clone());
    {
        let rebuild = rebuild.clone();
        let view = Rc::clone(view);
        state.on_toggle_group(move |title| {
            // Headers show the group upper-cased; match it back.
            let Some(group) = keybinds::GROUP_ORDER
                .iter()
                .find(|group| group.to_uppercase() == title.as_str())
            else {
                return;
            };
            {
                let mut view = view.borrow_mut();
                if !view.collapsed.remove(*group) {
                    view.collapsed.insert((*group).to_owned());
                }
            }
            rebuild();
        });
    }
    {
        let weak = app.as_weak();
        let kb_actions = Arc::clone(kb_actions);
        let view = Rc::clone(view);
        state.on_add(move || {
            let Some(app) = weak.upgrade() else { return };
            view.borrow_mut().target = None;
            let actions = kb_actions.lock().expect("kb actions").clone();
            let state = app.global::<KeybindsState>();
            open_editor(&state, &actions, "New keybind", "", "", "spawn", None);
            // New binds start as "launch an app".
            prefer_app_mode(&state, &actions, &[], "spawn");
        });
    }
    {
        let weak = app.as_weak();
        let kb_actions = Arc::clone(kb_actions);
        let view = Rc::clone(view);
        state.on_edit(move |index, chord| {
            let Some(app) = weak.upgrade() else { return };
            let state = app.global::<KeybindsState>();
            // A second click on the open row closes it.
            if state.get_editor_open() && state.get_editor_anchor() == chord {
                state.set_editor_open(false);
                return;
            }
            let actions = kb_actions.lock().expect("kb actions").clone();
            let mut view = view.borrow_mut();
            if index >= 0 {
                let Some(bind) = view.binds.get(index as usize).cloned() else {
                    return;
                };
                view.target = Some((index as usize, bind.chord.clone()));
                open_editor(
                    &state,
                    &actions,
                    "Edit keybind",
                    &chord,
                    &bind.chord,
                    &bind.action,
                    Some(&bind),
                );
                prefer_app_mode(&state, &actions, apps_of(&mut view), &bind.action);
            } else {
                // A default: the editor writes a user bind that overrides it.
                let Some((_, action)) = keybinds::DEFAULT_BINDS
                    .iter()
                    .find(|(default, _)| *default == chord.as_str())
                else {
                    return;
                };
                view.target = None;
                open_editor(
                    &state,
                    &actions,
                    "Override default",
                    &chord,
                    &chord,
                    action,
                    None,
                );
                prefer_app_mode(&state, &actions, apps_of(&mut view), action);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let rebuild = rebuild.clone();
        let view = Rc::clone(view);
        state.on_remove(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let bind = view.borrow().binds.get(index as usize).cloned();
            let Some(bind) = bind else { return };
            let removed = {
                let mut shell = shell.borrow_mut();
                let removed = doc_at_mut(&mut shell, bind.source_file).remove_keybind(&bind.chord);
                app.set_dirty(shell.any_modified());
                removed
            };
            if removed {
                app.global::<KeybindsState>().set_editor_open(false);
                rebuild();
            }
        });
    }
    {
        let weak = app.as_weak();
        let kb_actions = Arc::clone(kb_actions);
        state.on_action_filter(move |text| {
            let Some(app) = weak.upgrade() else { return };
            let lowered = text.trim().to_lowercase();
            let actions = kb_actions.lock().expect("kb actions").clone();
            // spawn is listed as its two uses rather than its raw summary.
            let entries = actions.iter().flat_map(|action| {
                if action.name == "spawn" {
                    vec![
                        ("spawn", "Launch an app".to_owned(), LAUNCH_APP),
                        ("spawn", "Run a shell command".to_owned(), "spawn"),
                    ]
                } else {
                    vec![(
                        action.name.as_str(),
                        action.summary.clone(),
                        action.name.as_str(),
                    )]
                }
            });
            let matches: Vec<ActionChoice> = entries
                .filter(|(name, summary, _)| {
                    name.contains(&lowered) || summary.to_lowercase().contains(&lowered)
                })
                .map(|(name, summary, value)| ActionChoice {
                    name: name.into(),
                    summary: summary.into(),
                    value: value.into(),
                })
                .collect();
            app.global::<KeybindsState>()
                .set_action_matches(Rc::new(VecModel::from(matches)).into());
        });
    }
    {
        let weak = app.as_weak();
        let kb_actions = Arc::clone(kb_actions);
        state.on_action_picked(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let state = app.global::<KeybindsState>();
            let actions = kb_actions.lock().expect("kb actions").clone();
            // A new action's parameter means something else: start empty.
            set_draft_action(&state, &actions, &name, "");
            state.set_draft_app_name("".into());
            state.set_action_picker_open(false);
        });
    }
    {
        let weak = app.as_weak();
        let view = Rc::clone(view);
        state.on_apps_filter(move |text| {
            let Some(app) = weak.upgrade() else { return };
            let mut view = view.borrow_mut();
            let apps = apps_of(&mut view);
            let lowered = text.trim().to_lowercase();
            let matches: Vec<AppChoice> = apps
                .iter()
                .filter(|app| {
                    app.name.to_lowercase().contains(&lowered)
                        || app.command.to_lowercase().contains(&lowered)
                })
                .map(|app| AppChoice {
                    name: app.name.clone().into(),
                    command: app.command.clone().into(),
                })
                .collect();
            app.global::<KeybindsState>()
                .set_app_matches(Rc::new(VecModel::from(matches)).into());
        });
    }
    {
        let weak = app.as_weak();
        state.on_special_key_picked(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let Some((key, _)) = keybinds::key_choices().into_iter().nth(index as usize) else {
                return;
            };
            // Keep the modifiers typed so far; replace the key.
            let state = app.global::<KeybindsState>();
            let chord = state.get_draft_chord();
            let (head, last) = chord.rsplit_once('+').unwrap_or(("", chord.as_str()));
            let chord = match (keybinds::is_modifier(last), head) {
                (true, _) => format!("{chord}+{key}"),
                (false, "") => key,
                (false, head) => format!("{head}+{key}"),
            };
            state.set_draft_chord(chord.into());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let view = Rc::clone(view);
        state.on_apply(move || {
            let Some(app) = weak.upgrade() else { return };
            let state = app.global::<KeybindsState>();
            let chord_body = state.get_draft_chord().trim().to_owned();
            if chord_body.is_empty() {
                state.set_conflict_note("Record or type a key combination.".into());
                return;
            }
            let action_name = state.get_draft_action().to_string();
            if action_name.is_empty() {
                state.set_conflict_note("Choose an action.".into());
                return;
            }
            let param = state.get_draft_param().trim().to_owned();
            let missing = match state.get_param_input() {
                ParamInput::App => "Choose an app.",
                ParamInput::Command => "Enter a command.",
                _ => "",
            };
            if param.is_empty() && !missing.is_empty() {
                state.set_conflict_note(missing.into());
                return;
            }
            let action = if param.is_empty() {
                action_name
            } else {
                format!("{action_name}:{param}")
            };
            let cooldown = state.get_draft_cooldown().trim().to_owned();
            let cooldown_ms = match cooldown.parse::<i64>() {
                _ if cooldown.is_empty() => None,
                Ok(ms) if (0..=3_600_000).contains(&ms) => Some(ms),
                _ => {
                    state.set_conflict_note("Cooldown is milliseconds, from 0 to 3600000.".into());
                    return;
                }
            };
            let submap = state.get_draft_submap().trim().to_owned();
            let chord = keybinds::compose_chord(&state.get_draft_scope(), &chord_body);
            let bind = KeybindEntry {
                chord: chord.clone(),
                action,
                repeat: state.get_draft_no_repeat().then_some(false),
                allow_when_locked: state.get_draft_locked().then_some(true),
                allow_when_inhibited: state.get_draft_inhibited().then_some(true),
                cooldown_ms,
                submap: (!submap.is_empty()).then_some(submap),
            };
            let target = view.borrow().target.clone();
            let original = target.as_ref().map(|(_, chord)| chord.clone());
            {
                let shell = shell.borrow();
                let docs = chain_docs(&shell);
                if let Some((_file, other)) =
                    keybinds::find_conflict(&docs, &chord, original.as_deref())
                {
                    state.set_conflict_note(
                        format!("Already bound to {other}; pick another combination.").into(),
                    );
                    return;
                }
            }
            {
                let mut shell = shell.borrow_mut();
                let write = |doc: &mut ConfigDocument| {
                    if let Some(original) = &original
                        && !original.eq_ignore_ascii_case(&chord)
                    {
                        doc.remove_keybind(original);
                    }
                    doc.set_keybind(&bind);
                };
                if let Some((index, _)) = target {
                    let Some(source) = view.borrow().binds.get(index).map(|bind| bind.source_file)
                    else {
                        return;
                    };
                    write(doc_at_mut(&mut shell, source));
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
                app.set_dirty(shell.any_modified());
            }
            state.set_editor_open(false);
            rebuild();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_events_become_chords() {
        use slint::platform::Key;
        let key = |k: Key| SharedString::from(k).to_string();
        assert_eq!(key_chord("h", true, false, false, false, false), "Mod+H");
        assert_eq!(
            key_chord(&key(Key::LeftArrow), true, false, false, true, false),
            "Mod+Shift+Left"
        );
        assert_eq!(
            key_chord(&key(Key::Return), false, true, false, false, false),
            "Ctrl+Return"
        );
        // A modifier pressed alone waits; released alone, it is the chord.
        assert_eq!(
            key_chord(&key(Key::Meta), true, false, false, false, false),
            ""
        );
        assert_eq!(
            key_chord(&key(Key::Meta), false, false, false, false, true),
            "Mod"
        );
        assert_eq!(
            key_chord(&key(Key::Shift), true, false, false, true, true),
            "Mod+Shift"
        );
        assert_eq!(key_chord("h", false, false, false, false, true), "");
        assert_eq!(
            key_chord(&key(Key::Escape), true, false, false, false, false),
            "Mod+Escape"
        );
        // Caps Lock and other unspellable keys are ignored.
        assert_eq!(
            key_chord(&key(Key::CapsLock), true, false, false, false, false),
            ""
        );
    }
}
