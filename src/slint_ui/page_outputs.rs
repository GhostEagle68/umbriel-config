//! The Outputs page: configured + detected monitors, per-field editors
//! with detected resolution/refresh dropdowns, and the guide's outputs
//! step.

use super::common::*;
use super::*;

/// The outputs guide step: one card per configured monitor, its fields
/// editable through the standard row editors.
pub(super) fn output_cards(shell: &Shell) -> Vec<SettingsCard> {
    let current: BTreeMap<String, String> = doc_at(shell, shell.includes.docs.len())
        .leaf_values()
        .into_iter()
        .collect();
    output_names(shell)
        .iter()
        .map(|name| {
            let key = format!("card:output:{name}");
            SettingsCard {
                title: name.clone().into(),
                key: key.clone().into(),
                expanded: card_expanded(shell, &key, true),
                rows: Rc::new(VecModel::from(output_monitor_rows(shell, name, &current))).into(),
            }
        })
        .collect()
}

/// Detected-then-configured output names, deduplicated.
fn output_names(shell: &Shell) -> Vec<String> {
    let main = shell.includes.docs.len();
    let mut names = outputs::configured(doc_at(shell, main));
    for monitor in &shell.guide_monitors {
        if !names.contains(&monitor.name) {
            names.push(monitor.name.clone());
        }
    }
    names
}

/// One monitor's field rows; with detected modes the single mode string
/// splits into resolution and refresh dropdowns.
fn output_monitor_rows(
    shell: &Shell,
    name: &str,
    current: &BTreeMap<String, String>,
) -> Vec<SettingRow> {
    let monitor = shell
        .guide_monitors
        .iter()
        .find(|monitor| monitor.name == name);
    let mut rows: Vec<SettingRow> = Vec::new();
    for field in outputs::FIELDS {
        if field.key == "mode"
            && let Some(detected) = monitor.filter(|monitor| !monitor.modes.is_empty())
        {
            rows.push(resolution_choice_row(shell, name, detected, current));
            rows.push(refresh_choice_row(shell, name, detected, current));
            continue;
        }
        rows.push(output_row(shell, name, field, current));
    }
    let home_label = setting_labels(shell)
        .get(shell.includes.docs.len())
        .cloned()
        .unwrap_or_default();
    for row in &mut rows {
        row.home_label = home_label.clone();
    }
    rows
}

/// The outputs page model: one card per monitor with its live info.
fn monitor_cards(shell: &Shell) -> Vec<MonitorCard> {
    let main = shell.includes.docs.len();
    let current: BTreeMap<String, String> = doc_at(shell, main).leaf_values().into_iter().collect();
    let configured = outputs::configured(doc_at(shell, main));
    let removable = configured.len() > 1;
    output_names(shell)
        .iter()
        .map(|name| {
            let monitor = shell
                .guide_monitors
                .iter()
                .find(|monitor| monitor.name == *name);
            let mut info: Vec<String> = Vec::new();
            if let Some(monitor) = monitor {
                if !monitor.description.is_empty() {
                    info.push(monitor.description.clone());
                }
                if let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index)) {
                    info.push(format!("Currently {}", mode.label()));
                }
            }
            MonitorCard {
                name: name.clone().into(),
                info: info.join(" · ").into(),
                connected: monitor.is_some(),
                configured: configured.contains(name),
                expanded: card_expanded(shell, &format!("monitor:{name}"), true),
                removable,
                rows: Rc::new(VecModel::from(output_monitor_rows(shell, name, &current))).into(),
            }
        })
        .collect()
}

/// Rescan the compositor's monitors; failures become the page's note.
pub(super) fn scan_outputs(shell: &mut Shell) {
    match live::outputs() {
        Ok(list) => {
            shell.guide_monitors = list;
            shell.live_note = String::new();
        }
        Err(err) => shell.live_note = format!("live state unavailable: {err}"),
    }
}

pub(super) fn rebuild_outputs(app: &AppWindow, shell: &Shell) {
    app.set_monitors(Rc::new(VecModel::from(monitor_cards(shell))).into());
    app.set_outputs_live_note(shell.live_note.clone().into());
    app.set_changed_count(changed_count(shell));
}

/// Distinct resolutions (width x height) a monitor reports, in report order.
fn resolutions(monitor: &live::LiveOutput) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for mode in &monitor.modes {
        let label = format!("{}x{}", mode.width, mode.height);
        if !out.contains(&label) {
            out.push(label);
        }
    }
    out
}

/// Refresh texts a monitor offers at one resolution, formatted like the
/// mode labels ("164.977").
fn refreshes(monitor: &live::LiveOutput, resolution: &str) -> Vec<String> {
    monitor
        .modes
        .iter()
        .filter(|mode| format!("{}x{}", mode.width, mode.height) == resolution)
        .map(|mode| (mode.refresh_mhz as f64 / 1000.0).to_string())
        .collect()
}

/// The mode string of one configured output, split into resolution and
/// refresh halves.
fn mode_parts(doc: &ConfigDocument, name: &str) -> (String, String) {
    match doc
        .get_string(&["output", name, "mode"])
        .unwrap_or_default()
        .split_once('@')
    {
        Some((resolution, refresh)) => (resolution.to_owned(), refresh.to_owned()),
        None => (String::new(), String::new()),
    }
}

pub(super) fn resolution_choice_row(
    shell: &Shell,
    name: &str,
    monitor: &live::LiveOutput,
    current: &BTreeMap<String, String>,
) -> SettingRow {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let (mut resolution, _) = mode_parts(doc, name);
    if resolution.is_empty()
        && let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index))
    {
        resolution = format!("{}x{}", mode.width, mode.height);
    }
    let key = format!("output.{name}.resolution");
    let mut row = blank_row(key.clone(), "Resolution", shell.includes.docs.len() as i32);
    row.kind = ValueKind::Choice;
    row.choices = Rc::new(VecModel::from(
        resolutions(monitor)
            .iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
    .into();
    row.value = resolution.into();
    row.hint = "Modes your monitor reports — the detected one is selected.".into();
    row.changed = current.get(&key) != shell.saved.get(main).and_then(|values| values.get(&key));
    row
}

pub(super) fn refresh_choice_row(
    shell: &Shell,
    name: &str,
    monitor: &live::LiveOutput,
    current: &BTreeMap<String, String>,
) -> SettingRow {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let (mut resolution, mut refresh) = mode_parts(doc, name);
    if resolution.is_empty()
        && let Some(mode) = monitor.current.and_then(|index| monitor.modes.get(index))
    {
        resolution = format!("{}x{}", mode.width, mode.height);
        refresh = (mode.refresh_mhz as f64 / 1000.0).to_string();
    }
    let key = format!("output.{name}.refresh");
    let mut row = blank_row(
        key.clone(),
        "Refresh rate",
        shell.includes.docs.len() as i32,
    );
    row.kind = ValueKind::Choice;
    row.choices = Rc::new(VecModel::from(
        refreshes(monitor, &resolution)
            .iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
    .into();
    row.value = refresh.into();
    row.hint = "Refresh rates your monitor supports at this resolution, in Hz.".into();
    row.changed = current.get(&key) != shell.saved.get(main).and_then(|values| values.get(&key));
    row
}

/// Plain-language description of an output field, shown under the label.
fn output_field_hint(key: &str) -> &'static str {
    match key {
        "enabled" => "This monitor is active, untick to switch it off.",
        "mode" => "Resolution and refresh rate combined, e.g. 3440x1440@164.977.",
        "position" => "Where the monitor sits, as x, y. Filled from your current layout.",
        "scale" => "Zoom level; 1.0 is native size.",
        "vrr" => {
            "Variable refresh rate (FreeSync/G-Sync). Recommended: fullscreen while gaming, otherwise disabled."
        }
        "hdr" => "HDR when the content supports it. Recommended: auto on HDR monitors.",
        "sdr_white" => "Brightness of normal (SDR) content in HDR mode, in nits. Recommended: 203.",
        "transform" => "Rotate or flip the display. Recommended: normal.",
        "tearing" => "Allow tearing for lower input lag. Recommended: off.",
        "direct_scanout" => "Send fullscreen apps straight to the display. Recommended: on.",
        "workspaces" => "Workspace names for this monitor; dynamic keeps them automatic.",
        _ => "",
    }
}

/// One output field as a row: the fixed vocabulary mapped onto editors
/// (toggle → checkbox, choice → dropdown, float → slider, the rest text).
pub(super) fn output_row(
    shell: &Shell,
    name: &str,
    field: &outputs::Field,
    current: &BTreeMap<String, String>,
) -> SettingRow {
    let main = shell.includes.docs.len();
    let doc = doc_at(shell, main);
    let path = ["output", name, field.key];
    let key = format!("output.{name}.{}", field.key);
    let mut row = blank_row(key.clone(), field.label, shell.includes.docs.len() as i32);
    row.hint = output_field_hint(field.key).into();
    let default_text = match &field.default {
        Some(outputs::DefaultValue::Bool(value)) => value.to_string(),
        Some(outputs::DefaultValue::Float(value)) => value.to_string(),
        Some(outputs::DefaultValue::Text(value)) => (*value).to_owned(),
        None => String::new(),
    };
    match &field.kind {
        outputs::FieldKind::Toggle => {
            let value = doc.get_bool(&path).unwrap_or(matches!(
                field.default,
                Some(outputs::DefaultValue::Bool(true))
            ));
            row.kind = ValueKind::Boolean;
            row.checked = value;
            row.value = value.to_string().into();
        }
        outputs::FieldKind::Choice(vocab) => {
            row.kind = ValueKind::Choice;
            row.choices = Rc::new(VecModel::from(
                vocab
                    .iter()
                    .map(|value| SharedString::from(*value))
                    .collect::<Vec<_>>(),
            ))
            .into();
            row.value = doc
                .get_string(&path)
                .filter(|value| !value.is_empty())
                .unwrap_or(default_text)
                .into();
        }
        outputs::FieldKind::Position => {
            row.kind = ValueKind::Text;
            row.value = match doc.get_integers(&path).unwrap_or_default().as_slice() {
                [x, y] => format!("{x}, {y}").into(),
                _ => String::new().into(),
            };
        }
        outputs::FieldKind::Float { min, max } => {
            row.kind = ValueKind::Float;
            row.min = *min as f32;
            row.max = *max as f32;
            row.value = doc
                .get_float(&path)
                .unwrap_or_else(|| default_text.parse().unwrap_or(0.0))
                .to_string()
                .into();
        }
        outputs::FieldKind::Text => {
            row.value = doc
                .get_string(&path)
                .filter(|value| !value.is_empty())
                .unwrap_or(default_text)
                .into();
        }
        outputs::FieldKind::Workspaces => {
            // The config accepts a count, a name list, or "dynamic" —
            // a plain string read misses the arrays people actually
            // have, so read all three shapes.
            row.value = workspaces_text(doc, &path)
                .filter(|value| !value.is_empty())
                .unwrap_or(default_text)
                .into();
        }
    }
    row.changed = current.get(&key) != shell.saved.get(main).and_then(|values| values.get(&key));
    row
}

/// Format raw editor input for an output field (not schema-backed) into
/// its TOML text form.
pub(super) fn format_output_value(shell: &Shell, key: &str, raw: &str) -> Result<String, String> {
    let field_key = key.rsplit('.').next().unwrap_or_default();
    let name = &key["output.".len()..key.len() - field_key.len() - 1];
    // The detected mode string splits into two dropdowns; both compose
    // the full mode this output saves.
    match field_key {
        "resolution" => {
            let Some(monitor) = shell
                .guide_monitors
                .iter()
                .find(|monitor| monitor.name == name)
            else {
                return Err("no detected monitor for this output".to_owned());
            };
            let matching: Vec<&live::LiveMode> = monitor
                .modes
                .iter()
                .filter(|mode| format!("{}x{}", mode.width, mode.height) == raw)
                .collect();
            let Some(mode) = matching
                .iter()
                .find(|mode| mode.preferred)
                .or_else(|| matching.first())
            else {
                return Err(format!("'{raw}' is not a mode this monitor reports"));
            };
            let refresh = (mode.refresh_mhz as f64 / 1000.0).to_string();
            return Ok(format!("{raw}@{refresh}"));
        }
        "refresh" => {
            let doc = doc_at(shell, shell.includes.docs.len());
            let (resolution, _) = mode_parts(doc, name);
            if resolution.is_empty() {
                return Err("pick a resolution first".to_owned());
            }
            return Ok(format!("{resolution}@{raw}"));
        }
        _ => {}
    }
    let Some(field) = outputs::FIELDS.iter().find(|field| field.key == field_key) else {
        return Err(format!("unknown output field in {key}"));
    };
    let raw = raw.trim();
    match &field.kind {
        outputs::FieldKind::Toggle => match raw {
            "true" | "false" => Ok(raw.to_owned()),
            _ => Err(format!("'{raw}' is not true or false")),
        },
        outputs::FieldKind::Float { min, max } => {
            let value: f64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a number"))?;
            Ok(value.clamp(*min, *max).to_string())
        }
        outputs::FieldKind::Position => {
            let mut parts = raw.split(',');
            let mut xy = [0_i32, 0];
            for slot in &mut xy {
                *slot = parts
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .parse()
                    .map_err(|_| format!("'{raw}' is not an x, y position"))?;
            }
            Ok(format!("[{}, {}]", xy[0], xy[1]))
        }
        outputs::FieldKind::Choice(_) | outputs::FieldKind::Text => {
            super::rows::commit_value(Some(&schema::Kind::Text), raw)
        }
        // A count stays a number, "dynamic" stays literal, names become
        // a TOML array — set_leaf_text parses the text as TOML, so the
        // array form keeps umbriel's list shape.
        outputs::FieldKind::Workspaces => match workspaces_parse(raw)? {
            Some(text) => Ok(text),
            None => {
                Err("workspaces: use comma-separated names, a count, or \"dynamic\"".to_owned())
            }
        },
    }
}

/// Editor text for an output's workspaces: a bare count, comma-joined
/// names, or the literal string.
fn workspaces_text(doc: &ConfigDocument, path: &[&str]) -> Option<String> {
    if let Some(count) = doc.get_integer(path) {
        return Some(count.to_string());
    }
    if let Some(names) = doc.get_strings(path) {
        return Some(names.join(", "));
    }
    doc.get_string(path)
}

/// Parse editor input into the TOML text `set_leaf_text` writes; `None`
/// when nothing sensible remains (empty input).
fn workspaces_parse(raw: &str) -> Result<Option<String>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("workspaces can't be empty — use names, a count, or \"dynamic\"".to_owned());
    }
    if let Ok(count) = trimmed.parse::<i64>() {
        return Ok(Some(count.to_string()));
    }
    if trimmed == "dynamic" {
        return Ok(Some("\"dynamic\"".to_owned()));
    }
    let names: Vec<&str> = trimmed
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        return Err("workspaces can't be empty — use names, a count, or \"dynamic\"".to_owned());
    }
    let listed: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
    Ok(Some(format!("[{}]", listed.join(", "))))
}

pub(super) fn install_outputs(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_outputs_refresh(move || {
            let Some(app) = weak.upgrade() else { return };
            scan_outputs(&mut shell.borrow_mut());
            let shell = shell.borrow();
            rebuild_outputs(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_output_add(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let name = name.trim().to_owned();
            let taken = {
                let shell = shell.borrow();
                let mut names = outputs::configured(&shell.doc);
                for monitor in &shell.guide_monitors {
                    if !names.contains(&monitor.name) {
                        names.push(monitor.name.clone());
                    }
                }
                names.contains(&name) || name.is_empty()
            };
            if taken {
                app.set_status(if name.is_empty() {
                    "Enter a connector name first, e.g. DP-3.".into()
                } else {
                    format!("{name} is already listed").into()
                });
                return;
            }
            shell
                .borrow_mut()
                .doc
                .set_bool(&["output", &name, "enabled"], true);
            shell
                .borrow_mut()
                .card_expanded
                .insert(format!("monitor:{name}"), true);
            app.set_outputs_add_name(String::new().into());
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_outputs(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_output_remove(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let removed = {
                let mut shell = shell.borrow_mut();
                let only = outputs::configured(&shell.doc).len() <= 1;
                if only {
                    None
                } else {
                    shell.doc.remove_table(&["output", &name]).then_some(())
                }
            };
            if removed.is_none() {
                app.set_status("The only configured output cannot be removed.".into());
                return;
            }
            let shell = shell.borrow();
            app.set_dirty(shell.any_modified());
            rebuild_outputs(&app, &shell);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_toggle_monitor(move |name| {
            let Some(app) = weak.upgrade() else { return };
            let key = format!("monitor:{name}");
            let open = !card_expanded(&shell.borrow(), &key, true);
            shell.borrow_mut().card_expanded.insert(key, open);
            let shell = shell.borrow();
            rebuild_outputs(&app, &shell);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspaces_text_reads_count_list_and_literal() {
        let doc = ConfigDocument::from_str(
            "[output.\"DP-3\"]\nworkspaces = [\"Games\", \"Steam\"]\n\n[output.\"eDP-1\"]\nworkspaces = 4\n\n[output.\"HDMI-1\"]\nworkspaces = \"dynamic\"\n",
        )
        .unwrap();
        assert_eq!(
            workspaces_text(&doc, &["output", "DP-3", "workspaces"]).unwrap(),
            "Games, Steam"
        );
        assert_eq!(
            workspaces_text(&doc, &["output", "eDP-1", "workspaces"]).unwrap(),
            "4"
        );
        assert_eq!(
            workspaces_text(&doc, &["output", "HDMI-1", "workspaces"]).unwrap(),
            "dynamic"
        );
        assert_eq!(
            workspaces_text(&doc, &["output", "none", "workspaces"]),
            None
        );
    }

    #[test]
    fn workspaces_parse_emits_toml_set_leaf_text_accepts() {
        // Names become a TOML array the document can parse back.
        let formatted = workspaces_parse("Games, Steam , Util").unwrap().unwrap();
        assert_eq!(formatted, "[\"Games\", \"Steam\", \"Util\"]");
        let mut doc = ConfigDocument::from_str("[output.\"DP-3\"]\n").unwrap();
        assert!(doc.set_leaf_text("output.DP-3.workspaces", &formatted));
        assert_eq!(
            workspaces_text(&doc, &["output", "DP-3", "workspaces"]).unwrap(),
            "Games, Steam, Util"
        );
        // A count stays a number, the literal stays a string.
        assert_eq!(workspaces_parse("4").unwrap().unwrap(), "4");
        assert_eq!(workspaces_parse("dynamic").unwrap().unwrap(), "\"dynamic\"");
        // Empty and comma-only input are rejected, not written.
        assert!(workspaces_parse("").is_err());
        assert!(workspaces_parse(" , ").is_err());
    }
}
