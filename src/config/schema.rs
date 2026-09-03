//! Runtime schema assembly: derive editable entries from Umbriel's packaged
//! default config so the GUI covers new keys without an app release. Sparse
//! maintainer refinements live in [`OVERLAY`]; a missing refinement degrades
//! a key to its derived treatment, never loses it.

use toml_edit::{DocumentMut, Item};

/// Widget kind and constraints for one key.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Bool,
    Integer { min: Option<i64>, max: Option<i64> },
    Float { min: Option<f64>, max: Option<f64> },
    Text,
}

/// A typed scalar value; also used for defaults.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}

/// One editable key, as presented to the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Key path, e.g. `["appearance", "blur", "radius"]`.
    pub path: Vec<String>,
    /// Dotted section the key was found under, e.g. `"appearance.blur"`.
    pub section: String,
    pub kind: Kind,
    /// Derived default; `None` when only the key's existence is known.
    pub default: Option<Value>,
    pub label: String,
    /// Changing this key requires a compositor restart.
    pub restart: bool,
}

impl Entry {
    /// Dotted path for overlay lookups, e.g. `"appearance.blur.radius"`.
    pub fn dotted(&self) -> String {
        self.path.join(".")
    }
}

/// Maintainer refinements over derived entries, keyed by dotted path.
pub struct Overlay {
    /// Sections never offered as settings (handled by dedicated editors
    /// later, or not settings at all).
    pub skip_sections: &'static [&'static str],
    /// Label overrides where the humanized key name is not good enough.
    pub labels: &'static [(&'static str, &'static str)],
    /// Keys whose changes require a compositor restart.
    pub restart: &'static [&'static str],
}

pub const OVERLAY: Overlay = Overlay {
    skip_sections: &["include", "keybinds"],
    labels: &[],
    restart: &["general.xwayland"],
};

/// Derive entries from a packaged default config. Active keys only here;
/// commented-out keys are mined in a follow-up step.
pub fn assemble(packaged: &str) -> Vec<Entry> {
    let Ok(doc) = packaged.parse::<DocumentMut>() else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    walk_table(doc.as_table(), &mut Vec::new(), &mut entries);
    entries
}

fn walk_table(table: &toml_edit::Table, path: &mut Vec<String>, out: &mut Vec<Entry>) {
    for (key, item) in table.iter() {
        path.push(key.to_owned());
        match item {
            Item::Value(value) => {
                let section = path[..path.len() - 1].join(".");
                if let Some(entry) = entry_for(path, value, &section) {
                    out.push(entry);
                }
            }
            Item::Table(nested) => {
                walk_table(nested, path, out);
            }
            // Arrays (autostart, width_presets), tables-of-tables (window
            // rules), and array values get dedicated editors later.
            _ => {}
        }
        path.pop();
    }
}

fn entry_for(path: &[String], value: &toml_edit::Value, section: &str) -> Option<Entry> {
    if OVERLAY.skip_sections.contains(&path[0].as_str()) {
        return None;
    }
    let (kind, default) = match value {
        toml_edit::Value::Boolean(v) => (Kind::Bool, Value::Bool(*v.value())),
        toml_edit::Value::Integer(v) => {
            let raw = *v.value();
            let range = v
                .decor()
                .suffix()
                .and_then(toml_edit::RawString::as_str)
                .and_then(mine_range);
            (
                Kind::Integer {
                    min: range.map(|r| r.0 as i64),
                    max: range.map(|r| r.1 as i64),
                },
                Value::Integer(raw),
            )
        }
        toml_edit::Value::Float(v) => {
            let raw = *v.value();
            let range = v
                .decor()
                .suffix()
                .and_then(toml_edit::RawString::as_str)
                .and_then(mine_range);
            (
                Kind::Float {
                    min: range.map(|r| r.0),
                    max: range.map(|r| r.1),
                },
                Value::Float(raw),
            )
        }
        toml_edit::Value::String(v) => (Kind::Text, Value::Text(v.value().to_owned())),
        _ => return None,
    };
    let dotted = path.join(".");
    Some(Entry {
        label: overlay_label(&dotted, &path[path.len() - 1]),
        restart: OVERLAY.restart.contains(&dotted.as_str()),
        path: path.to_vec(),
        section: section.to_owned(),
        kind,
        default: Some(default),
    })
}

/// Best-effort numeric range from a value's trailing decor comment
/// (`# 0-10000`, `# -200 to 200`, `# 0.0-1.0`). Returns (min, max).
fn mine_range(decor: &str) -> Option<(f64, f64)> {
    let comment = decor.split('#').nth(1)?;
    let body = comment.trim().trim_end_matches(" ms").trim();
    let (a, b) = if let Some((min, max)) = body.split_once(" to ") {
        (min.trim().to_owned(), max.trim().to_owned())
    } else if let Some(rest) = body.strip_prefix('-') {
        let (min, max) = rest.split_once('-')?;
        (format!("-{min}"), max.trim().to_owned())
    } else {
        let (min, max) = body.split_once('-')?;
        (min.trim().to_owned(), max.trim().to_owned())
    };
    let min: f64 = a.split_whitespace().next().unwrap_or(&a).parse().ok()?;
    let max: f64 = b.split_whitespace().next().unwrap_or(&b).parse().ok()?;
    (max >= min).then_some((min, max))
}

fn overlay_label(dotted: &str, key: &str) -> String {
    for (path, label) in OVERLAY.labels {
        if *path == dotted {
            return (*label).to_owned();
        }
    }
    humanize(key)
}

/// `drag_opacity` -> `Drag opacity`.
fn humanize(key: &str) -> String {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
    .replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
[general]
xwayland = true                # requires restart
autostart = []

[appearance]
border_width = 2               # 0-100
corner_radius = 10
drag_opacity = 0.75            # 0.0-1.0
mod_hint = \"left\"

[appearance.blur]
passes = 3                     # 0-8
noise = 0.02                   # 0.0-1.0

[appearance.shadow]
offset_x = 2                   # -200 to 200

[include]
files = []

[keybinds]
\"Mod+Return\" = \"spawn:kitty\"
";

    #[test]
    fn derives_kinds_defaults_and_sections() {
        let entries = assemble(FIXTURE);
        let get = |dotted: &str| entries.iter().find(|e| e.dotted() == dotted).unwrap();

        assert_eq!(get("general.xwayland").kind, Kind::Bool);
        assert_eq!(get("general.xwayland").default, Some(Value::Bool(true)));
        assert_eq!(get("general.xwayland").section, "general");
        assert!(get("general.xwayland").restart);

        assert_eq!(
            get("appearance.border_width").kind,
            Kind::Integer {
                min: Some(0),
                max: Some(100)
            }
        );
        assert_eq!(
            get("appearance.drag_opacity").kind,
            Kind::Float {
                min: Some(0.0),
                max: Some(1.0)
            }
        );
        assert_eq!(get("appearance.mod_hint").kind, Kind::Text);
        assert_eq!(get("appearance.blur.passes").section, "appearance.blur");
        assert_eq!(
            get("appearance.shadow.offset_x").kind,
            Kind::Integer {
                min: Some(-200),
                max: Some(200)
            }
        );
        assert_eq!(get("appearance.border_width").label, "Border width");
    }

    #[test]
    fn skips_non_settings_sections_and_collections() {
        let entries = assemble(FIXTURE);
        assert!(entries.iter().all(|e| e.dotted() != "include.files"));
        assert!(entries.iter().all(|e| e.path[0] != "keybinds"));
        assert!(entries.iter().all(|e| e.dotted() != "general.autostart"));
        assert_eq!(entries.len(), 8);
    }

    #[test]
    fn missing_ranges_are_none_but_kept() {
        let entries = assemble(FIXTURE);
        let radius = entries
            .iter()
            .find(|e| e.dotted() == "appearance.corner_radius")
            .unwrap();
        assert_eq!(
            radius.kind,
            Kind::Integer {
                min: None,
                max: None
            }
        );
    }

    #[test]
    fn broken_input_yields_no_entries() {
        assert!(assemble("not [valid").is_empty());
    }
}
