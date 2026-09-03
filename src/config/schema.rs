//! Runtime schema assembly: derive editable entries from Umbriel's packaged
//! default config so the GUI covers new keys without an app release. Sparse
//! maintainer refinements live in [`OVERLAY`]; a missing refinement degrades
//! a key to its derived treatment, never loses it.
//! Commented-out keys are mined from the raw text; their values are the
//! presumed defaults (the compositor's true fallbacks live in its code).

use std::collections::BTreeSet;
use toml_edit::{DocumentMut, Item};

/// Widget kind and constraints for one key.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Bool,
    Integer {
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        min: Option<f64>,
        max: Option<f64>,
    },
    Text,
    /// Array of scalars, edited as a comma-separated string in the element type.
    List,
    /// Fixed vocabulary mined from the value's comment, e.g.
    /// `# popin, zoom, slide, fade, none`.
    Choice(Vec<String>),
    /// `#RRGGBB` or `#RRGGBBAA` color string.
    Color,
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

/// Add entries for commented-out keys (`# key = value`), including keys
/// under fully commented sections (`# [environment]`). Active keys always
/// win; a commented duplicate is skipped.
fn mine_comments(packaged: &str, entries: &mut Vec<Entry>) {
    let known: std::collections::HashSet<String> = entries.iter().map(Entry::dotted).collect();
    let mut section = String::new();
    for line in packaged.lines() {
        let trimmed = line.trim();
        let Some(body) = trimmed.strip_prefix('#') else {
            if let Some(header) = parse_header(trimmed) {
                section = header;
            }
            continue;
        };
        let body = body.trim();
        if let Some(header) = parse_header(body) {
            section = header;
            continue;
        }
        let Some((key, raw)) = body.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !is_bare_key(key) || section.is_empty() || is_skipped(&section) {
            continue;
        }
        let dotted = format!("{section}.{key}");
        if known.contains(&dotted) {
            continue;
        }
        if let Some((kind, default)) = classify_value(raw.trim()) {
            let mut path: Vec<String> = section.split('.').map(str::to_owned).collect();
            path.push(key.to_owned());
            entries.push(Entry {
                label: overlay_label(&dotted, key),
                restart: OVERLAY.restart.contains(&dotted.as_str()),
                path,
                section: section.clone(),
                kind,
                default: Some(default),
            });
        }
    }
}

/// `[section.nested]` with bare, dotted names only; quoted or array headers
/// (outputs, `[[window_rule]]`) are left to their dedicated editors.
fn parse_header(line: &str) -> Option<String> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    (!inner.is_empty()
        && inner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.'))
    .then(|| inner.to_owned())
}

fn is_bare_key(key: &str) -> bool {
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_skipped(section: &str) -> bool {
    let top = section.split('.').next().unwrap_or_default();
    OVERLAY.skip_sections.contains(&top)
}

/// Classify a raw assignment value: bool, integer, float, or quoted string
/// (a trailing comment after the closing quote is ignored).
fn classify_value(raw: &str) -> Option<(Kind, Value)> {
    if let Ok(value) = raw.parse::<bool>() {
        return Some((Kind::Bool, Value::Bool(value)));
    }
    if let Ok(value) = raw.parse::<i64>() {
        return Some((
            Kind::Integer {
                min: None,
                max: None,
            },
            Value::Integer(value),
        ));
    }
    if let Ok(value) = raw.parse::<f64>() {
        return Some((
            Kind::Float {
                min: None,
                max: None,
            },
            Value::Float(value),
        ));
    }
    let (value, trailing) = quoted_text(raw, '"').or_else(|| quoted_text(raw, '\''))?;
    let kind = if is_color(value) {
        Kind::Color
    } else {
        mine_choices(trailing).map_or(Kind::Text, Kind::Choice)
    };
    Some((kind, Value::Text(value.to_owned())))
}

/// Derive entries from a packaged default config. Active keys only here;
/// commented-out keys are mined in a follow-up step.
pub fn assemble(packaged: &str) -> Vec<Entry> {
    let Ok(doc) = packaged.parse::<DocumentMut>() else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    walk_table(doc.as_table(), &mut Vec::new(), &mut entries);
    mine_comments(packaged, &mut entries);
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
            // Tables-of-tables (window rules) get a dedicated editor later.
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
        toml_edit::Value::Boolean(v) => (Kind::Bool, Some(Value::Bool(*v.value()))),
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
                Some(Value::Integer(raw)),
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
                Some(Value::Float(raw)),
            )
        }
        toml_edit::Value::String(v) => {
            let text = v.value();
            let suffix = v.decor().suffix().and_then(toml_edit::RawString::as_str);
            let kind = if is_color(text) {
                Kind::Color
            } else {
                suffix
                    .and_then(mine_choices)
                    .map_or(Kind::Text, Kind::Choice)
            };
            (kind, Some(Value::Text(text.to_owned())))
        }
        toml_edit::Value::Array(_) => (Kind::List, None),
        _ => return None,
    };
    let dotted = path.join(".");
    Some(Entry {
        label: overlay_label(&dotted, &path[path.len() - 1]),
        restart: OVERLAY.restart.contains(&dotted.as_str()),
        path: path.to_vec(),
        section: section.to_owned(),
        kind,
        default,
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

/// Best-effort vocabulary from a value's trailing comment: comma-separated
/// words (`# popin, zoom, fade`) or ` or `-separated words
/// (`# "scrolling" or "dwindle"`). Any piece that is not a single word
/// rejects the whole comment, so prose never becomes a dropdown.
fn mine_choices(comment: &str) -> Option<Vec<String>> {
    let body = comment.split('#').nth(1)?.trim();
    let separator = if body.contains(" or ") { " or " } else { "," };
    let words: Vec<String> = body
        .split(separator)
        .map(|word| word.trim().trim_matches('"').to_owned())
        .collect();
    (words.len() >= 2 && words.iter().all(|word| is_word_like(word))).then_some(words)
}

fn is_word_like(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `#RRGGBB` or `#RRGGBBAA`.
fn is_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    (hex.len() == 6 || hex.len() == 8) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// A quoted string and whatever follows the closing quote (the trailing
/// comment for commented-out keys).
fn quoted_text(raw: &str, quote: char) -> Option<(&str, &str)> {
    let rest = raw.strip_prefix(quote)?;
    let end = rest.find(quote)?;
    Some((&rest[..end], &rest[end + 1..]))
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
pub fn humanize(key: &str) -> String {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
    .replace('_', " ")
}

/// Case-insensitive filter for the settings search: the label or the dotted
/// path must contain every whitespace-separated term, in any order.
pub fn matches(entry: &Entry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    let dotted = entry.dotted().to_lowercase();
    let label = entry.label.to_lowercase();
    query
        .split_whitespace()
        .all(|term| dotted.contains(term) || label.contains(term))
}

/// Differences between two schema key sets, by dotted path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

impl SchemaDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    /// Human one-liner, e.g. `3 new: a, b, c; 1 removed: old`.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.added.is_empty() {
            parts.push(format!(
                "{} new: {}",
                self.added.len(),
                name_list(&self.added)
            ));
        }
        if !self.removed.is_empty() {
            parts.push(format!(
                "{} removed: {}",
                self.removed.len(),
                name_list(&self.removed)
            ));
        }
        parts.join("; ")
    }
}

fn name_list(names: &[String]) -> String {
    if names.len() > 3 {
        format!("{}, …", names[..3].join(", "))
    } else {
        names.join(", ")
    }
}

/// Dotted paths of every entry, sorted.
pub fn key_set(entries: &[Entry]) -> BTreeSet<String> {
    entries.iter().map(Entry::dotted).collect()
}

/// Keys present in `new` but not `old`, and vice versa.
pub fn diff(old: &BTreeSet<String>, new: &BTreeSet<String>) -> SchemaDiff {
    SchemaDiff {
        added: new.difference(old).cloned().collect(),
        removed: old.difference(new).cloned().collect(),
    }
}

/// Config families owned by dedicated surfaces — the Outputs page, or
/// editors on the roadmap — rather than schema pages.
const MANAGED_SECTIONS: &[&str] = &["include", "keybinds", "output", "window_rule", "layer_rule"];

/// Document keys no surface claims: not in the assembled schema and not in
/// a managed family. These belong on the Raw page so nothing in a user's
/// config is ever silently invisible.
pub fn uncovered(paths: &[String], schema_keys: &BTreeSet<String>) -> Vec<String> {
    paths
        .iter()
        .filter(|path| {
            let top = path.split('.').next().unwrap_or_default();
            !MANAGED_SECTIONS.contains(&top) && !schema_keys.contains(*path)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r##"[general]
xwayland = true                # requires restart
autostart = []

[appearance]
border_width = 2               # 0-100
corner_radius = 10
drag_opacity = 0.75            # 0.0-1.0
mod_hint = "left"

[appearance.blur]
passes = 3                     # 0-8
noise = 0.02                   # 0.0-1.0

[appearance.shadow]
offset_x = 2                   # -200 to 200

[layout]
mode = "dwindle"               # "scrolling" or "dwindle"

[colors]
background = "#141419FF"
# accent = "#7AA3FFFF"

[include]
files = []

[keybinds]
"Mod+Return" = "spawn:kitty"
"##;

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
        assert_eq!(entries.len(), 12);
    }

    #[test]
    fn arrays_become_list_entries() {
        let entries = assemble(FIXTURE);
        let autostart = entries
            .iter()
            .find(|e| e.dotted() == "general.autostart")
            .expect("array entry");
        assert_eq!(autostart.kind, Kind::List);
        assert_eq!(autostart.default, None);
        assert_eq!(autostart.label, "Autostart");
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

    const COMMENTED: &str = "\
[colors]
# background = \"#141419FF\"
text_primary = \"#E8E8EAFF\"
# backdrop = \"#000000FF\"       # fullscreen gaps

# [environment]
# GTK_THEME = \"Adwaita:dark\"

# [output.\"Some Make ABC123\"]
# scale = 1.25

# prose only, nothing to mine

[general]
focus_on_activate = false
# focus_on_activate = true
";

    #[test]
    fn mines_commented_keys_under_active_and_commented_sections() {
        let entries = assemble(COMMENTED);
        let get = |dotted: &str| entries.iter().find(|e| e.dotted() == dotted);

        let background = get("colors.background").expect("mined");
        assert_eq!(background.kind, Kind::Color);
        assert_eq!(background.default, Some(Value::Text("#141419FF".into())));
        assert_eq!(background.section, "colors");
        assert_eq!(background.label, "Background");

        assert!(get("colors.backdrop").is_some());
        assert!(get("environment.GTK_THEME").is_some());
        assert_eq!(
            get("environment.GTK_THEME").unwrap().default,
            Some(Value::Text("Adwaita:dark".into()))
        );
    }

    #[test]
    fn active_keys_win_over_commented_duplicates() {
        let entries = assemble(COMMENTED);
        let found: Vec<_> = entries
            .iter()
            .filter(|e| e.dotted() == "general.focus_on_activate")
            .collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].default, Some(Value::Bool(false)));
    }

    #[test]
    fn quoted_and_complex_headers_are_ignored() {
        let entries = assemble(COMMENTED);
        assert!(entries.iter().all(|e| e.path[0] != "output"));
    }

    #[test]
    fn diff_reports_additions_and_removals() {
        let old = BTreeSet::from(["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        let new = BTreeSet::from(["b".to_owned(), "c".to_owned(), "d".to_owned()]);
        let drift = diff(&old, &new);
        assert_eq!(drift.added, vec!["d".to_owned()]);
        assert_eq!(drift.removed, vec!["a".to_owned()]);
        assert!(!drift.is_empty());
    }

    #[test]
    fn diff_of_equal_sets_is_empty() {
        let set = BTreeSet::from(["a".to_owned()]);
        assert!(diff(&set, &set).is_empty());
    }

    #[test]
    fn uncovered_lists_only_unclaimed_keys() {
        let keys = key_set(&assemble(FIXTURE));
        let paths = vec![
            "general.xwayland".to_owned(),
            "environment.PROTON_ENABLE_WAYLAND".to_owned(),
            "events.lid_close".to_owned(),
            "keybinds.Mod+Return".to_owned(),
            "output.DP-3.scale".to_owned(),
            "window_rule".to_owned(),
        ];
        assert_eq!(
            uncovered(&paths, &keys),
            vec![
                "environment.PROTON_ENABLE_WAYLAND".to_owned(),
                "events.lid_close".to_owned(),
            ]
        );
    }

    #[test]
    fn summary_caps_name_lists() {
        let drift = SchemaDiff {
            added: (1..=5).map(|i| format!("k{i}")).collect(),
            removed: vec!["old".to_owned()],
        };
        assert_eq!(drift.summary(), "5 new: k1, k2, k3, …; 1 removed: old");
    }

    #[test]
    fn key_set_collects_dotted_paths() {
        let entries = vec![Entry {
            path: vec!["general".to_owned(), "xwayland".to_owned()],
            section: "general".to_owned(),
            kind: Kind::Bool,
            default: Some(Value::Bool(true)),
            label: "Xwayland".to_owned(),
            restart: false,
        }];
        assert_eq!(
            key_set(&entries),
            BTreeSet::from(["general.xwayland".to_owned()])
        );
    }

    #[test]
    fn matches_by_path_label_and_terms() {
        let entries = assemble(FIXTURE);
        let find = |dotted: &str| entries.iter().find(|e| e.dotted() == dotted).unwrap();

        let xwayland = find("general.xwayland");
        assert!(matches(xwayland, "xwayland"));
        assert!(matches(xwayland, "XWayland"));
        assert!(matches(xwayland, "   "));

        let radius = find("appearance.corner_radius");
        assert!(matches(radius, "corner radius"));
        assert!(matches(radius, "radius corner"));
        assert!(!matches(radius, "corner border"));
    }

    #[test]
    fn mines_choices_and_colors() {
        let entries = assemble(FIXTURE);
        let find = |dotted: &str| entries.iter().find(|e| e.dotted() == dotted).unwrap();

        assert_eq!(
            find("layout.mode").kind,
            Kind::Choice(vec!["scrolling".to_owned(), "dwindle".to_owned()])
        );
        assert_eq!(find("colors.background").kind, Kind::Color);
        assert_eq!(find("colors.accent").kind, Kind::Color);
        assert_eq!(find("appearance.mod_hint").kind, Kind::Text);
    }

    #[test]
    fn prose_comments_never_become_choices() {
        let entries = assemble("[notes]\n# x = \"a\" # see this or that guide, please\n");
        let found = entries.iter().find(|e| e.dotted() == "notes.x").unwrap();
        assert_eq!(found.kind, Kind::Text);
        assert_eq!(found.default, Some(Value::Text("a".into())));
    }
}
