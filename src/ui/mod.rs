//! egui shell: loads the resolved config, tracks modifications, saves with
//! validation. Pages render from the schema assembled from umbriel's
//! packaged default config; changes write through to the document.

use eframe::egui;
use std::path::PathBuf;
use std::str::FromStr;
use umbriel_config::config::{
    discovery, document::ConfigDocument, outputs, schema, state, validate,
};

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
    /// Active settings-search query; empty means browse normally.
    search: String,
    /// Notice from schema sync or the startup drift check; dismissable.
    schema_note: Option<String>,
}

impl App {
    fn new(path: PathBuf) -> Self {
        let (doc, healthy, load_error) = match ConfigDocument::load(&path) {
            Ok(doc) => (doc, true, None),
            Err(err) => (
                // An empty document keeps the UI alive; saving stays disabled
                // so a broken file is never overwritten from here.
                ConfigDocument::from_str("").expect("empty TOML parses"),
                false,
                Some(err.to_string()),
            ),
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
            schema_note,
            search: String::new(),
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
        if let Some(Page::Section(current)) = self.page.as_ref()
            && !self.sections().contains(current)
        {
            self.page = None;
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

    /// Schema from the installed packaged default; empty when unavailable.
    fn load_schema(env: &discovery::Env) -> Vec<schema::Entry> {
        discovery::packaged_default(env)
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| schema::assemble(&text))
            .unwrap_or_default()
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
                        outputs_page(ui, &mut self.doc, &mut self.add_output);
                    });
                }
            }
        });
    }
}

/// Outputs page: one group per configured output plus add-by-name. Renders
/// the possibility space, never just what is set — an empty list invites.
fn outputs_page(ui: &mut egui::Ui, doc: &mut ConfigDocument, add_output: &mut String) {
    let names = outputs::configured(doc);
    if names.is_empty() {
        ui.label(
            "No outputs configured yet. Add one below to set its mode, scale,\n\
             or workspace names — umbriel's defaults apply until then.",
        );
        ui.add_space(8.0);
    }
    for name in &names {
        egui::CollapsingHeader::new(name)
            .default_open(names.len() == 1)
            .show(ui, |ui| {
                for field in outputs::FIELDS {
                    output_field_row(ui, doc, name, field);
                }
            });
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label("Add output");
        ui.text_edit_singleline(add_output)
            .on_hover_text("Connector name, e.g. DP-3 or eDP-1");
        if ui.button("Add").clicked() {
            let name = add_output.trim().to_owned();
            if !name.is_empty() && !names.contains(&name) {
                doc.set_bool(&["output", &name, "enabled"], true);
                *add_output = String::new();
            }
        }
    });
}

/// One field of one output, writing through on change like `entry_row`.
fn output_field_row(
    ui: &mut egui::Ui,
    doc: &mut ConfigDocument,
    name: &str,
    field: &outputs::Field,
) {
    let path = ["output", name, field.key];
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
    }
}
