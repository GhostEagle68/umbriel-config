//! egui shell: loads the resolved config, tracks modifications, saves with
//! validation. Pages render from the schema assembled from umbriel's
//! packaged default config; changes write through to the document.

use eframe::egui;
use std::path::PathBuf;
use std::str::FromStr;
use umbriel_config::config::{
    discovery, document::ConfigDocument, keybinds, outputs, rules, schema, state, validate,
};
use umbriel_config::live;

/// Launch the GUI for `path`.
pub fn run(path: PathBuf) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 560.0])
            .with_title("Umbriel Config"),
        ..Default::default()
    };
    eframe::run_native(
        "umbriel-config",
        options,
        Box::new(move |_cc| Ok(Box::new(App::new(path)))),
    )
    .map_err(|err| anyhow::anyhow!("GUI failed: {err}"))
}

/// Which page the central panel shows.
#[derive(Clone, PartialEq)]
enum Page {
    /// A top-level schema section, e.g. `"general"`.
    Section(String),
    Outputs,
    /// Window or layer rules; the payload is the TOML section name.
    Rules(&'static str),
    /// The `[keybinds]` chord table.
    Keybinds,
    /// Keys in the config no other page claims.
    Raw,
}

struct App {
    path: PathBuf,
    doc: ConfigDocument,
    /// False when the config could not be loaded; saving stays disabled.
    healthy: bool,
    load_error: Option<String>,
    /// Report from the last save's `umbriel validate` run.
    last_validation: Option<validate::Report>,
    /// Why saving or validation failed after the last save attempt.
    validation_note: Option<String>,
    /// Schema assembled from the installed packaged default.
    schema: Vec<schema::Entry>,
    /// Selected central-panel page.
    page: Option<Page>,
    /// Text buffer for the Outputs page's add-by-name row.
    add_output: String,
    /// Connected monitors, scanned on first visit to the Outputs page.
    live_outputs: Vec<live::LiveOutput>,
    /// Whether a live scan was attempted (avoids rescanning every frame).
    live_scanned: bool,
    /// Why live enumeration is unavailable, when it failed.
    live_note: Option<String>,
    /// Active settings-search query; empty means browse normally.
    search: String,
    /// Notice from schema sync or the startup drift check; dismissable.
    schema_note: Option<String>,
    /// Action vocabulary from the installed umbriel (or the snapshot).
    actions: Vec<keybinds::LiveAction>,
    /// Search filter for the keybinds page.
    kb_search: String,
    /// The single open keybind editor, if any.
    kb_editor: Option<KbEditor>,
}

impl App {
    fn new(path: PathBuf) -> Self {
        let (doc, healthy, load_error) = match ConfigDocument::load(&path) {
            Ok(doc) => (doc, true, None),
            Err(err) => {
                // A missing file is a fresh start: saving creates it. Anything
                // else (a broken file) keeps saving disabled so it is never
                // overwritten from here.
                let healthy = err.is_not_found();
                let load_error = (!healthy).then_some(err.to_string());
                (
                    ConfigDocument::from_str("").expect("empty TOML parses"),
                    healthy,
                    load_error,
                )
            }
        };
        let env = discovery::Env::from_process();
        let schema = Self::load_schema(&env);
        let schema_note = Self::startup_note(&env, &schema);
        Self {
            path,
            doc,
            healthy,
            load_error,
            last_validation: None,
            validation_note: None,
            schema,
            page: None,
            add_output: String::new(),
            live_outputs: Vec::new(),
            live_scanned: false,
            live_note: None,
            schema_note,
            search: String::new(),
            actions: keybinds::builtin_actions(),
            kb_search: String::new(),
            kb_editor: None,
        }
    }

    /// Re-read the packaged default and swap in the fresh schema. Never
    /// touches the user's config; a new key shows its default until edited.
    fn sync_schema(&mut self) {
        let env = discovery::Env::from_process();
        let fresh = Self::load_schema(&env);
        let fresh_set = schema::key_set(&fresh);
        let drift = schema::diff(&schema::key_set(&self.schema), &fresh_set);
        let _ = state::store(&state::snapshot_path(&env), &fresh_set);
        self.schema_note = Some(if fresh.is_empty() {
            "No packaged default found; install umbriel and sync again.".to_owned()
        } else if drift.is_empty() {
            "Schema is up to date.".to_owned()
        } else {
            format!("Synced from umbriel: {}.", drift.summary())
        });
        self.schema = fresh;
        self.actions = Self::load_actions();
        if let Some(Page::Section(current)) = self.page.as_ref()
            && !self.sections().contains(current)
        {
            self.page = None;
        }
    }

    /// Enumerate connected monitors; the Refresh button re-runs it.
    fn scan_outputs(&mut self) {
        self.live_scanned = true;
        match live::outputs() {
            Ok(list) => {
                self.live_outputs = list;
                self.live_note = None;
            }
            Err(err) => {
                self.live_outputs = Vec::new();
                self.live_note = Some(err.to_string());
            }
        }
    }

    /// Outputs page: live monitors first (Configure creates their config),
    /// then configured-but-disconnected tables, plus add-by-name. Renders
    /// the possibility space, never just what is set — an empty list invites.
    fn outputs_page(&mut self, ui: &mut egui::Ui) {
        if !self.live_scanned {
            self.scan_outputs();
        }
        let configured = outputs::configured(&self.doc);
        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.scan_outputs();
            }
            if let Some(note) = &self.live_note {
                ui.colored_label(
                    egui::Color32::from_rgb(230, 180, 80),
                    format!("live state unavailable: {note}"),
                );
            }
        });
        ui.add_space(4.0);

        // Connected monitors first, then configured ones that are away.
        let mut names: Vec<String> = Vec::new();
        for output in &self.live_outputs {
            if !names.contains(&output.name) {
                names.push(output.name.clone());
            }
        }
        for name in &configured {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        if names.is_empty() {
            ui.label(
                "No outputs connected or configured. Add one below to set its\n\
                 mode, scale, or workspace names — umbriel's defaults apply until then.",
            );
            ui.add_space(8.0);
        }
        for name in &names {
            let live = self.live_outputs.iter().find(|output| output.name == *name);
            let is_configured = configured.contains(name);
            let title = if is_configured && live.is_none() {
                format!("{name} (not connected)")
            } else {
                name.clone()
            };
            egui::CollapsingHeader::new(title)
                .default_open(names.len() == 1)
                .show(ui, |ui| {
                    if let Some(live) = live {
                        if !live.description.is_empty() {
                            ui.label(&live.description);
                        }
                        if let Some(mode) = live.current.and_then(|index| live.modes.get(index)) {
                            ui.label(format!("Currently: {}", mode.label()));
                        }
                        if !is_configured && ui.button("Configure").clicked() {
                            self.doc.set_bool(&["output", name, "enabled"], true);
                        }
                    } else {
                        ui.label("Not connected — settings apply when it is plugged in.");
                    }
                    if is_configured {
                        for field in outputs::FIELDS {
                            output_field_row(ui, &mut self.doc, name, field, live);
                        }
                        ui.add_space(4.0);
                        let remove = ui.add_enabled(
                            names.len() > 1,
                            egui::Button::new(
                                egui::RichText::new("Remove output")
                                    .color(egui::Color32::from_rgb(240, 100, 100)),
                            )
                            .small(),
                        );
                        let remove = remove
                            .on_hover_text(
                                "Deletes this output's block from the config; umbriel's\n\
                                 defaults apply after saving. Nothing is written until Save.",
                            )
                            .on_disabled_hover_text("The only output cannot be removed.");
                        if remove.clicked() {
                            self.doc.remove_table(&["output", name]);
                        }
                    }
                });
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Add output");
            ui.text_edit_singleline(&mut self.add_output)
                .on_hover_text("Connector name, e.g. DP-3 or eDP-1");
            if ui.button("Add").clicked() {
                let name = self.add_output.trim().to_owned();
                if !name.is_empty() && !names.contains(&name) {
                    self.doc.set_bool(&["output", &name, "enabled"], true);
                    self.add_output.clear();
                }
            }
        });
    }

    /// Window/layer rules: one collapsible card per `[[section]]` entry.
    /// Every field starts unset; only what the user fills in is written.
    fn rules_page(&mut self, ui: &mut egui::Ui, name: &'static str) {
        let label = if name == "window_rule" {
            "Window rules"
        } else {
            "Layer rules"
        };
        let (match_fields, settings_fields): (&[rules::Field], &[rules::Field]) =
            if name == "window_rule" {
                (rules::WINDOW_MATCH, rules::WINDOW_SETTINGS)
            } else {
                (rules::LAYER_MATCH, rules::LAYER_SETTINGS)
            };
        ui.heading(label);
        ui.separator();
        if ui.button("Add rule").clicked() {
            self.doc.add_rule(name);
        }
        let count = self.doc.rule_count(name);
        if count == 0 {
            ui.add_space(4.0);
            ui.label(format!(
                "No {label} yet. Add one — every field starts unset, and only\n\
                 what you fill in is written to the config."
            ));
            return;
        }
        ui.add_space(4.0);
        for index in 0..count {
            let title = rules::rule_title(&self.doc, name, index, match_fields);
            egui::CollapsingHeader::new(title)
                .default_open(count == 1)
                .id_salt(format!("{name}-{index}"))
                .show(ui, |ui| {
                    ui.strong("Match");
                    for field in match_fields {
                        rule_field_row(ui, &mut self.doc, name, index, field);
                    }
                    ui.add_space(4.0);
                    ui.strong("Settings");
                    for field in settings_fields {
                        rule_field_row(ui, &mut self.doc, name, index, field);
                    }
                    ui.add_space(4.0);
                    if ui.button("Remove rule").clicked() {
                        self.doc.remove_rule(name, index);
                    }
                });
        }
    }

    /// Keybinds: one merged, grouped list — umbriel's built-in defaults
    /// with your overrides replacing them in place — plus a single draft
    /// editor. Nothing is written until the editor's Apply.
    fn keybinds_page(&mut self, ui: &mut egui::Ui) {
        ui.heading("Keybinds");
        ui.separator();

        // While capturing a chord the page is only the prompt, so pressed
        // keys cannot leak into any text field.
        if self
            .kb_editor
            .as_ref()
            .is_some_and(|editor| editor.capturing)
        {
            self.capture_chord(ui);
            return;
        }

        let mut mod_key = self
            .doc
            .get_string(&["general", "mod_key"])
            .unwrap_or_else(|| "Super".to_owned());
        let mod_changed = ui
            .horizontal(|ui| {
                ui.label("Modifier (Mod)");
                ui.text_edit_singleline(&mut mod_key)
                    .on_hover_text(
                        "What \"Mod\" means in the chords below; umbriel's default is Super.",
                    )
                    .changed()
            })
            .inner;
        if mod_changed && !mod_key.trim().is_empty() {
            self.doc.set_string(&["general", "mod_key"], mod_key.trim());
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.kb_search)
                    .hint_text("Search keybinds…")
                    .desired_width(200.0),
            );
            if ui.button("Add keybind").clicked() {
                self.kb_editor = Some(KbEditor {
                    target: None,
                    original: None,
                    draft: keybinds::BindDraft {
                        use_mod: true,
                        ..Default::default()
                    },
                    capturing: false,
                });
            }
        });
        ui.label(
            egui::RichText::new(
                "Bold rows are yours; the rest are umbriel's built-in defaults. \
                 Click any row to edit it.",
            )
            .weak()
            .small(),
        );

        // The new-bind editor renders above the list.
        if self
            .kb_editor
            .as_ref()
            .is_some_and(|editor| editor.target.is_none())
        {
            self.kb_editor_ui(ui);
            ui.add_space(6.0);
        }

        // Merged rows: built-in defaults overridden in place by user
        // binds, then user binds that no default claims.
        let binds = self.doc.keybinds();
        let mut rows: Vec<MergedRow> = Vec::new();
        for (chord, action) in keybinds::DEFAULT_BINDS {
            match binds.iter().find(|b| b.chord.eq_ignore_ascii_case(chord)) {
                Some(user) => rows.push(MergedRow {
                    chord: user.chord.clone(),
                    action: user.action.clone(),
                    repeat: user.repeat,
                    allow_when_locked: user.allow_when_locked,
                    submap: user.submap.clone(),
                    user: Some(user.chord.clone()),
                    shadows_default: true,
                }),
                None => rows.push(MergedRow {
                    chord: (*chord).to_owned(),
                    action: (*action).to_owned(),
                    repeat: None,
                    allow_when_locked: None,
                    submap: None,
                    user: None,
                    shadows_default: false,
                }),
            }
        }
        for bind in &binds {
            if !keybinds::DEFAULT_BINDS
                .iter()
                .any(|(chord, _)| chord.eq_ignore_ascii_case(&bind.chord))
            {
                rows.push(MergedRow {
                    chord: bind.chord.clone(),
                    action: bind.action.clone(),
                    repeat: bind.repeat,
                    allow_when_locked: bind.allow_when_locked,
                    submap: bind.submap.clone(),
                    user: Some(bind.chord.clone()),
                    shadows_default: false,
                });
            }
        }

        // Chords bound twice in the user's file highlight red.
        let duplicated = |chord: &str| {
            binds
                .iter()
                .filter(|b| b.chord.eq_ignore_ascii_case(chord))
                .count()
                > 1
        };

        let needle = self.kb_search.trim().to_lowercase();
        let edit_target = self
            .kb_editor
            .as_ref()
            .and_then(|editor| editor.target.clone());
        let mut groups: Vec<(&'static str, Vec<&MergedRow>)> = Vec::new();
        for row in &rows {
            let group = keybinds::action_group(&row.action);
            match groups.iter_mut().find(|(name, _)| *name == group) {
                Some((_, list)) => list.push(row),
                None => groups.push((group, vec![row])),
            }
        }
        groups.sort_by_key(|(name, _)| {
            keybinds::GROUP_ORDER
                .iter()
                .position(|known| known == name)
                .unwrap_or(usize::MAX)
        });
        for (group, entries) in &groups {
            let visible: Vec<&MergedRow> = entries
                .iter()
                .copied()
                .filter(|row| {
                    needle.is_empty()
                        || row.chord.to_lowercase().contains(&needle)
                        || row.action.to_lowercase().contains(&needle)
                        || keybinds::describe(&row.action, &self.actions)
                            .to_lowercase()
                            .contains(&needle)
                })
                .collect();
            if visible.is_empty() {
                continue;
            }
            ui.add_space(6.0);
            ui.strong(format!("{group} ({})", visible.len()));
            for row in visible {
                if edit_target
                    .as_ref()
                    .is_some_and(|target| target.eq_ignore_ascii_case(&row.chord))
                {
                    self.kb_editor_ui(ui);
                } else if keybind_display_row(
                    ui,
                    &mut self.doc,
                    &self.actions,
                    row,
                    duplicated(&row.chord),
                ) {
                    self.kb_editor = Some(KbEditor {
                        target: Some(row.chord.clone()),
                        original: row.user.clone(),
                        draft: keybinds::BindDraft::from_parts(
                            &row.chord,
                            &row.action,
                            row.repeat,
                            row.allow_when_locked,
                            row.submap.clone(),
                        ),
                        capturing: false,
                    });
                }
            }
        }
    }

    /// Full-page chord-capture prompt: Esc cancels, Backspace clears the
    /// chord, any other recordable key completes it. "Mod" comes from the
    /// editor's checkbox — the compositor keeps the mod key for itself
    /// while a GUI is focused.
    fn capture_chord(&mut self, ui: &mut egui::Ui) {
        let Some(editor) = self.kb_editor.as_mut() else {
            return;
        };
        ui.add_space(16.0);
        ui.colored_label(
            egui::Color32::from_rgb(140, 200, 140),
            "Press the keys for this keybind…  Esc cancels, Backspace clears.",
        );
        let events: Vec<egui::Event> = ui.input(|input| input.events.clone());
        for event in events {
            let egui::Event::Key {
                key,
                modifiers,
                pressed: true,
                repeat: false,
                ..
            } = event
            else {
                continue;
            };
            match key {
                egui::Key::Escape => editor.capturing = false,
                egui::Key::Backspace => {
                    editor.draft.chord.clear();
                    editor.capturing = false;
                }
                _ => {
                    if let Some(name) = key_name(key) {
                        let mut parts: Vec<String> = Vec::new();
                        if modifiers.shift {
                            parts.push("Shift".to_owned());
                        }
                        if modifiers.ctrl {
                            parts.push("Ctrl".to_owned());
                        }
                        if modifiers.alt {
                            parts.push("Alt".to_owned());
                        }
                        parts.push(name);
                        editor.draft.chord = parts.join("+");
                        editor.capturing = false;
                    }
                }
            }
            break;
        }
    }

    /// The single draft editor. Writes nothing to the document until
    /// Apply (or Replace, when the chord is already taken).
    fn kb_editor_ui(&mut self, ui: &mut egui::Ui) {
        let Some(mut editor) = self.kb_editor.take() else {
            return;
        };
        let mut close = false;
        let mut apply = false;
        let mut replace = false;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Chord");
                ui.add(egui::TextEdit::singleline(&mut editor.draft.chord).desired_width(150.0))
                    .on_hover_text(keybinds::CHORD_HINT);
                if ui.button("⌨ Press keys…").clicked() {
                    editor.capturing = true;
                }
                key_picker(ui, &mut editor.draft.chord);
                ui.checkbox(&mut editor.draft.use_mod, "Mod")
                    .on_hover_text("Prefix the chord with Mod (your mod_key)");
            });
            ui.horizontal(|ui| {
                ui.label("Action");
                ui.add(egui::TextEdit::singleline(&mut editor.draft.action).desired_width(260.0));
                action_picker(ui, &mut editor.draft.action, &self.actions);
            });
            egui::CollapsingHeader::new(egui::RichText::new("advanced").small())
                .id_salt("kb-editor-advanced")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let mut repeat = editor.draft.repeat.unwrap_or(true);
                        ui.checkbox(&mut repeat, "repeat")
                            .on_hover_text("Auto-repeat while held");
                        editor.draft.repeat = (!repeat).then_some(false);
                        let mut locked = editor.draft.allow_when_locked.unwrap_or(false);
                        ui.checkbox(&mut locked, "locked")
                            .on_hover_text("Works while the session is locked");
                        editor.draft.allow_when_locked = locked.then_some(true);
                        ui.add(
                            egui::TextEdit::singleline(&mut editor.draft.scope)
                                .hint_text("submap scope")
                                .desired_width(110.0),
                        )
                        .on_hover_text("Only active inside this submap layer");
                        let mut submap = editor.draft.submap.clone().unwrap_or_default();
                        ui.add(
                            egui::TextEdit::singleline(&mut submap)
                                .hint_text("then enter submap")
                                .desired_width(110.0),
                        )
                        .on_hover_text(
                            "Switch to this submap layer after the action (or \"reset\")",
                        );
                        editor.draft.submap =
                            (!submap.trim().is_empty()).then(|| submap.trim().to_owned());
                    });
                });
            let composed = editor.draft.composed_chord();
            ui.label(egui::RichText::new(format!("→ {composed}")).weak().small());
            let conflict =
                keybinds::find_conflict(&[&self.doc], &composed, editor.original.as_deref());
            if composed.is_empty() || editor.draft.action.trim().is_empty() {
                ui.label(egui::RichText::new("A chord and an action are both needed.").weak());
            } else if let Some((_, other)) = &conflict {
                ui.colored_label(
                    egui::Color32::from_rgb(240, 100, 100),
                    format!("⚠ {composed} is already bound to {other}"),
                );
                if ui.button("Replace it").clicked() {
                    replace = true;
                    apply = true;
                }
            } else if ui.button("Apply").clicked() {
                apply = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
        if apply {
            let composed = editor.draft.composed_chord();
            if replace
                && let Some(file) = self
                    .doc
                    .keybinds()
                    .iter()
                    .find(|b| {
                        b.chord.eq_ignore_ascii_case(&composed)
                            && editor.original.as_deref() != Some(b.chord.as_str())
                    })
                    .map(|b| b.chord.clone())
            {
                self.doc.remove_table(&["keybinds", &file]);
            }
            self.doc.set_keybind(
                &composed,
                editor.draft.action.trim(),
                editor.draft.repeat,
                editor.draft.allow_when_locked,
                editor.draft.submap.as_deref(),
            );
            if let Some(original) = &editor.original
                && !original.eq_ignore_ascii_case(&composed)
            {
                self.doc.remove_table(&["keybinds", original]);
            }
            self.kb_editor = None;
        } else if close {
            self.kb_editor = None;
        } else {
            self.kb_editor = Some(editor);
        }
    }

    fn save(&mut self) {
        self.last_validation = None;
        self.validation_note = None;
        if let Err(err) = self.doc.save(&self.path) {
            self.validation_note = Some(err.to_string());
            return;
        }
        match validate::validate(&self.path) {
            Ok(report) => self.last_validation = Some(report),
            Err(err) => self.validation_note = Some(err.to_string()),
        }
    }

    /// Top-level sections in schema (file) order.
    fn sections(&self) -> Vec<String> {
        let mut sections: Vec<String> = Vec::new();
        for entry in &self.schema {
            let top = top_level(&entry.section);
            if !sections.contains(&top) {
                sections.push(top);
            }
        }
        sections
    }

    /// Keys in the config that no surface claims; drives the Raw page.
    fn raw_keys(&self) -> Vec<String> {
        schema::uncovered(&self.doc.value_paths(), &schema::key_set(&self.schema))
    }

    /// Schema from the installed packaged default; empty when unavailable.
    fn load_schema(env: &discovery::Env) -> Vec<schema::Entry> {
        discovery::packaged_default(env)
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| schema::assemble(&text))
            .unwrap_or_default()
    }

    /// Live action list from the installed umbriel; the committed snapshot
    /// when it can't be asked.
    fn load_actions() -> Vec<keybinds::LiveAction> {
        match std::process::Command::new("umbriel")
            .args(["msg", "--help"])
            .output()
        {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                let parsed = keybinds::actions_from_help(&text);
                if parsed.is_empty() {
                    keybinds::builtin_actions()
                } else {
                    parsed
                }
            }
            _ => keybinds::builtin_actions(),
        }
    }

    /// Diff the fresh schema against the last run's snapshot and refresh it.
    /// Silent on the first run (no snapshot yet) and when nothing changed.
    fn startup_note(env: &discovery::Env, entries: &[schema::Entry]) -> Option<String> {
        let current = schema::key_set(entries);
        let seen = state::load(&state::snapshot_path(env));
        let drift = schema::diff(&seen, &current);
        let _ = state::store(&state::snapshot_path(env), &current);
        (!seen.is_empty() && !drift.is_empty())
            .then(|| format!("Umbriel changed since last run: {}.", drift.summary()))
    }
}

fn top_level(section: &str) -> String {
    section.split('.').next().unwrap_or_default().to_owned()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let raw_keys = self.raw_keys();
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Umbriel Config");
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search settings")
                        .desired_width(150.0),
                );
                ui.label(self.path.display().to_string());
                if self.doc.is_modified() {
                    ui.colored_label(egui::Color32::from_rgb(230, 180, 80), "unsaved changes");
                }
                if ui.button("Sync schema").clicked() {
                    self.sync_schema();
                }
                if ui.button("Save").clicked() && self.healthy {
                    self.save();
                }
            });
        });
        if let Some(report) = &self.last_validation {
            egui::Panel::top("validation").show(ui, |ui| {
                for diagnostic in &report.diagnostics {
                    let (color, label) = if diagnostic.is_error() {
                        (egui::Color32::from_rgb(240, 100, 100), "error")
                    } else {
                        (egui::Color32::from_rgb(230, 180, 80), "warning")
                    };
                    ui.colored_label(color, format!("{label}: {}", diagnostic.message()));
                }
            });
        }
        if let Some(note) = &self.validation_note {
            egui::Panel::top("validation_note").show(ui, |ui| {
                ui.colored_label(egui::Color32::from_rgb(240, 100, 100), note);
            });
        }
        let schema_note = self.schema_note.clone();
        if let Some(note) = schema_note {
            egui::Panel::top("schema_note").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(egui::Color32::from_rgb(140, 200, 140), note);
                    if ui.small_button("Dismiss").clicked() {
                        self.schema_note = None;
                    }
                });
            });
        }
        egui::Panel::left("sidebar").show(ui, |ui| {
            let sections = self.sections();
            ui.vertical(|ui| {
                for section in &sections {
                    let clicked = ui
                        .selectable_value(
                            &mut self.page,
                            Some(Page::Section(section.clone())),
                            schema::humanize(section),
                        )
                        .clicked();
                    if clicked {
                        self.search.clear();
                    }
                }
                if !sections.is_empty() {
                    ui.separator();
                }
                if ui
                    .selectable_value(&mut self.page, Some(Page::Outputs), "Outputs")
                    .clicked()
                {
                    self.search.clear();
                }
                if ui
                    .selectable_value(
                        &mut self.page,
                        Some(Page::Rules("window_rule")),
                        "Window rules",
                    )
                    .clicked()
                {
                    self.search.clear();
                }
                if ui
                    .selectable_value(
                        &mut self.page,
                        Some(Page::Rules("layer_rule")),
                        "Layer rules",
                    )
                    .clicked()
                {
                    self.search.clear();
                }
                if ui
                    .selectable_value(&mut self.page, Some(Page::Keybinds), "Keybinds")
                    .clicked()
                {
                    self.search.clear();
                }
                if !raw_keys.is_empty() {
                    ui.separator();
                    if ui
                        .selectable_value(
                            &mut self.page,
                            Some(Page::Raw),
                            format!("Other settings ({})", raw_keys.len()),
                        )
                        .clicked()
                    {
                        self.search.clear();
                    }
                }
            });
        });
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(error) = &self.load_error {
                ui.colored_label(
                    egui::Color32::from_rgb(240, 100, 100),
                    format!("Could not load config: {error}"),
                );
                ui.label("Fix the file (see `umbriel validate`) and reopen the app.");
                return;
            }
            if self.healthy && !self.path.exists() {
                ui.add_space(24.0);
                ui.heading("Welcome to Umbriel Config");
                ui.add_space(4.0);
                ui.label(format!(
                    "No config file exists yet, so umbriel is running on its built-in\n\
                     defaults. Create {} to start customizing. Umbriel picks up\n\
                     every change the moment it is saved.",
                    self.path.display()
                ));
                ui.add_space(12.0);
                if ui
                    .button("Create config")
                    .on_hover_text(
                        "Creates the file; nothing is written until you change something.",
                    )
                    .clicked()
                {
                    self.save();
                }
                return;
            }
            let query = self.search.trim().to_owned();
            if !query.is_empty() {
                let found: Vec<&schema::Entry> = self
                    .schema
                    .iter()
                    .filter(|entry| schema::matches(entry, &query))
                    .collect();
                ui.heading(format!(
                    "{} setting{} matching \"{query}\"",
                    found.len(),
                    if found.len() == 1 { "" } else { "s" }
                ));
                ui.separator();
                if found.is_empty() {
                    ui.label("Nothing found. Try fewer or different words.");
                    return;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut current_section = String::new();
                    for entry in found {
                        if entry.section != current_section {
                            current_section = entry.section.clone();
                            ui.add_space(6.0);
                            ui.heading(schema::humanize(&current_section));
                        }
                        entry_row(ui, &mut self.doc, entry);
                    }
                });
                return;
            }
            let sections = self.sections();
            let page = self.page.clone().or_else(|| {
                sections
                    .first()
                    .map(|section| Page::Section(section.clone()))
            });
            let Some(page) = page else {
                return;
            };
            self.page = Some(page.clone());
            match page {
                Page::Section(section) => {
                    if self.schema.is_empty() {
                        ui.label(
                            "No umbriel schema found.\n\
                             Install umbriel (or its packaged default config) and reopen.",
                        );
                        return;
                    }
                    ui.heading(schema::humanize(&section));
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let mut current_group = String::new();
                        for entry in &self.schema {
                            if top_level(&entry.section) != section {
                                continue;
                            }
                            if entry.section != section && entry.section != current_group {
                                current_group = entry.section.clone();
                                let group = entry
                                    .section
                                    .strip_prefix(&format!("{section}."))
                                    .unwrap_or(&entry.section);
                                ui.add_space(6.0);
                                ui.heading(schema::humanize(group));
                            }
                            entry_row(ui, &mut self.doc, entry);
                        }
                    });
                }
                Page::Outputs => {
                    ui.heading("Outputs");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.outputs_page(ui);
                    });
                }
                Page::Rules(name) => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.rules_page(ui, name);
                    });
                }
                Page::Keybinds => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.keybinds_page(ui);
                    });
                }
                Page::Raw => {
                    ui.heading("Other settings");
                    ui.separator();
                    ui.label(
                        "These keys are in your config but have no dedicated page yet.\n\
                         They are shown here so nothing is hidden; scalar values are editable.",
                    );
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for dotted in &raw_keys {
                            raw_row(ui, &mut self.doc, dotted);
                        }
                    });
                }
            }
        });
    }
}

/// One field of one output, writing through on change like `entry_row`.
/// Live state upgrades the mode field to a dropdown of the monitor's modes.
fn output_field_row(
    ui: &mut egui::Ui,
    doc: &mut ConfigDocument,
    name: &str,
    field: &outputs::Field,
    live_output: Option<&live::LiveOutput>,
) {
    let path = ["output", name, field.key];
    let live_modes = live_output
        .map(|output| output.modes.as_slice())
        .unwrap_or_default();
    match &field.kind {
        outputs::FieldKind::Toggle => {
            let mut value = doc.get_bool(&path).unwrap_or(matches!(
                field.default,
                Some(outputs::DefaultValue::Bool(true))
            ));
            if ui.checkbox(&mut value, field.label).changed() {
                doc.set_bool(&path, value);
            }
        }
        outputs::FieldKind::Choice(values) => {
            let mut value = doc.get_string(&path).unwrap_or_else(|| default_text(field));
            let original = value.clone();
            ui.horizontal(|ui| {
                ui.label(field.label);
                egui::ComboBox::from_id_salt(format!("output-field-{name}-{}", field.key))
                    .selected_text(&value)
                    .show_ui(ui, |ui| {
                        for choice in *values {
                            ui.selectable_value(&mut value, (*choice).to_owned(), *choice);
                        }
                    });
            });
            if value != original {
                doc.set_string(&path, &value);
            }
        }
        outputs::FieldKind::Text => {
            if field.key == "mode" && !live_modes.is_empty() {
                let mut value = doc.get_string(&path).unwrap_or_default();
                let original = value.clone();
                ui.horizontal(|ui| {
                    ui.label(field.label);
                    egui::ComboBox::from_id_salt(format!("output-mode-{name}"))
                        .selected_text(&value)
                        .show_ui(ui, |ui| {
                            for mode in live_modes {
                                let label = mode.label();
                                ui.selectable_value(&mut value, label.clone(), label);
                            }
                        });
                });
                if value != original {
                    doc.set_string(&path, &value);
                }
            } else {
                let mut value = doc.get_string(&path).unwrap_or_default();
                let changed = ui
                    .horizontal(|ui| {
                        ui.label(field.label);
                        ui.text_edit_singleline(&mut value)
                    })
                    .inner
                    .changed();
                if changed {
                    doc.set_string(&path, &value);
                }
            }
        }
        outputs::FieldKind::Position => {
            let current = doc.get_integers(&path).unwrap_or_default();
            let mut x = current.first().copied().unwrap_or(0);
            let mut y = current.get(1).copied().unwrap_or(0);
            let changed = ui
                .horizontal(|ui| {
                    ui.label(field.label);
                    let changed_x = ui.add(egui::DragValue::new(&mut x)).changed();
                    let changed_y = ui.add(egui::DragValue::new(&mut y)).changed();
                    changed_x || changed_y
                })
                .inner;
            if changed {
                doc.set_integers(&path, &[x, y]);
            }
        }
        outputs::FieldKind::Float { min, max } => {
            let mut value = doc.get_float(&path).unwrap_or_else(|| default_float(field));
            if ui
                .add(egui::Slider::new(&mut value, *min..=*max).text(field.label))
                .changed()
            {
                doc.set_float(&path, value);
            }
        }
        outputs::FieldKind::Workspaces => {
            let mut text = workspaces_text(doc, &path);
            if text.is_empty() {
                text = "dynamic".to_owned();
            }
            let changed = ui
                .horizontal(|ui| {
                    ui.label(field.label);
                    ui.text_edit_singleline(&mut text)
                        .on_hover_text("Workspace count, comma-separated names, or \"dynamic\"")
                        .changed()
                })
                .inner;
            if changed {
                store_workspaces(doc, &path, &text);
            }
        }
    }
}

/// One field of one rule card. Rules are user-authored: nothing has a
/// default, so unset fields stay blank and clearing a field removes it.
fn rule_field_row(
    ui: &mut egui::Ui,
    doc: &mut ConfigDocument,
    name: &str,
    index: usize,
    field: &rules::Field,
) {
    let id = format!("rule-{name}-{index}-{}", field.key);
    match &field.kind {
        rules::FieldKind::Text => {
            let mut value = doc.rule_string(name, index, field.key).unwrap_or_default();
            let changed = ui
                .horizontal(|ui| {
                    ui.label(field.label);
                    ui.text_edit_singleline(&mut value)
                        .on_hover_text("Leave empty to leave unset")
                })
                .inner
                .changed();
            if changed {
                if value.is_empty() {
                    doc.rule_unset(name, index, field.key);
                } else {
                    doc.rule_set_string(name, index, field.key, &value);
                }
            }
        }
        rules::FieldKind::Toggle => {
            let mut value = doc.rule_bool(name, index, field.key).unwrap_or(false);
            if ui.checkbox(&mut value, field.label).changed() {
                doc.rule_set_bool(name, index, field.key, value);
            }
        }
        rules::FieldKind::Choice(options) => {
            let mut value = doc.rule_string(name, index, field.key).unwrap_or_default();
            let original = value.clone();
            ui.horizontal(|ui| {
                ui.label(field.label);
                egui::ComboBox::from_id_salt(id)
                    .selected_text(if value.is_empty() {
                        "(unset)".to_owned()
                    } else {
                        value.clone()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut value, String::new(), "(unset)");
                        for option in *options {
                            ui.selectable_value(&mut value, (*option).to_owned(), *option);
                        }
                    });
            });
            if value != original {
                if value.is_empty() {
                    doc.rule_unset(name, index, field.key);
                } else {
                    doc.rule_set_string(name, index, field.key, &value);
                }
            }
        }
        rules::FieldKind::Float { min, max } => {
            let mut value = doc.rule_float(name, index, field.key).unwrap_or(*min);
            if ui
                .add(egui::Slider::new(&mut value, *min..=*max).text(field.label))
                .changed()
            {
                doc.rule_set_float(name, index, field.key, value);
            }
        }
        rules::FieldKind::Integer { min, max } => {
            let mut value = doc.rule_integer(name, index, field.key).unwrap_or(*min);
            if ui
                .add(egui::Slider::new(&mut value, *min..=*max).text(field.label))
                .changed()
            {
                doc.rule_set_integer(name, index, field.key, value);
            }
        }
        rules::FieldKind::Size => {
            let current = doc
                .rule_integers(name, index, field.key)
                .unwrap_or_default();
            let mut width = current.first().copied().unwrap_or(1920);
            let mut height = current.get(1).copied().unwrap_or(1080);
            let changed = ui
                .horizontal(|ui| {
                    ui.label(field.label);
                    let width_changed = ui.add(egui::DragValue::new(&mut width)).changed();
                    let height_changed = ui.add(egui::DragValue::new(&mut height)).changed();
                    width_changed || height_changed
                })
                .inner;
            if changed {
                doc.rule_set_integers(name, index, field.key, &[width, height]);
            }
        }
        rules::FieldKind::Position => {
            let (mut x, mut y, mut anchor) = doc
                .rule_position(name, index, field.key)
                .unwrap_or((0, 0, None));
            let changed = ui
                .horizontal(|ui| {
                    ui.label(field.label);
                    let x_changed = ui.add(egui::DragValue::new(&mut x)).changed();
                    let y_changed = ui.add(egui::DragValue::new(&mut y)).changed();
                    let mut anchor_changed = false;
                    egui::ComboBox::from_id_salt(format!("{id}-anchor"))
                        .selected_text(anchor.clone().unwrap_or_else(|| "anchor".to_owned()))
                        .show_ui(ui, |ui| {
                            for candidate in rules::ANCHORS {
                                anchor_changed |= ui
                                    .selectable_value(
                                        &mut anchor,
                                        Some((*candidate).to_owned()),
                                        *candidate,
                                    )
                                    .changed();
                            }
                        });
                    x_changed || y_changed || anchor_changed
                })
                .inner;
            if changed {
                doc.rule_set_position(name, index, field.key, x, y, anchor.as_deref());
            }
        }
    }
}

/// One row of the merged keybind list: a built-in default or user bind,
/// with the user's override already applied in place.
struct MergedRow {
    chord: String,
    action: String,
    repeat: Option<bool>,
    allow_when_locked: Option<bool>,
    submap: Option<String>,
    /// File-exact chord of the user bind backing this row, if any.
    user: Option<String>,
    /// The row shadows a built-in default (a ↺ reset is available).
    shadows_default: bool,
}

/// The single open keybind editor. `target` places it inline under its
/// row (None renders it at the top, for a new bind); `original` is the
/// file-exact chord being edited, so a rename removes the old entry.
struct KbEditor {
    target: Option<String>,
    original: Option<String>,
    draft: keybinds::BindDraft,
    capturing: bool,
}

/// One merged keybind row: the chord (bold when yours, red when the same
/// chord is bound twice) and the human action text; either opens the
/// editor. Overridden defaults get ↺ reset, user-only binds ✕ remove.
/// Returns whether the row was clicked for editing.
fn keybind_display_row(
    ui: &mut egui::Ui,
    doc: &mut ConfigDocument,
    actions: &[keybinds::LiveAction],
    row: &MergedRow,
    duplicated: bool,
) -> bool {
    let summary = keybinds::describe(&row.action, actions);
    let mut edit = false;
    ui.horizontal(|ui| {
        let mut chord = egui::RichText::new(&row.chord).monospace();
        if row.user.is_some() {
            chord = chord.strong();
        }
        if duplicated {
            chord = chord.color(egui::Color32::from_rgb(240, 100, 100));
        }
        if ui
            .selectable_label(false, chord)
            .on_hover_text("Click to edit")
            .clicked()
        {
            edit = true;
        }
        if ui
            .selectable_label(false, summary.as_str())
            .on_hover_text(row.action.as_str())
            .clicked()
        {
            edit = true;
        }
        if let Some(file) = &row.user {
            let remove = if row.shadows_default {
                ui.button("↺")
                    .on_hover_text("Reset to umbriel's built-in default")
                    .clicked()
            } else {
                ui.button("✕")
                    .on_hover_text("Remove this keybind")
                    .clicked()
            };
            if remove {
                doc.remove_table(&["keybinds", file]);
            }
        }
    });
    edit
}

/// Searchable picker of special keys (media, mouse, wheel, numpad);
/// picking writes the bare keysym into the draft chord.
fn key_picker(ui: &mut egui::Ui, chord: &mut String) {
    egui::ComboBox::from_id_salt("kb-key-picker")
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text("key…")
        .show_ui(ui, |ui| {
            let search_id = egui::Id::new("kb-key-picker-search");
            let mut filter = combo_filter(ui, search_id);
            ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Search keys…"));
            ui.memory_mut(|mem| mem.data.insert_temp(search_id, filter.clone()));
            let needle = filter.to_lowercase();
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for (keysym, label) in keybinds::COMMON_KEYS {
                        if !needle.is_empty()
                            && !format!("{keysym} {label}").to_lowercase().contains(&needle)
                        {
                            continue;
                        }
                        if ui
                            .selectable_label(false, *label)
                            .on_hover_text(*keysym)
                            .clicked()
                        {
                            *chord = (*keysym).to_owned();
                        }
                    }
                });
        });
}

/// Searchable action picker over the installed umbriel's live list;
/// picking a parameterized action writes `name:` ready for the argument.
fn action_picker(ui: &mut egui::Ui, action: &mut String, actions: &[keybinds::LiveAction]) {
    egui::ComboBox::from_id_salt("kb-action-picker")
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text("pick…")
        .show_ui(ui, |ui| {
            let search_id = egui::Id::new("kb-action-picker-search");
            let mut filter = combo_filter(ui, search_id);
            ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Search actions…"));
            ui.memory_mut(|mem| mem.data.insert_temp(search_id, filter.clone()));
            let needle = filter.to_lowercase();
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for live in actions {
                        if !needle.is_empty()
                            && !format!("{} {} {}", live.name, live.param, live.summary)
                                .to_lowercase()
                                .contains(&needle)
                        {
                            continue;
                        }
                        let value = if live.param.is_empty() {
                            live.name.clone()
                        } else {
                            format!("{}:", live.name)
                        };
                        let label = if live.param.is_empty() {
                            live.name.clone()
                        } else {
                            format!("{} {}", live.name, live.param)
                        };
                        if ui
                            .selectable_label(*action == value, egui::RichText::new(label))
                            .on_hover_text(live.summary.clone())
                            .clicked()
                        {
                            *action = value;
                        }
                    }
                });
        });
}

fn default_text(field: &outputs::Field) -> String {
    match field.default {
        Some(outputs::DefaultValue::Text(value)) => value.to_owned(),
        _ => String::new(),
    }
}

fn default_float(field: &outputs::Field) -> f64 {
    match field.default {
        Some(outputs::DefaultValue::Float(value)) => value,
        _ => 0.0,
    }
}

/// `workspaces` (count, name list, or "dynamic") as editable text.
fn workspaces_text(doc: &ConfigDocument, path: &[&str]) -> String {
    if let Some(count) = doc.get_integer(path) {
        return count.to_string();
    }
    if let Some(names) = doc.get_strings(path) {
        return names.join(", ");
    }
    doc.get_string(path).unwrap_or_default()
}

/// Parse back: a bare integer is a count, "dynamic" stays literal, anything
/// else is a comma-separated name list. Empty text writes nothing.
fn store_workspaces(doc: &mut ConfigDocument, path: &[&str], text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Ok(count) = trimmed.parse::<i64>() {
        doc.set_integer(path, count);
    } else if trimmed == "dynamic" {
        doc.set_string(path, "dynamic");
    } else {
        let names: Vec<String> = trimmed
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        if !names.is_empty() {
            doc.set_strings(path, &names);
        }
    }
}

/// Render one schema entry, writing changes straight through to the document.
fn entry_row(ui: &mut egui::Ui, doc: &mut ConfigDocument, entry: &schema::Entry) {
    let parts: Vec<&str> = entry.path.iter().map(String::as_str).collect();
    let label = if entry.restart {
        format!("{} (restart to apply)", entry.label)
    } else {
        entry.label.clone()
    };
    match &entry.kind {
        schema::Kind::Bool => {
            let mut value = doc.get_bool(&parts).unwrap_or(match entry.default {
                Some(schema::Value::Bool(value)) => value,
                _ => false,
            });
            if ui.checkbox(&mut value, &label).changed() {
                doc.set_bool(&parts, value);
            }
        }
        schema::Kind::Integer { min, max } => {
            let mut value = doc.get_integer(&parts).unwrap_or(match entry.default {
                Some(schema::Value::Integer(value)) => value,
                _ => 0,
            });
            let changed = match (min, max) {
                (Some(min), Some(max)) => ui
                    .add(egui::Slider::new(&mut value, *min..=*max).text(&label))
                    .changed(),
                _ => ui
                    .horizontal(|ui| {
                        ui.label(&label);
                        ui.add(egui::DragValue::new(&mut value))
                    })
                    .inner
                    .changed(),
            };
            if changed {
                doc.set_integer(&parts, value);
            }
        }
        schema::Kind::Float { min, max } => {
            let mut value = doc.get_float(&parts).unwrap_or(match entry.default {
                Some(schema::Value::Float(value)) => value,
                _ => 0.0,
            });
            let changed = match (min, max) {
                (Some(min), Some(max)) => ui
                    .add(egui::Slider::new(&mut value, *min..=*max).text(&label))
                    .changed(),
                _ => ui
                    .horizontal(|ui| {
                        ui.label(&label);
                        ui.add(egui::DragValue::new(&mut value).speed(0.01))
                    })
                    .inner
                    .changed(),
            };
            if changed {
                doc.set_float(&parts, value);
            }
        }
        schema::Kind::Text => {
            let mut value = doc
                .get_string(&parts)
                .unwrap_or_else(|| match &entry.default {
                    Some(schema::Value::Text(value)) => value.clone(),
                    _ => String::new(),
                });
            let changed = ui
                .horizontal(|ui| {
                    ui.label(&label);
                    ui.text_edit_singleline(&mut value)
                })
                .inner
                .changed();
            if changed {
                doc.set_string(&parts, &value);
            }
        }
        schema::Kind::List => {
            let mut text = array_text(doc, &parts).unwrap_or_default();
            let changed = ui
                .horizontal(|ui| {
                    ui.label(&label);
                    ui.text_edit_singleline(&mut text)
                        .on_hover_text("Comma-separated list")
                        .changed()
                })
                .inner;
            if changed {
                store_array(doc, &parts, &text);
            }
        }
        schema::Kind::Choice(options) => {
            let mut value = doc
                .get_string(&parts)
                .unwrap_or_else(|| match &entry.default {
                    Some(schema::Value::Text(value)) => value.clone(),
                    _ => String::new(),
                });
            let original = value.clone();
            ui.horizontal(|ui| {
                ui.label(&label);
                egui::ComboBox::from_id_salt(entry.dotted())
                    .selected_text(&value)
                    .show_ui(ui, |ui| {
                        for option in options {
                            ui.selectable_value(&mut value, option.clone(), option);
                        }
                    });
            });
            if value != original {
                doc.set_string(&parts, &value);
            }
        }
        schema::Kind::Color => {
            let current = doc
                .get_string(&parts)
                .unwrap_or_else(|| match &entry.default {
                    Some(schema::Value::Text(value)) => value.clone(),
                    _ => String::new(),
                });
            let mut color = parse_color(&current).unwrap_or(egui::Color32::from_rgb(255, 255, 255));
            let changed = ui
                .horizontal(|ui| {
                    ui.label(&label);
                    ui.color_edit_button_srgba(&mut color).changed()
                })
                .inner;
            if changed {
                doc.set_string(&parts, &color_to_hex(color));
            }
        }
    }
}

/// One otherwise-uncovered key: scalars edit by type, arrays edit as
/// comma-separated text in their element type; tables stay read-only
/// notes.
fn raw_row(ui: &mut egui::Ui, doc: &mut ConfigDocument, dotted: &str) {
    let parts: Vec<&str> = dotted.split('.').collect();
    if let Some(mut value) = doc.get_bool(&parts) {
        if ui.checkbox(&mut value, dotted).changed() {
            doc.set_bool(&parts, value);
        }
        return;
    }
    if let Some(mut value) = doc.get_integer(&parts) {
        let changed = ui
            .horizontal(|ui| {
                ui.label(dotted);
                ui.add(egui::DragValue::new(&mut value))
            })
            .inner
            .changed();
        if changed {
            doc.set_integer(&parts, value);
        }
        return;
    }
    if let Some(mut value) = doc.get_float(&parts) {
        let changed = ui
            .horizontal(|ui| {
                ui.label(dotted);
                ui.add(egui::DragValue::new(&mut value).speed(0.01))
            })
            .inner
            .changed();
        if changed {
            doc.set_float(&parts, value);
        }
        return;
    }
    if let Some(mut value) = doc.get_string(&parts) {
        let changed = ui
            .horizontal(|ui| {
                ui.label(dotted);
                ui.text_edit_singleline(&mut value)
            })
            .inner
            .changed();
        if changed {
            doc.set_string(&parts, &value);
        }
        return;
    }
    ui.label(format!(
        "{dotted}  (table — edit the file directly for now)"
    ));
}

/// Comma-separated text for an array value, in its element type. An empty
/// array matches the integer getter first and edits as numbers-or-names.
fn array_text(doc: &ConfigDocument, path: &[&str]) -> Option<String> {
    if let Some(values) = doc.get_integers(path) {
        return Some(
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(values) = doc.get_floats(path) {
        return Some(
            values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    doc.get_strings(path).map(|values| values.join(", "))
}

/// Parse back: all-integers stay integers, all-numbers become floats,
/// anything else is a name list. Empty text writes an empty string array.
fn store_array(doc: &mut ConfigDocument, path: &[&str], text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        doc.set_strings(path, &[]);
        return;
    }
    let items: Vec<String> = trimmed
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_owned)
        .collect();
    if items.iter().all(|item| item.parse::<i64>().is_ok()) {
        let values: Vec<i64> = items.iter().map(|item| item.parse().unwrap()).collect();
        doc.set_integers(path, &values);
    } else if items.iter().all(|item| item.parse::<f64>().is_ok()) {
        let values: Vec<f64> = items.iter().map(|item| item.parse().unwrap()).collect();
        doc.set_floats(path, &values);
    } else {
        doc.set_strings(path, &items);
    }
}

/// `#RRGGBB[AA]` to an egui color.
fn parse_color(text: &str) -> Option<egui::Color32> {
    let hex = text.strip_prefix('#')?;
    if (hex.len() != 6 && hex.len() != 8) || !hex.is_ascii() {
        return None;
    }
    let channel = |range: std::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
    Some(egui::Color32::from_rgba_unmultiplied(
        channel(0..2)?,
        channel(2..4)?,
        channel(4..6)?,
        channel(6..8).unwrap_or(255),
    ))
}

fn color_to_hex(color: egui::Color32) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.r(),
        color.g(),
        color.b(),
        color.a()
    )
}

/// The per-dropdown search buffer, persisted in egui's memory under `id`
/// so each dropdown remembers its own filter.
fn combo_filter(ui: &mut egui::Ui, id: egui::Id) -> String {
    ui.memory_mut(|mem| {
        let filter = mem
            .data
            .get_temp_mut_or_insert_with(id, String::new)
            .clone();
        mem.data.insert_temp(id, filter.clone());
        filter
    })
}

/// egui key → xkb keysym name for recorded chords; `None` for keys that
/// cannot start a chord (clipboard commands and unmapped extras).
fn key_name(key: egui::Key) -> Option<String> {
    use egui::Key as K;
    let debug = format!("{key:?}");
    // Letters render as one capital letter ("A".."Z") in Debug.
    if debug.len() == 1 {
        return Some(debug.to_lowercase());
    }
    // Digits render as "Num0".."Num9".
    if let Some(digit) = debug.strip_prefix("Num") {
        return Some(digit.to_owned());
    }
    let name = match key {
        K::Enter => "Return",
        K::Space => "space",
        K::Tab => "Tab",
        K::Backspace => "BackSpace",
        K::ArrowLeft => "Left",
        K::ArrowRight => "Right",
        K::ArrowUp => "Up",
        K::ArrowDown => "Down",
        K::Home => "Home",
        K::End => "End",
        K::PageUp => "Page_Up",
        K::PageDown => "Page_Down",
        K::Insert => "Insert",
        K::Delete => "Delete",
        K::Backtick => "grave",
        K::Minus => "minus",
        K::Equals => "equal",
        K::Comma => "comma",
        K::Period => "period",
        K::Slash => "slash",
        K::Backslash => "backslash",
        K::Semicolon => "semicolon",
        K::Quote => "apostrophe",
        K::OpenBracket => "bracketleft",
        K::CloseBracket => "bracketright",
        K::OpenCurlyBracket => "braceleft",
        K::CloseCurlyBracket => "braceright",
        K::Colon => "colon",
        K::Pipe => "bar",
        K::Questionmark => "question",
        K::Exclamationmark => "exclam",
        K::Plus => "plus",
        K::F1 => "F1",
        K::F2 => "F2",
        K::F3 => "F3",
        K::F4 => "F4",
        K::F5 => "F5",
        K::F6 => "F6",
        K::F7 => "F7",
        K::F8 => "F8",
        K::F9 => "F9",
        K::F10 => "F10",
        K::F11 => "F11",
        K::F12 => "F12",
        _ => return None,
    };
    Some(name.to_owned())
}
