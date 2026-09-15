//! Schema-entry → `SettingRow` rendering, per-row refresh, and the
//! shared write path for text edits and slider releases.

use super::common::*;
use super::page_outputs::{output_row, refresh_choice_row, resolution_choice_row};
use super::*;

/// One schema row, editor-ready: typed value, checked state, swatch, hint.
fn setting_row(doc: &ConfigDocument, entry: &schema::Entry) -> SettingRow {
    let value = typed_value(doc, entry).unwrap_or_else(|| "—".to_owned());
    let parts: Vec<&str> = entry.path.iter().map(String::as_str).collect();
    let (min, max) = kind_bounds(&entry.kind);
    SettingRow {
        label: entry.label.clone().into(),
        value: value.clone().into(),
        key: entry.path.join(".").into(),
        kind: value_kind(&entry.kind),
        choices: choice_model(&entry.kind),
        swatch: if matches!(entry.kind, schema::Kind::Color) {
            swatch_for(&value)
        } else {
            slint::Color::from_argb_u8(0, 0, 0, 0).into()
        },
        checked: doc.get_bool(&parts).unwrap_or(false),
        hint: entry_hint(entry).into(),
        min,
        max,
        home: -1,
        home_label: String::new().into(),
        changed: false,
        available: false,
        is_new: false,
        preview: String::new().into(),
    }
}

/// One schema row resolved against the chain: owner badge, changed dot,
/// NEW badge, availability.
pub(super) fn schema_row(
    shell: &Shell,
    sets: &[BTreeSet<String>],
    labels: &[SharedString],
    current: &[BTreeMap<String, String>],
    entry: &schema::Entry,
) -> SettingRow {
    let main = shell.includes.docs.len();
    let dotted = entry.dotted();
    let home = entry_home(sets, &dotted);
    let mut row = setting_row(doc_at(shell, home.unwrap_or(main)), entry);
    row.is_new = shell.new_keys.contains(dotted.as_str());
    row.available = home.is_none();
    row.home = home.map_or(-1, |home| home as i32);
    if let Some(label) = home.and_then(|home| labels.get(home)) {
        row.home_label = label.clone();
    }
    row.changed = match home {
        Some(home) => {
            current.get(home).and_then(|values| values.get(&dotted))
                != shell.saved.get(home).and_then(|values| values.get(&dotted))
        }
        None => false,
    };
    row
}

/// Refresh one row after a committed edit.
pub(super) fn refresh_row(app: &AppWindow, shell: &Shell, key: &str) {
    rebuild_row(app, shell, key, false);
}

/// Find a row by key in the cards model and hand it to `update`;
/// the row is written back when `update` returns true. Returns whether
/// the row was found.
fn update_row(app: &AppWindow, key: &str, update: impl FnOnce(&mut SettingRow) -> bool) -> bool {
    let cards = app.get_cards();
    let Some(cards) = cards.as_any().downcast_ref::<VecModel<SettingsCard>>() else {
        return false;
    };
    for gi in 0..cards.row_count() {
        let Some(card) = cards.row_data(gi) else {
            continue;
        };
        let Some(rows) = card.rows.as_any().downcast_ref::<VecModel<SettingRow>>() else {
            continue;
        };
        for ri in 0..rows.row_count() {
            let Some(mut old) = rows.row_data(ri) else {
                continue;
            };
            if old.key.as_str() == key {
                if update(&mut old) {
                    rows.set_row_data(ri, old);
                }
                return true;
            }
        }
    }
    false
}

/// Rebuild a row and overwrite it even when the text is unchanged — for
/// when a sibling edit changes this row's options (resolution → refresh).
pub(super) fn rebuild_row(app: &AppWindow, shell: &Shell, key: &str, force: bool) {
    let sets = chain_path_sets(shell);
    let labels = setting_labels(shell);
    let row = if let Some(entry) = shell
        .schema
        .iter()
        .find(|entry| entry.path.join(".") == key)
    {
        let home = entry_home(&sets, key).unwrap_or(shell.includes.docs.len());
        let current: BTreeMap<String, String> =
            doc_at(shell, home).leaf_values().into_iter().collect();
        let mut row = setting_row(doc_at(shell, home), entry);
        row.home = home as i32;
        if let Some(label) = labels.get(home) {
            row.home_label = label.clone();
        }
        row.changed = current.get(key) != shell.saved.get(home).and_then(|values| values.get(key));
        row
    } else if let Some(rest) = key.strip_prefix("output.") {
        let Some((name, field_key)) = rest.rsplit_once('.') else {
            return;
        };
        let monitor = shell
            .guide_monitors
            .iter()
            .find(|monitor| monitor.name == name);
        let current: BTreeMap<String, String> = doc_at(shell, shell.includes.docs.len())
            .leaf_values()
            .into_iter()
            .collect();
        let mut row = match field_key {
            "resolution" => {
                let Some(monitor) = monitor else { return };
                resolution_choice_row(shell, name, monitor, &current)
            }
            "refresh" => {
                let Some(monitor) = monitor else { return };
                refresh_choice_row(shell, name, monitor, &current)
            }
            _ => {
                let Some(field) = outputs::FIELDS.iter().find(|field| field.key == field_key)
                else {
                    return;
                };
                output_row(shell, name, field, &current)
            }
        };
        row.home_label = labels
            .get(shell.includes.docs.len())
            .cloned()
            .unwrap_or_default();
        row
    } else {
        return;
    };

    // Same text = nothing to re-render; keeps the editor's focus. A
    // forced rebuild also swaps changed options, and a lingering drag
    // preview always clears.
    update_row(app, key, |old| {
        let write = force || old.value != row.value || !old.preview.is_empty();
        if write {
            *old = row;
        }
        write
    });
}

/// Show a dragged slider's live value in the row's text box without
/// touching the document.
pub(super) fn preview_row(app: &AppWindow, key: &str, value_text: &str) {
    // Patch the preview text only: touching `value` would re-evaluate
    // the Slider's binding mid-drag and kill the gesture (the thumb
    // fights the user at the extremes). The unchanged-text guard keeps
    // the Slider's `changed` callback from looping back through
    // set_row_data.
    update_row(app, key, |old| {
        let write = old.preview.as_str() != value_text;
        if write {
            old.preview = value_text.into();
        }
        write
    });
}

/// Text for a slider float, rounded for whole-number kinds so the write
/// path can parse it back as i64.
pub(super) fn slider_text(schema: &[schema::Entry], key: &str, value: f32) -> String {
    let is_int = schema.iter().any(|entry| {
        entry.path.join(".") == key && matches!(entry.kind, schema::Kind::Integer { .. })
    });
    if is_int {
        format!("{}", value.round() as i64)
    } else {
        format!("{value}")
    }
}

fn value_kind(kind: &schema::Kind) -> ValueKind {
    match kind {
        schema::Kind::Bool => ValueKind::Boolean,
        schema::Kind::Integer { .. } => ValueKind::Integer,
        schema::Kind::Float { .. } => ValueKind::Float,
        schema::Kind::Text => ValueKind::Text,
        schema::Kind::List => ValueKind::List,
        schema::Kind::Choice(_) => ValueKind::Choice,
        schema::Kind::Color => ValueKind::Color,
        schema::Kind::Curve => ValueKind::Curve,
    }
}

/// Slider bounds for number kinds that carry both a min and a max;
/// (0.0, 0.0) means un-ranged; no slider.
fn kind_bounds(kind: &schema::Kind) -> (f32, f32) {
    match kind {
        schema::Kind::Integer {
            min: Some(min),
            max: Some(max),
        } => (*min as f32, *max as f32),
        schema::Kind::Float {
            min: Some(min),
            max: Some(max),
        } => (*min as f32, *max as f32),
        _ => (0.0, 0.0),
    }
}

/// Commit form: turn editor input into the value text `set_leaf_text`
/// expects, validating/clamping per Kind. `Err` = user-facing rejection.
pub(super) fn commit_value(kind: Option<&schema::Kind>, raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    match kind {
        Some(schema::Kind::List) => {
            let inner = raw.trim().trim_start_matches('[').trim_end_matches(']');
            Ok(format!("[{inner}]"))
        }
        Some(schema::Kind::Bool) => Ok(raw.to_owned()),
        Some(schema::Kind::Integer { min, max }) => {
            let mut value: i64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a whole number"))?;
            if let Some(min) = min {
                value = value.max(*min);
            }
            if let Some(max) = max {
                value = value.min(*max);
            }
            Ok(value.to_string())
        }
        Some(schema::Kind::Float { min, max }) => {
            let mut value: f64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a number"))?;
            if let Some(min) = min {
                value = value.max(*min);
            }
            if let Some(max) = max {
                value = value.min(*max);
            }
            Ok(value.to_string())
        }
        // Strings in the file are quoted; Debug-format escapes the same way
        // TOML basic strings do for the ASCII values configs use.
        Some(
            schema::Kind::Text
            | schema::Kind::Choice(_)
            | schema::Kind::Color
            | schema::Kind::Curve,
        ) => Ok(format!("{raw:?}")),
        _ => Ok(raw.to_owned()),
    }
}

/// Clean, editor-ready text for a schema key, read through the typed
/// getters — never the TOML repr, which carries trailing comments.
fn typed_value(doc: &ConfigDocument, entry: &schema::Entry) -> Option<String> {
    let parts: Vec<&str> = entry.path.iter().map(String::as_str).collect();
    match &entry.kind {
        schema::Kind::Bool => doc.get_bool(&parts).map(|v| v.to_string()),
        schema::Kind::Integer { .. } => doc.get_integer(&parts).map(|v| v.to_string()),
        schema::Kind::Float { .. } => doc.get_float(&parts).map(|v| v.to_string()),
        schema::Kind::List => list_text(doc, &parts),
        schema::Kind::Text
        | schema::Kind::Choice(_)
        | schema::Kind::Color
        | schema::Kind::Curve => doc.get_string(&parts),
    }
}

/// Comma-joined list display.
fn list_text(doc: &ConfigDocument, parts: &[&str]) -> Option<String> {
    if let Some(values) = doc.get_integers(parts) {
        return Some(
            values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    if let Some(values) = doc.get_floats(parts) {
        return Some(
            values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    doc.get_strings(parts).map(|values| values.join(", "))
}

/// Hex color (#RRGGBB or #RRGGBBAA) to swatch brush; black when unparseable.
fn swatch_for(value: &str) -> slint::Brush {
    let hex = value.trim().trim_start_matches('#');
    let channel = |range: std::ops::Range<usize>| {
        hex.get(range)
            .and_then(|bits| u8::from_str_radix(bits, 16).ok())
            .unwrap_or(0)
    };
    slint::Brush::from(slint::Color::from_rgb_u8(
        channel(0..2),
        channel(2..4),
        channel(4..6),
    ))
}

/// Mined metadata shown beside the editor: range from the Kind payload,
/// then the mined unit. Empty when the key carries neither.
fn entry_hint(entry: &schema::Entry) -> String {
    let range = match &entry.kind {
        schema::Kind::Integer { min, max } => Some(range_text(
            min.map(|v| v.to_string()),
            max.map(|v| v.to_string()),
        )),
        schema::Kind::Float { min, max } => Some(range_text(
            min.map(|v| v.to_string()),
            max.map(|v| v.to_string()),
        )),
        _ => None,
    };
    let mut hints: Vec<String> = range.into_iter().filter(|r| !r.is_empty()).collect();
    if let Some(unit) = &entry.unit {
        hints.push(unit.clone());
    }
    hints.join(" • ")
}

fn range_text(min: Option<String>, max: Option<String>) -> String {
    match (min, max) {
        (Some(min), Some(max)) => format!("{min}–{max}"),
        (Some(min), None) => format!("≥ {min}"),
        (None, Some(max)) => format!("≤ {max}"),
        (None, None) => String::new(),
    }
}

/// Best-effort decor strip for non-schema values (repr may carry comments).
pub(super) fn strip_decor(raw: &str) -> String {
    let trimmed = raw.trim();
    match trimmed.split_once(" #") {
        Some((value, _)) => value.trim_end().to_owned(),
        None => trimmed.to_owned(),
    }
}

/// Register the value-editing callbacks: text edits and slider
/// releases both funnel through one write path.
pub(super) fn install_value_editing(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_set_value(move |key, value| {
            let Some(app) = weak.upgrade() else { return };
            commit_edit(&app, &shell, &key, &value);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_set_slider_value(move |key, value| {
            let Some(app) = weak.upgrade() else { return };
            let raw = slider_text(&shell.borrow().schema, &key, value);
            commit_edit(&app, &shell, &key, &raw);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_slider_preview(move |key, value| {
            let Some(app) = weak.upgrade() else { return };
            let raw = slider_text(&shell.borrow().schema, &key, value);
            preview_row(&app, &key, &raw);
        });
    }
}

/// Shared write path for text edits and slider releases: format the raw
/// input per Kind, write through set_leaf_text, then refresh the row.
fn commit_edit(app: &AppWindow, shell: &Rc<RefCell<Shell>>, key: &str, raw: &str) {
    // Rule fields ("window_rule[0].match.app_id") write straight
    // to the owning document — no schema formatting, no
    // save-popup destination.
    if let Some((family, doc, index, field_key)) = rules::parse_rule_key(key) {
        let field = rules::fields(family)
            .0
            .iter()
            .chain(rules::fields(family).1)
            .find(|field| field.key == field_key);
        let Some(field) = field else { return };
        let target = {
            let shell = shell.borrow();
            doc.unwrap_or_else(|| super::page_rules::rule_target(&shell, family))
        };
        let result = {
            let mut shell = shell.borrow_mut();
            rules::apply_field_text(doc_at_mut(&mut shell, target), family, index, field, raw)
        };
        if let Err(err) = result {
            app.set_status(err.into());
            return;
        }
        let shell = shell.borrow();
        app.set_dirty(shell.any_modified());
        super::page_rules::rebuild_rule_page(app, &shell);
        return;
    }
    let formatted = {
        let shell = shell.borrow();
        if key.starts_with("output.") {
            // Output fields aren't schema-backed; their fixed
            // vocabulary formats the input instead.
            super::page_outputs::format_output_value(&shell, key, raw)
        } else {
            let kind = shell
                .schema
                .iter()
                .find(|entry| entry.path.join(".") == key)
                .map(|entry| &entry.kind);
            commit_value(kind, raw)
        }
    };
    let value_text = match formatted {
        Ok(value_text) => value_text,
        Err(err) => {
            app.set_status(err.into());
            return;
        }
    };
    let home = {
        let shell = shell.borrow();
        let sets = chain_path_sets(&shell);
        // Writes follow ownership: an existing key is edited where
        // it lives. Where a brand-new key goes is chosen in the
        // save popup.
        entry_home(&sets, key).unwrap_or(shell.includes.docs.len())
    };
    let accepted = {
        let mut shell = shell.borrow_mut();
        let main = shell.includes.docs.len();
        let doc: &mut ConfigDocument = if home == main {
            &mut shell.doc
        } else if let Some(inc) = shell.includes.docs.get_mut(home) {
            &mut inc.doc
        } else {
            return;
        };
        doc.set_leaf_text(key, &value_text)
    };
    if !accepted {
        app.set_status(format!("umbriel would reject {key} = {value_text}").into());
        return;
    }
    let shell = shell.borrow();
    app.set_dirty(shell.any_modified());
    refresh_row(app, &shell, key);
    // Shader assignments live on their own page; refresh the
    // assignment dropdowns after a change.
    if key.starts_with("animation.") && key.ends_with(".shader") {
        super::page_shaders::rebuild_shaders(app, &shell);
    }
    // A new resolution changes which refreshes exist — rebuild
    // that dropdown too, even if its text stayed the same.
    if key.starts_with("output.") && key.ends_with(".resolution") {
        let name = &key["output.".len()..key.len() - ".resolution".len()];
        rebuild_row(app, &shell, &format!("output.{name}.refresh"), true);
    }
    app.set_changed_count(changed_count(&shell));
}
