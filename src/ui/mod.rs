//! egui shell: loads the resolved config, tracks modifications, saves with
//! validation. Pages render from the schema assembled from umbriel's
//! packaged default config; changes write through to the document.

use std::path::PathBuf;
use std::str::FromStr;

use eframe::egui;

use umbriel_config::config::{discovery, document::ConfigDocument, schema, validate};

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
    /// Selected top-level section, e.g. `"general"`.
    section: Option<String>,
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
        let schema = discovery::packaged_default(&discovery::Env::from_process())
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| schema::assemble(&text))
            .unwrap_or_default();
        Self {
            path,
            doc,
            healthy,
            load_error,
            last_validation: None,
            validation_note: None,
            schema,
            section: None,
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
                ui.label(self.path.display().to_string());
                if self.doc.is_modified() {
                    ui.colored_label(egui::Color32::from_rgb(230, 180, 80), "unsaved changes");
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
        egui::Panel::left("sidebar").show(ui, |ui| {
            let sections = self.sections();
            ui.vertical(|ui| {
                for section in &sections {
                    ui.selectable_value(
                        &mut self.section,
                        Some(section.clone()),
                        schema::humanize(section),
                    );
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
            if self.schema.is_empty() {
                ui.label(
                    "No umbriel schema found.\n\
                     Install umbriel (or its packaged default config) and reopen.",
                );
                return;
            }
            let sections = self.sections();
            let Some(section) = self.section.clone().or_else(|| sections.first().cloned()) else {
                return;
            };
            self.section = Some(section.clone());
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
        });
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
