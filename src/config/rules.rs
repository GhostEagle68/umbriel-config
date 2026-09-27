//! Window- and layer-rule field vocabulary, mirroring umbriel's parser
//! (`src/config/config.cpp`). Rules are a fixed field set under dynamic
//! `[[window_rule]]` / `[[layer_rule]]` instances, like outputs inverted.

use super::document::ConfigDocument;
use super::umbriel_schema;

/// One configurable field of a rule; `key` is dotted within the rule table
/// (`"match.app_id"`, `"default_floating"`).
#[derive(Clone)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
}

fn field(key: &str, label: &str, kind: FieldKind) -> Field {
    Field {
        key: key.to_owned(),
        label: label.to_owned(),
        kind,
    }
}

fn choice(values: &[&str]) -> FieldKind {
    FieldKind::Choice(values.iter().map(|value| (*value).to_owned()).collect())
}

#[derive(Clone)]
pub enum FieldKind {
    /// Free text; match fields hold regular expressions.
    Text,
    Toggle,
    Choice(Vec<String>),
    /// Like `Choice`, but the vocabulary is the user's configured/detected
    /// output names — dynamic, so it can't be a `&'static` list — and
    /// anything else is still accepted (an output not currently detected
    /// is a legitimate value to pre-configure).
    OutputChoice,
    Float {
        min: f64,
        max: f64,
    },
    Integer {
        min: i64,
        max: i64,
    },
    /// Inline `{ width, height }` positive integers (pixels).
    SizePx,
    /// Inline `{ width, height }` fractions, each 0.1-1.0.
    SizeFraction,
    /// Inline `{ x, y, anchor }`; anchors: top_left, top_right, bottom_left,
    /// bottom_right, top, bottom, left, right, center.
    Position,
    /// Array of strings, edited comma-separated.
    List,
    /// `#RRGGBB` or `#RRGGBBAA`.
    Color,
    /// Anything else: TOML text as typed (`3`, `"web"`, `{ x = 1 }`),
    /// else a string.
    Raw,
}

pub const ANCHORS: &[&str] = &[
    "top_left",
    "top_right",
    "bottom_left",
    "bottom_right",
    "top",
    "bottom",
    "left",
    "right",
    "center",
];

/// What a window rule can match on.
fn window_match() -> Vec<Field> {
    vec![
        field("match.app_id", "App id (regex)", FieldKind::Text),
        field("match.title", "Title (regex)", FieldKind::Text),
        field("match.xdg_tag", "Xdg tag (regex)", FieldKind::Text),
        field(
            "match.content_type",
            "Content type",
            choice(&["none", "photo", "video", "game"]),
        ),
        field("match.is_focused", "Is focused", FieldKind::Toggle),
        field("match.at_startup", "At startup", FieldKind::Toggle),
    ]
}

/// What a window rule can configure.
fn window_settings() -> Vec<Field> {
    vec![
        field("default_floating", "Floating", FieldKind::Toggle),
        field("default_fullscreen", "Fullscreen", FieldKind::Toggle),
        field("default_maximize", "Maximize", FieldKind::Toggle),
        field(
            "default_maximize_to_edges",
            "Maximize to edges",
            FieldKind::Toggle,
        ),
        field("default_focused", "Focused", FieldKind::Toggle),
        field("default_pinned", "Pinned", FieldKind::Toggle),
        field("focus_on_activate", "Focus on activate", FieldKind::Toggle),
        field("tearing", "Tearing", FieldKind::Toggle),
        field("blur", "Blur", FieldKind::Toggle),
        field("blur_popups", "Blur popups", FieldKind::Toggle),
        field("blur_optimized", "Blur optimized", FieldKind::Toggle),
        field(
            "opacity",
            "Opacity",
            FieldKind::Float { min: 0.0, max: 1.0 },
        ),
        field(
            "blur_ignore_alpha",
            "Blur ignore alpha",
            FieldKind::Float { min: 0.0, max: 1.0 },
        ),
        field("vrr", "VRR", choice(&["disabled", "always", "fullscreen"])),
        field("hdr", "HDR", choice(&["off", "on", "auto", "fullscreen"])),
        field("default_output", "Output", FieldKind::OutputChoice),
        field(
            "default_floating_size_px",
            "Floating size (px)",
            FieldKind::SizePx,
        ),
        field(
            "default_floating_size",
            "Floating size (fraction)",
            FieldKind::SizeFraction,
        ),
        field(
            "default_scrolling_extent_px",
            "Scrolling extent (px)",
            FieldKind::Integer {
                min: 1,
                max: 100_000,
            },
        ),
        field(
            "default_scrolling_extent",
            "Scrolling extent (fraction)",
            FieldKind::Float { min: 0.1, max: 1.0 },
        ),
        field("default_position", "Position", FieldKind::Position),
        field(
            "default_workspace",
            "Workspace",
            FieldKind::Integer { min: 1, max: 64 },
        ),
        field(
            "default_scrolling_column",
            "Scrolling column",
            FieldKind::Text,
        ),
        field(
            "default_scrolling_column_order",
            "Scrolling column order",
            FieldKind::Integer {
                min: i64::MIN,
                max: i64::MAX,
            },
        ),
    ]
}

/// What a layer rule can match on.
fn layer_match() -> Vec<Field> {
    vec![field(
        "match.namespace",
        "Namespace (regex)",
        FieldKind::Text,
    )]
}

/// What a layer rule can configure.
fn layer_settings() -> Vec<Field> {
    vec![
        field("blur", "Blur", FieldKind::Toggle),
        field("blur_popups", "Blur popups", FieldKind::Toggle),
        field("blur_optimized", "Blur optimized", FieldKind::Toggle),
        field(
            "blur_ignore_alpha",
            "Blur ignore alpha",
            FieldKind::Float { min: 0.0, max: 1.0 },
        ),
    ]
}

/// What a security-context rule can match on (sandboxed apps).
fn security_match() -> Vec<Field> {
    vec![
        field(
            "match.sandbox_engine",
            "Sandbox engine (regex)",
            FieldKind::Text,
        ),
        field("match.app_id", "App id (regex)", FieldKind::Text),
    ]
}

/// What a security-context rule can configure: the wayland globals a
/// sandboxed client may bind. umbriel rejects an empty list, so the UI
/// unsets the key instead of writing one.
fn security_settings() -> Vec<Field> {
    vec![field("allow_globals", "Allow globals", FieldKind::List)]
}

/// The rule families the UI edits, by TOML section name.
pub const FAMILIES: &[&str] = &["window_rule", "layer_rule", "security_context_rule"];

/// One rule family's fields: what a rule matches on, what it sets.
pub struct Family {
    pub name: &'static str,
    pub matches: Vec<Field>,
    pub settings: Vec<Field>,
}

impl Family {
    pub fn field(&self, key: &str) -> Option<&Field> {
        self.matches
            .iter()
            .chain(&self.settings)
            .find(|field| field.key == key)
    }
}

/// The families from umbriel's schema when there is one, else the
/// built-in lists.
pub fn families(schema: Option<&[umbriel_schema::Key]>) -> Vec<Family> {
    FAMILIES
        .iter()
        .map(|name| match schema {
            Some(keys) => from_umbriel(keys, name),
            None => builtin(name),
        })
        .collect()
}

/// The hand-written lists: the fallback for an umbriel without
/// `umbriel schema`, and the labels and editors the schema can't name.
fn builtin(name: &'static str) -> Family {
    let (matches, settings) = match name {
        "window_rule" => (window_match(), window_settings()),
        "layer_rule" => (layer_match(), layer_settings()),
        _ => (security_match(), security_settings()),
    };
    Family {
        name,
        matches,
        settings,
    }
}

/// A family from umbriel's schema (`window_rule[].opacity`, …): every
/// key the parser reads, typed by the schema. The built-in list still
/// supplies labels, its order, and the editors for shapes the schema
/// describes as tables (position, sizes) or plain strings (output names).
fn from_umbriel(keys: &[umbriel_schema::Key], name: &'static str) -> Family {
    let known = builtin(name);
    let prefix = format!("{name}[].");
    let mut tables: Vec<String> = Vec::new();
    let mut fields: Vec<Field> = Vec::new();
    for key in keys {
        let Some(path) = key.path.strip_prefix(&prefix) else {
            continue;
        };
        // The keys of a table field are edited with it.
        if path == "match"
            || tables
                .iter()
                .any(|table| path.starts_with(&format!("{table}.")))
        {
            continue;
        }
        let builtin = known.field(path);
        let kind = match (key.kind.as_str(), builtin) {
            ("table", Some(builtin)) => builtin.kind.clone(),
            (
                "string",
                Some(Field {
                    kind: FieldKind::OutputChoice,
                    ..
                }),
            ) => FieldKind::OutputChoice,
            ("bool", _) => FieldKind::Toggle,
            // Numbers get a slider only with both bounds.
            ("int", _) if key.min.is_some() && key.max.is_some() => FieldKind::Integer {
                min: key.min.unwrap_or_default() as i64,
                max: key.max.unwrap_or_default() as i64,
            },
            ("float", _) if key.min.is_some() && key.max.is_some() => FieldKind::Float {
                min: key.min.unwrap_or_default(),
                max: key.max.unwrap_or_default(),
            },
            ("color", _) => FieldKind::Color,
            ("enum", _) => FieldKind::Choice(key.values.clone()),
            ("string", _) => FieldKind::Text,
            ("string_array", _) => FieldKind::List,
            _ => FieldKind::Raw,
        };
        if key.kind == "table" {
            tables.push(path.to_owned());
        }
        let label = builtin.map_or_else(
            || {
                let short = path.strip_prefix("match.").unwrap_or(path);
                let label =
                    super::schema::humanize(short.strip_prefix("default_").unwrap_or(short));
                match key.format.as_deref() {
                    Some("regex") => format!("{label} (regex)"),
                    _ => label,
                }
            },
            |builtin| builtin.label.clone(),
        );
        fields.push(Field {
            key: path.to_owned(),
            label,
            kind,
        });
    }
    // The built-in order first (it groups related fields), new keys after.
    let rank = |field: &Field| {
        known
            .matches
            .iter()
            .chain(&known.settings)
            .position(|builtin| builtin.key == field.key)
            .unwrap_or(usize::MAX)
    };
    fields.sort_by_key(rank);
    let (matches, settings) = fields
        .into_iter()
        .partition(|field| field.key.starts_with("match."));
    Family {
        name,
        matches,
        settings,
    }
}

/// The key form the UI passes through its value callbacks. Rules can
/// live in several files of the include chain, so the key carries the
/// chain index of the document it edits: `window_rule[1:2].match.app_id`
/// is rule 2 of family `window_rule` in chain document 1.
pub fn rule_key(family: &str, doc: usize, index: usize, key: &str) -> String {
    format!("{family}[{doc}:{index}].{key}")
}

/// Split a `family[doc:index].key` rule key back into its parts. A bare
/// `family[index].key` (no doc) parses with `None` for the document.
pub fn parse_rule_key(key: &str) -> Option<(&str, Option<usize>, usize, &str)> {
    let (family, rest) = key.split_once('[')?;
    let (head, rest) = rest.split_once(']')?;
    let field = rest.strip_prefix('.')?;
    let (doc, index): (Option<usize>, usize) = match head.split_once(':') {
        Some((doc, index)) => {
            let doc: usize = doc.parse().ok()?;
            let index: usize = index.parse().ok()?;
            (Some(doc), index)
        }
        None => (None, head.parse().ok()?),
    };
    Some((family, doc, index, field))
}

/// Editor text for one field; `""` means unset.
pub fn field_text(doc: &ConfigDocument, family: &str, index: usize, field: &Field) -> String {
    match &field.kind {
        FieldKind::Text | FieldKind::OutputChoice => doc
            .rule_string(family, index, &field.key)
            .unwrap_or_default(),
        FieldKind::Toggle => doc
            .rule_bool(family, index, &field.key)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        FieldKind::Choice(_) => doc
            .rule_string(family, index, &field.key)
            .unwrap_or_default(),
        FieldKind::Float { .. } => doc
            .rule_float(family, index, &field.key)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        FieldKind::Integer { .. } => doc
            .rule_integer(family, index, &field.key)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        FieldKind::SizePx => match doc.rule_size_px(family, index, &field.key) {
            Some((width, height)) => format!("{width}x{height}"),
            None => String::new(),
        },
        FieldKind::SizeFraction => match doc.rule_size_fraction(family, index, &field.key) {
            Some((width, height)) => format!("{width}x{height}"),
            None => String::new(),
        },
        FieldKind::Position => match doc.rule_position(family, index, &field.key) {
            Some((x, y, Some(anchor))) => format!("{x}, {y}, {anchor}"),
            Some((x, y, None)) => format!("{x}, {y}"),
            None => String::new(),
        },
        FieldKind::List => doc
            .rule_strings(family, index, &field.key)
            .map(|values| values.join(", "))
            .unwrap_or_default(),
        FieldKind::Color => doc
            .rule_string(family, index, &field.key)
            .unwrap_or_default(),
        FieldKind::Raw => doc.rule_raw(family, index, &field.key).unwrap_or_default(),
    }
}

/// Commit editor text for one field. Empty text — or "(unset)" for a
/// choice — removes the key; anything else parses per the field's kind
/// and writes through the `rule_set_*` primitives.
pub fn apply_field_text(
    doc: &mut ConfigDocument,
    family: &str,
    index: usize,
    field: &Field,
    raw: &str,
) -> Result<(), String> {
    let raw = raw.trim();
    if raw.is_empty() {
        doc.rule_unset(family, index, &field.key);
        return Ok(());
    }
    match &field.kind {
        FieldKind::Text | FieldKind::OutputChoice => {
            doc.rule_set_string(family, index, &field.key, raw);
        }
        FieldKind::Toggle => match raw {
            "true" | "false" => doc.rule_set_bool(family, index, &field.key, raw == "true"),
            _ => return Err(format!("'{raw}' is not true or false")),
        },
        FieldKind::Choice(options) => {
            let Some(choice) = options.iter().find(|option| option.as_str() == raw) else {
                return Err(format!("'{raw}' is not one of the accepted values"));
            };
            doc.rule_set_string(family, index, &field.key, choice);
        }
        FieldKind::Float { min, max } => {
            let value: f64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a number"))?;
            doc.rule_set_float(family, index, &field.key, value.clamp(*min, *max));
        }
        FieldKind::Integer { min, max } => {
            let value: i64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a whole number"))?;
            doc.rule_set_integer(family, index, &field.key, value.clamp(*min, *max));
        }
        FieldKind::SizePx => {
            let text = raw.replace(['x', 'X'], ",");
            let mut parts = text.split(',');
            let width = parts
                .next()
                .unwrap_or_default()
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("'{raw}' is not a size like 1920x1080"))?;
            let height = parts
                .next()
                .unwrap_or_default()
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("'{raw}' is not a size like 1920x1080"))?;
            if parts.next().is_some() || width < 1 || height < 1 {
                return Err(format!("'{raw}' is not a size like 1920x1080"));
            }
            doc.rule_set_size_px(
                family,
                index,
                &field.key,
                width.clamp(1, 100_000),
                height.clamp(1, 100_000),
            );
        }
        FieldKind::SizeFraction => {
            let text = raw.replace(['x', 'X'], ",");
            let mut parts = text.split(',');
            let width = parts
                .next()
                .unwrap_or_default()
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("'{raw}' is not a size like 0.5x0.6"))?;
            let height = parts
                .next()
                .unwrap_or_default()
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("'{raw}' is not a size like 0.5x0.6"))?;
            if parts.next().is_some() {
                return Err(format!("'{raw}' is not a size like 0.5x0.6"));
            }
            doc.rule_set_size_fraction(
                family,
                index,
                &field.key,
                width.clamp(0.1, 1.0),
                height.clamp(0.1, 1.0),
            );
        }
        FieldKind::Position => {
            let mut parts = raw.split(',');
            let x = parts
                .next()
                .unwrap_or_default()
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("'{raw}' is not an x, y position"))?;
            let y = parts
                .next()
                .unwrap_or_default()
                .trim()
                .parse::<i64>()
                .map_err(|_| format!("'{raw}' is not an x, y position"))?;
            let anchor = match parts.next().map(str::trim) {
                None | Some("") => None,
                Some(anchor) => {
                    if !ANCHORS.contains(&anchor) {
                        return Err(format!(
                            "'{anchor}' is not an anchor (top_left, bottom_right, center, …)"
                        ));
                    }
                    Some(anchor)
                }
            };
            if parts.next().is_some() {
                return Err(format!("'{raw}' is not an x, y position"));
            }
            doc.rule_set_position(family, index, &field.key, x, y, anchor);
        }
        FieldKind::List => {
            let values: Vec<String> = raw
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect();
            if values.is_empty() {
                doc.rule_unset(family, index, &field.key);
                return Ok(());
            }
            doc.rule_set_strings(family, index, &field.key, &values);
        }
        FieldKind::Color => {
            if !super::schema::is_color(raw) {
                return Err(format!("'{raw}' is not a hex color (#RRGGBB or #RRGGBBAA)"));
            }
            doc.rule_set_string(family, index, &field.key, raw);
        }
        FieldKind::Raw => doc.rule_set_raw(family, index, &field.key, raw),
    }
    Ok(())
}

/// Card title for a rule: the first match value, or a fallback.
pub fn rule_title(
    doc: &super::document::ConfigDocument,
    name: &str,
    index: usize,
    match_fields: &[Field],
) -> String {
    for field in match_fields {
        if let Some(value) = doc.rule_string(name, index, &field.key)
            && !value.is_empty()
        {
            let leaf = field.key.rsplit('.').next().unwrap_or(&field.key);
            return format!("{leaf} = {value}");
        }
    }
    format!("Rule {}", index + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn unique(fields: &[Field]) -> bool {
        let mut keys: Vec<_> = fields.iter().map(|field| field.key.as_str()).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        keys.len() == total
    }

    #[test]
    fn field_keys_are_unique_per_list() {
        let schema = umbriel_schema::parse(umbriel_schema::FIXTURE)
            .unwrap()
            .options;
        for family in families(None).iter().chain(&families(Some(&schema))) {
            assert!(unique(&family.matches), "{}", family.name);
            assert!(unique(&family.settings), "{}", family.name);
        }
    }

    #[test]
    fn bool_and_choice_fields_use_their_kinds() {
        assert!(matches!(
            field("window_rule", "default_floating").kind,
            FieldKind::Toggle
        ));
        assert!(matches!(
            field("window_rule", "match.content_type").kind,
            FieldKind::Choice(_)
        ));
    }

    /// A built-in field (the no-schema fallback).
    fn field(family: &'static str, key: &str) -> Field {
        builtin(family).field(key).unwrap().clone()
    }

    /// A field as umbriel's schema describes it.
    fn schema_field(family: &'static str, key: &str) -> Field {
        let schema = umbriel_schema::parse(umbriel_schema::FIXTURE)
            .unwrap()
            .options;
        from_umbriel(&schema, family).field(key).unwrap().clone()
    }

    #[test]
    fn schema_families_cover_what_umbriel_reads() {
        let kind = |key| schema_field("window_rule", key).kind;
        assert!(matches!(kind("border_color_outer"), FieldKind::Color));
        assert!(matches!(
            kind("outer_border_width"),
            FieldKind::Integer { min: 0, max: 100 }
        ));
        assert!(matches!(kind("shadow"), FieldKind::Toggle));
        assert!(matches!(kind("match.is_scratchpad"), FieldKind::Toggle));
        assert!(matches!(kind("default_scratchpad"), FieldKind::Text));
        // A number or a workspace name.
        assert!(matches!(kind("default_workspace"), FieldKind::Raw));
        // Shapes and vocabularies the schema calls tables and strings.
        assert!(matches!(kind("default_position"), FieldKind::Position));
        assert!(matches!(
            kind("default_floating_size_px"),
            FieldKind::SizePx
        ));
        assert!(matches!(
            kind("default_floating_size"),
            FieldKind::SizeFraction
        ));
        assert!(matches!(kind("default_output"), FieldKind::OutputChoice));
        match kind("match.content_type") {
            FieldKind::Choice(values) => assert!(values.contains(&"game".to_owned())),
            _ => panic!("content_type is a choice"),
        }
        // A table field's own keys are edited with it, not listed.
        let family = from_umbriel(
            &umbriel_schema::parse(umbriel_schema::FIXTURE)
                .unwrap()
                .options,
            "window_rule",
        );
        assert!(family.field("default_position.x").is_none());
        assert!(
            family
                .matches
                .iter()
                .all(|field| field.key.starts_with("match."))
        );
        // Known fields keep their curated labels; new ones are humanized.
        assert_eq!(
            schema_field("window_rule", "default_floating").label,
            "Floating"
        );
        assert_eq!(
            schema_field("window_rule", "outer_border_width").label,
            "Outer border width"
        );
        assert_eq!(
            schema_field("layer_rule", "match.namespace").label,
            "Namespace (regex)"
        );
        assert!(matches!(
            schema_field("security_context_rule", "allow_globals").kind,
            FieldKind::List
        ));
    }

    #[test]
    fn color_and_raw_fields_round_trip() {
        let mut doc = ConfigDocument::from_str("[[window_rule]]\n").unwrap();
        let color = schema_field("window_rule", "border_color_focused");
        let workspace = schema_field("window_rule", "default_workspace");
        apply_field_text(&mut doc, "window_rule", 0, &color, "#E5C07BFF").unwrap();
        assert!(apply_field_text(&mut doc, "window_rule", 0, &color, "yellow").is_err());
        assert_eq!(field_text(&doc, "window_rule", 0, &color), "#E5C07BFF");
        apply_field_text(&mut doc, "window_rule", 0, &workspace, "3").unwrap();
        assert_eq!(
            doc.rule_integer("window_rule", 0, "default_workspace"),
            Some(3)
        );
        apply_field_text(&mut doc, "window_rule", 0, &workspace, "web").unwrap();
        assert_eq!(field_text(&doc, "window_rule", 0, &workspace), "\"web\"");
        assert_eq!(
            doc.rule_string("window_rule", 0, "default_workspace")
                .as_deref(),
            Some("web")
        );
    }

    #[test]
    fn rule_keys_round_trip() {
        let key = rule_key("window_rule", 1, 2, "match.app_id");
        assert_eq!(key, "window_rule[1:2].match.app_id");
        assert_eq!(
            parse_rule_key(&key),
            Some(("window_rule", Some(1), 2, "match.app_id"))
        );
        // Doc-less keys (a family-wide fallback target) still parse.
        assert_eq!(
            parse_rule_key("window_rule[2].opacity"),
            Some(("window_rule", None, 2, "opacity"))
        );
        assert_eq!(parse_rule_key("window_rule[2]"), None);
        assert_eq!(parse_rule_key("layout.gap"), None);
    }

    #[test]
    fn field_text_round_trips_every_kind() {
        let mut doc = ConfigDocument::from_str(
            "[[window_rule]]\nmatch.app_id = \"kitty\"\nopacity = 0.8\n\
             default_workspace = 3\ndefault_floating_size_px = { width = 1024, height = 768 }\n\
             default_scrolling_extent = 0.6\n\
             default_position = { x = 10, y = 20, anchor = \"top_left\" }\n",
        )
        .unwrap();
        doc.add_rule("window_rule");
        let index = doc.rule_count("window_rule") - 1;

        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            &field("window_rule", "match.app_id"),
            "firefox",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            &field("window_rule", "default_floating"),
            "true",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            &field("window_rule", "vrr"),
            "fullscreen",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            &field("window_rule", "hdr"),
            "off",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            &field("window_rule", "blur_ignore_alpha"),
            "1.5",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            &field("window_rule", "default_scrolling_column_order"),
            "-2",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            &field("window_rule", "default_output"),
            "DP-1",
        )
        .unwrap();

        let text = |key: &str| field_text(&doc, "window_rule", index, &field("window_rule", key));
        assert_eq!(text("match.app_id"), "firefox");
        assert_eq!(text("default_floating"), "true");
        assert_eq!(text("vrr"), "fullscreen");
        assert_eq!(text("hdr"), "off");
        // Clamped into the field's range.
        assert_eq!(text("blur_ignore_alpha"), "1");
        assert_eq!(text("default_scrolling_column_order"), "-2");
        assert_eq!(text("default_output"), "DP-1");
        // The pre-seeded first rule still reads back through the same path.
        assert_eq!(
            field_text(
                &doc,
                "window_rule",
                0,
                &field("window_rule", "default_floating_size_px")
            ),
            "1024x768"
        );
        assert_eq!(
            field_text(
                &doc,
                "window_rule",
                0,
                &field("window_rule", "default_scrolling_extent")
            ),
            "0.6"
        );
        assert_eq!(
            field_text(
                &doc,
                "window_rule",
                0,
                &field("window_rule", "default_position")
            ),
            "10, 20, top_left"
        );
    }

    #[test]
    fn list_fields_join_and_split() {
        let mut doc = ConfigDocument::from_str("[[security_context_rule]]\n").unwrap();
        apply_field_text(
            &mut doc,
            "security_context_rule",
            0,
            &field("security_context_rule", "allow_globals"),
            "zwlr_layer_shell_v1, zwlr_foreign_toplevel_v1",
        )
        .unwrap();
        assert_eq!(
            field_text(
                &doc,
                "security_context_rule",
                0,
                &field("security_context_rule", "allow_globals")
            ),
            "zwlr_layer_shell_v1, zwlr_foreign_toplevel_v1"
        );
        // An all-commas edit is an unset, not an empty write (umbriel
        // rejects those).
        apply_field_text(
            &mut doc,
            "security_context_rule",
            0,
            &field("security_context_rule", "allow_globals"),
            " , ",
        )
        .unwrap();
        assert_eq!(
            field_text(
                &doc,
                "security_context_rule",
                0,
                &field("security_context_rule", "allow_globals")
            ),
            ""
        );
    }

    #[test]
    fn empty_text_unsets_and_bad_input_is_rejected() {
        let mut doc = ConfigDocument::from_str("[[window_rule]]\n").unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            0,
            &field("window_rule", "match.title"),
            "editor",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            0,
            &field("window_rule", "match.title"),
            "",
        )
        .unwrap();
        assert_eq!(
            field_text(&doc, "window_rule", 0, &field("window_rule", "match.title")),
            ""
        );

        assert!(
            apply_field_text(
                &mut doc,
                "window_rule",
                0,
                &field("window_rule", "default_floating_size_px"),
                "1920x"
            )
            .is_err()
        );
        assert!(
            apply_field_text(
                &mut doc,
                "window_rule",
                0,
                &field("window_rule", "default_position"),
                "10, 20, middle"
            )
            .is_err()
        );
        assert!(
            apply_field_text(
                &mut doc,
                "window_rule",
                0,
                &field("window_rule", "vrr"),
                "sometimes"
            )
            .is_err()
        );
        assert!(
            apply_field_text(
                &mut doc,
                "window_rule",
                0,
                &field("window_rule", "opacity"),
                "opaque"
            )
            .is_err()
        );
        // Nothing was written by the failed parses.
        assert_eq!(doc.text(), "[[window_rule]]\n");
    }
}
