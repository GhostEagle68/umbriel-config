//! First-run onboarding and the guided setup walk.

use super::common::*;
use super::rows::schema_row;
use super::*;

/// Minimal starter written by the onboarding panel: comments only, so it
/// is valid TOML and loads as an empty healthy document.
const STARTER_CONFIG: &str = "\
# umbriel configuration — created by Umbriel Config.
# Reference: https://github.com/noctalia-dev/umbriel
";

/// The four startup states: only the plain install gets the first-run
/// panel; a configured machine without umbriel gets the quiet banner.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SetupMode {
    Normal,
    FreshWithUmbriel,
    PlainInstall,
    MissingUmbriel,
}

/// "Umbriel present" is the packaged default being installed; "config
/// exists" is the main config file being on disk.
pub(super) fn setup_mode(umbriel_present: bool, config_exists: bool) -> SetupMode {
    match (umbriel_present, config_exists) {
        (true, true) => SetupMode::Normal,
        (true, false) => SetupMode::FreshWithUmbriel,
        (false, true) => SetupMode::MissingUmbriel,
        (false, false) => SetupMode::PlainInstall,
    }
}

/// One guided-setup walk: the curated cards, in visit order. Each step is
/// its title plus the curated key definitions it shows.
pub(super) struct Guide {
    pub(super) steps: Vec<(String, Vec<&'static GuideKey>)>,
    pub(super) index: usize,
}

/// The guided-setup steps: a hand-picked handful of the most-wanted
/// settings per card (outputs is prepended when monitors are detected;
/// keybinds joins when its editor exists). Each key carries a plain
/// label and a one-line description — the guide speaks human, not
/// config-file. Keys missing from the installed schema are skipped.
pub(super) struct GuideKey {
    pub(super) key: &'static str,
    pub(super) label: &'static str,
    pub(super) description: &'static str,
}

const GUIDE_STEPS: &[(&str, &[GuideKey])] = &[
    (
        "layout",
        &[
            GuideKey {
                key: "layout.mode",
                label: "Layout mode",
                description: "How windows are arranged, scrolling columns or the classic dwindle / master trees.",
            },
            GuideKey {
                key: "layout.gap",
                label: "Gap",
                description: "Space between windows",
            },
            GuideKey {
                key: "layout.scrolling.default_width_fraction",
                label: "New column width",
                description: "How much of the screen a new column starts with.",
            },
        ],
    ),
    (
        "input",
        &[
            GuideKey {
                key: "input.keyboard.layout",
                label: "Keyboard layout",
                description: "Country layout for your keyboard, e.g. us or de,us.",
            },
            GuideKey {
                key: "input.keyboard.numlock_toggle",
                label: "Num Lock on connect",
                description: "Turn Num Lock on whenever a keyboard connects.",
            },
            GuideKey {
                key: "input.middle_click_paste",
                label: "Middle-click paste",
                description: "Paste the selected text with a middle click.",
            },
            GuideKey {
                key: "input.touchpad.tap",
                label: "Tap to click",
                description: "Tapping the touchpad counts as a click.",
            },
            GuideKey {
                key: "input.cursor.size",
                label: "Cursor size",
                description: "Mouse cursor size.",
            },
        ],
    ),
    (
        "general",
        &[
            GuideKey {
                key: "general.xwayland",
                label: "XWayland",
                description: "Run X11 apps through xwayland-satellite. Changing this needs a fresh Umbriel session.",
            },
            GuideKey {
                key: "general.show_cheatsheet",
                label: "Cheat sheet at start",
                description: "Show the keybind overlay when the session starts.",
            },
            GuideKey {
                key: "general.focus_on_activate",
                label: "Let apps take focus",
                description: "Allow newly opened apps to demand focus.",
            },
            GuideKey {
                key: "general.autostart",
                label: "Autostart commands",
                description: "Commands run once when the session starts.",
            },
        ],
    ),
    (
        "appearance",
        &[
            GuideKey {
                key: "appearance.border_width",
                label: "Border width",
                description: "Thickness of the window border, in logical pixels.",
            },
            GuideKey {
                key: "appearance.corner_radius",
                label: "Corner radius",
                description: "How rounded the window corners are.",
            },
            GuideKey {
                key: "appearance.blur.radius",
                label: "Blur radius",
                description: "How far the blur spreads behind windows.",
            },
            GuideKey {
                key: "appearance.blur.passes",
                label: "Blur passes",
                description: "How many times the blur is applied, more is smoother.",
            },
            GuideKey {
                key: "colors.border.focused",
                label: "Focused border color",
                description: "Border color of the focused window, as hex (e.g. #7AA3FFFF).",
            },
        ],
    ),
];

/// Card heading for one guided-setup step.
fn guide_title(section: &str, step: usize, total: usize) -> String {
    let display = if section == "output" {
        "outputs".to_owned()
    } else {
        let mut owned = section.to_owned();
        if let Some(first) = owned.get_mut(..1) {
            first.make_ascii_uppercase();
        }
        owned
    };
    format!("Guided setup — {display} (step {step} of {total})")
}

/// Arm the guided walk: the schema sections get suggested defaults, and
/// any detected monitors become an outputs step with their live state
/// (enabled, mode, scale, position) plus umbriel's defaults filled in.
/// `false` when there is no schema at all (no umbriel detected).
pub(super) fn start_guide(shell: &mut Shell) -> bool {
    // Pre-existing values are never clobbered: suggested defaults and
    // detected state only fill settings that are still unset. That makes
    // the walk safe to re-run from Settings.
    let main = shell.includes.docs.len();
    let current: BTreeMap<String, String> = doc_at(shell, main).leaf_values().into_iter().collect();
    let mut steps: Vec<(String, Vec<&'static GuideKey>)> = Vec::new();
    for (title, metas) in GUIDE_STEPS {
        let mut keys: Vec<&'static GuideKey> = Vec::new();
        for meta in metas.iter() {
            let Some(entry) = shell.schema.iter().find(|entry| entry.dotted() == meta.key) else {
                continue; // not in this umbriel's schema
            };
            keys.push(meta);
            if current.contains_key(meta.key) {
                continue;
            }
            let Some(default) = entry.default.as_ref() else {
                continue;
            };
            let default_text = match default {
                schema::Value::Bool(v) => format!("{v}"),
                schema::Value::Integer(v) => format!("{v}"),
                schema::Value::Float(v) => format!("{v}"),
                schema::Value::Text(v) => v.clone(),
            };
            let Ok(value_text) = super::rows::commit_value(Some(&entry.kind), &default_text) else {
                continue;
            };
            shell.doc.set_leaf_text(meta.key, &value_text);
        }
        if !keys.is_empty() {
            steps.push(((*title).to_owned(), keys));
        }
    }
    // Everything the app can detect is filled in the same way: each
    // connected monitor joins the walk as an outputs card — first, since
    // output setup is the most important decision.
    let monitors = live::outputs().unwrap_or_default();
    for monitor in &monitors {
        let name = monitor.name.as_str();
        let is_set = |field: &str| current.contains_key(&format!("output.{name}.{field}"));
        if !is_set("enabled") {
            shell
                .doc
                .set_bool(&["output", name, "enabled"], monitor.enabled);
        }
        if !is_set("mode")
            && let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index))
        {
            shell
                .doc
                .set_string(&["output", name, "mode"], &mode.label());
        }
        if !is_set("scale") {
            shell
                .doc
                .set_float(&["output", name, "scale"], monitor.scale);
        }
        if !is_set("position") {
            shell.doc.set_integers(
                &["output", name, "position"],
                &[i64::from(monitor.position.0), i64::from(monitor.position.1)],
            );
        }
        for field in outputs::FIELDS {
            if matches!(field.key, "enabled" | "mode" | "scale" | "position") {
                continue; // detection fills these
            }
            if is_set(field.key) {
                continue;
            }
            let path = ["output", name, field.key];
            match &field.default {
                Some(outputs::DefaultValue::Bool(value)) => {
                    shell.doc.set_bool(&path, *value);
                }
                Some(outputs::DefaultValue::Float(value)) => {
                    shell.doc.set_float(&path, *value);
                }
                Some(outputs::DefaultValue::Text(value)) => {
                    shell.doc.set_string(&path, value);
                }
                None => {}
            }
        }
    }
    if !monitors.is_empty() {
        steps.insert(0, ("output".to_owned(), Vec::new()));
    }
    if steps.is_empty() {
        return false;
    }
    // The filled-in values become the saved baseline: a row only shows
    // its dot once the user moves away from the suggestion.
    shell.reset_saved();
    shell.guide_monitors = monitors;
    shell.guide = Some(Guide { steps, index: 0 });
    true
}

/// Render the guide's current step into the cards model and update
/// the card heading/controls.
pub(super) fn guide_show_step(app: &AppWindow, shell: &Shell) {
    let Some(guide) = shell.guide.as_ref() else {
        return;
    };
    let (title, metas) = &guide.steps[guide.index];
    if title == "output" {
        app.set_cards(Rc::new(VecModel::from(super::page_outputs::output_cards(shell))).into());
    } else {
        app.set_cards(Rc::new(VecModel::from(guide_cards(shell, metas))).into());
    }
    app.set_guide_title(guide_title(title, guide.index + 1, guide.steps.len()).into());
    app.set_guide_first(guide.index == 0);
    app.set_guide_last(guide.index == guide.steps.len() - 1);
}

/// One guided-setup card: the step's curated keys in order, resolved to
/// schema rows (keys this umbriel's schema lacks are skipped). The
/// curated label and description replace the mined ones — the guide
/// speaks human, not config-file.
fn guide_cards(shell: &Shell, metas: &[&'static GuideKey]) -> Vec<SettingsCard> {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let main = shell.includes.docs.len();
    let current: Vec<BTreeMap<String, String>> = (0..=main)
        .map(|i| doc_at(shell, i).leaf_values().into_iter().collect())
        .collect();
    let rows: Vec<SettingRow> = metas
        .iter()
        .filter_map(|meta| shell.schema.iter().find(|entry| entry.dotted() == meta.key))
        .map(|entry| {
            let mut row = schema_row(shell, &sets, &labels, &current, entry);
            if let Some(meta) = metas.iter().find(|meta| meta.key == entry.dotted()) {
                row.label = meta.label.into();
                row.hint = meta.description.into();
            }
            // A first-time walk has no use for the "new key" and owning-file
            // badges; every row would carry them.
            row.is_new = false;
            row.home = -1;
            row
        })
        .collect();
    vec![SettingsCard {
        title: String::new().into(),
        key: String::new().into(),
        expanded: true,
        rows: Rc::new(VecModel::from(rows)).into(),
    }]
}

pub(super) fn install_guide(app: &AppWindow, shell: &Rc<RefCell<Shell>>, env: &discovery::Env) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_onboarding_create(move || {
            let Some(app) = weak.upgrade() else { return };
            // The panel only shows while the file is absent; never clobber
            // a config that appeared meanwhile.
            let created = {
                let shell = shell.borrow_mut();
                if shell.path.exists() {
                    app.set_show_onboarding(false);
                    return;
                }
                if let Some(parent) = shell.path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::write(&shell.path, STARTER_CONFIG) {
                    Ok(()) => {
                        app.set_show_onboarding(false);
                        app.set_status("Created config.toml.".into());
                        true
                    }
                    Err(err) => {
                        app.set_status(format!("Couldn't create config.toml: {err}").into());
                        false
                    }
                }
            };
            if !created {
                return;
            }
            // With a schema source, walk the frequent sections with the
            // suggested defaults filled in; without one there is nothing
            // to guide through (the empty state explains).
            if !start_guide(&mut shell.borrow_mut()) {
                return;
            }
            guide_show_step(&app, &shell.borrow());
            // First run: the config was just created, so there is nothing
            // to go back to.
            app.set_guide_cancellable(false);
            app.set_show_guide(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_guide_back(move || {
            let Some(app) = weak.upgrade() else { return };
            {
                let mut shell = shell.borrow_mut();
                let Some(guide) = shell.guide.as_mut() else {
                    return;
                };
                if guide.index == 0 {
                    return;
                }
                guide.index -= 1;
            }
            guide_show_step(&app, &shell.borrow());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_guide_relaunch(move || {
            let Some(app) = weak.upgrade() else { return };
            if app.get_dirty() {
                app.set_status(
                    "Save or discard your changes before running the guided setup.".into(),
                );
                return;
            }
            if !start_guide(&mut shell.borrow_mut()) {
                app.set_status(
                    "Nothing to guide through yet — install umbriel and sync the schema.".into(),
                );
                return;
            }
            guide_show_step(&app, &shell.borrow());
            app.set_guide_cancellable(true);
            app.set_show_guide(true);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_guide_cancel(move || {
            let Some(app) = weak.upgrade() else { return };
            let mut shell = shell.borrow_mut();
            // Drop the filled-in suggestions and restore the on-disk
            // baseline that start_guide replaced.
            shell.doc.discard();
            for inc in &mut shell.includes.docs {
                inc.doc.discard();
            }
            shell.reset_saved();
            shell.guide = None;
            app.set_show_guide(false);
            super::sections::refresh_shown_page(&app, &shell);
            app.set_status("Guided setup cancelled.".into());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        let env = env.clone();
        app.on_guide_next(move || {
            let Some(app) = weak.upgrade() else { return };
            // Advance one step, or — on the last — write the config and
            // land in the app. Borrowed in an inner scope so the step
            // data outlives the &mut.
            let advance = {
                let mut shell = shell.borrow_mut();
                let Some(guide) = shell.guide.as_mut() else {
                    return;
                };
                guide.index += 1;
                guide.index < guide.steps.len()
            };
            if advance {
                guide_show_step(&app, &shell.borrow());
                return;
            }
            // Finish: write every modified doc, then validate through
            // umbriel (absent installs skip validation gracefully).
            let mut shell = shell.borrow_mut();
            if super::save::save_all_and_validate(&app, &mut shell, &env) {
                shell.guide = None;
                app.set_show_guide(false);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_mode_covers_the_four_states() {
        assert_eq!(setup_mode(true, true), SetupMode::Normal);
        assert_eq!(setup_mode(true, false), SetupMode::FreshWithUmbriel);
        assert_eq!(setup_mode(false, true), SetupMode::MissingUmbriel);
        assert_eq!(setup_mode(false, false), SetupMode::PlainInstall);
    }

    #[test]
    fn starter_config_loads_as_empty_healthy_document() {
        let doc = ConfigDocument::from_str(STARTER_CONFIG).expect("starter parses");
        assert!(doc.leaf_values().is_empty());
        assert!(!doc.is_modified());
    }
}
