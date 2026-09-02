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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    General,
    Appearance,
    Animation,
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
    page: Page,
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
            page: Page::General,
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

    fn general_page(&mut self, ui: &mut egui::Ui) {
        ui.heading("General");
        checkbox(
            ui,
            &mut self.doc,
            &["general", "xwayland"],
            true,
            "Xwayland (restart to apply)",
        );
        checkbox(
            ui,
            &mut self.doc,
            &["general", "show_cheatsheet"],
            true,
            "Show cheatsheet on startup",
        );
        checkbox(
            ui,
            &mut self.doc,
            &["general", "focus_on_activate"],
            false,
            "Focus on activate requests",
        );
        checkbox(
            ui,
            &mut self.doc,
            &["general", "honor_restored_maximize"],
            false,
            "Honor restored maximize",
        );
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
        egui::Panel::left("sidebar").show(ui, |ui| {
            ui.selectable_value(&mut self.page, Page::General, "General");
            ui.selectable_value(&mut self.page, Page::Appearance, "Appearance");
            ui.selectable_value(&mut self.page, Page::Animation, "Animation");
        });
        egui::CentralPanel::default().show(ui, |ui| match &self.load_error {
            Some(error) => {
                ui.colored_label(
                    egui::Color32::from_rgb(240, 100, 100),
                    format!("Could not load config: {error}"),
                );
                ui.label("Fix the file (see `umbriel validate`) and reopen the app.");
            }
            None => match self.page {
                Page::General => self.general_page(ui),
                Page::Appearance => {
                    ui.label("Appearance page arrives in the next step.");
                }
                Page::Animation => {
                    ui.label("Animation page arrives in the next step.");
                }
            },
        });
    }
}

/// Checkbox bound to a config key, falling back to `default` when unset.
/// Changes write through immediately so `is_modified` stays truthful.
fn checkbox(
    ui: &mut egui::Ui,
    doc: &mut ConfigDocument,
    path: &[&str],
    default: bool,
    label: &str,
) {
    let mut value = doc.get_bool(path).unwrap_or(default);
    if ui.checkbox(&mut value, label).changed() {
        doc.set_bool(path, value);
    }
}
