//! egui shell: loads the resolved config, tracks modifications, saves with
//! validation. Pages plug in over `App` in later steps.

use eframe::egui;
use std::{path::PathBuf, str::FromStr};
use umbriel_config::config::{document::ConfigDocument, validate};

/// Launch the GUI with the given config path, or return an error if the config
/// is invalid.
pub fn run(path: PathBuf) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 520.0])
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
        Self {
            path,
            doc,
            healthy,
            load_error,
            last_validation: None,
            validation_note: None,
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
        egui::CentralPanel::default().show(ui, |ui| match &self.load_error {
            Some(error) => {
                ui.colored_label(
                    egui::Color32::from_rgb(240, 100, 100),
                    format!("Could not load config: {error}"),
                );
                ui.label("Fix the file (see `umbriel validate`) and reopen the app.");
            }
            None => {
                ui.heading("Ready");
                ui.label(
                    "Pages arrive in the next steps: General, Appearance, Animation.\n\
                              The loaded document is live:",
                );
                ui.label(format!(
                    "general.xwayland = {:?}",
                    self.doc.get_bool(&["general", "xwayland"])
                ));
            }
        });
    }
}
