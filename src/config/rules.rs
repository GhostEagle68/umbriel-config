//! Window- and layer-rule field vocabulary, mirroring umbriel's parser
//! (`src/config/config.cpp`). Rules are a fixed field set under dynamic
//! `[[window_rule]]` / `[[layer_rule]]` instances, like outputs inverted.

use super::document::ConfigDocument;

/// One configurable field of a rule; `key` is dotted within the rule table
/// (`"match.app_id"`, `"default_floating"`).
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
}

pub enum FieldKind {
    /// Free text; match fields hold regular expressions.
    Text,
    Toggle,
    Choice(&'static [&'static str]),
    Float {
        min: f64,
        max: f64,
    },
    Integer {
        min: i64,
        max: i64,
    },
    /// `[width, height]` positive integers.
    Size,
    /// Inline `{ x, y, anchor }`; anchors: top_left, top_right, bottom_left,
    /// bottom_right, top, bottom, left, right, center.
    Position,
    /// Array of strings, edited comma-separated.
    List,
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
pub const WINDOW_MATCH: &[Field] = &[
    Field {
        key: "match.app_id",
        label: "App id (regex)",
        kind: FieldKind::Text,
    },
    Field {
        key: "match.title",
        label: "Title (regex)",
        kind: FieldKind::Text,
    },
    Field {
        key: "match.xdg_tag",
        label: "Xdg tag (regex)",
        kind: FieldKind::Text,
    },
    Field {
        key: "match.content_type",
        label: "Content type",
        kind: FieldKind::Choice(&["none", "photo", "video", "game"]),
    },
    Field {
        key: "match.is_focused",
        label: "Is focused",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "match.at_startup",
        label: "At startup",
        kind: FieldKind::Toggle,
    },
];

/// What a window rule can configure.
pub const WINDOW_SETTINGS: &[Field] = &[
    Field {
        key: "default_floating",
        label: "Floating",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "default_fullscreen",
        label: "Fullscreen",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "default_maximize",
        label: "Maximize",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "default_maximize_to_edges",
        label: "Maximize to edges",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "default_focused",
        label: "Focused",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "default_pinned",
        label: "Pinned",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "focus_on_activate",
        label: "Focus on activate",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "tearing",
        label: "Tearing",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "blur",
        label: "Blur",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "blur_popups",
        label: "Blur popups",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "blur_optimized",
        label: "Blur optimized",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "opacity",
        label: "Opacity",
        kind: FieldKind::Float { min: 0.0, max: 1.0 },
    },
    Field {
        key: "blur_ignore_alpha",
        label: "Blur ignore alpha",
        kind: FieldKind::Float { min: 0.0, max: 1.0 },
    },
    Field {
        key: "vrr",
        label: "VRR",
        kind: FieldKind::Choice(&["disabled", "always", "fullscreen"]),
    },
    Field {
        key: "hdr",
        label: "HDR",
        kind: FieldKind::Choice(&["off", "on", "auto", "fullscreen"]),
    },
    Field {
        key: "default_output",
        label: "Output",
        kind: FieldKind::Text,
    },
    Field {
        key: "default_size",
        label: "Size",
        kind: FieldKind::Size,
    },
    Field {
        key: "default_position",
        label: "Position",
        kind: FieldKind::Position,
    },
    Field {
        key: "default_width",
        label: "Width fraction",
        kind: FieldKind::Float { min: 0.1, max: 1.0 },
    },
    Field {
        key: "default_height",
        label: "Height fraction",
        kind: FieldKind::Float { min: 0.1, max: 1.0 },
    },
    Field {
        key: "default_workspace",
        label: "Workspace",
        kind: FieldKind::Integer { min: 1, max: 64 },
    },
    Field {
        key: "default_scrolling_column",
        label: "Scrolling column",
        kind: FieldKind::Text,
    },
    Field {
        key: "default_scrolling_column_order",
        label: "Scrolling column order",
        kind: FieldKind::Integer {
            min: i64::MIN,
            max: i64::MAX,
        },
    },
];

/// What a layer rule can match on.
pub const LAYER_MATCH: &[Field] = &[Field {
    key: "match.namespace",
    label: "Namespace (regex)",
    kind: FieldKind::Text,
}];

/// What a layer rule can configure.
pub const LAYER_SETTINGS: &[Field] = &[
    Field {
        key: "blur",
        label: "Blur",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "blur_popups",
        label: "Blur popups",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "blur_optimized",
        label: "Blur optimized",
        kind: FieldKind::Toggle,
    },
    Field {
        key: "blur_ignore_alpha",
        label: "Blur ignore alpha",
        kind: FieldKind::Float { min: 0.0, max: 1.0 },
    },
];

/// What a security-context rule can match on (sandboxed apps).
pub const SECURITY_MATCH: &[Field] = &[
    Field {
        key: "match.sandbox_engine",
        label: "Sandbox engine (regex)",
        kind: FieldKind::Text,
    },
    Field {
        key: "match.app_id",
        label: "App id (regex)",
        kind: FieldKind::Text,
    },
];

/// What a security-context rule can configure: the wayland globals a
/// sandboxed client may bind. umbriel rejects an empty list, so the UI
/// unsets the key instead of writing one.
pub const SECURITY_SETTINGS: &[Field] = &[Field {
    key: "allow_globals",
    label: "Allow globals",
    kind: FieldKind::List,
}];

/// The rule families the UI edits, keyed by TOML section name.
pub fn fields(name: &str) -> (&'static [Field], &'static [Field]) {
    match name {
        "window_rule" => (WINDOW_MATCH, WINDOW_SETTINGS),
        "layer_rule" => (LAYER_MATCH, LAYER_SETTINGS),
        _ => (SECURITY_MATCH, SECURITY_SETTINGS),
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
        FieldKind::Text => doc
            .rule_string(family, index, field.key)
            .unwrap_or_default(),
        FieldKind::Toggle => doc
            .rule_bool(family, index, field.key)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        FieldKind::Choice(_) => doc
            .rule_string(family, index, field.key)
            .unwrap_or_default(),
        FieldKind::Float { .. } => doc
            .rule_float(family, index, field.key)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        FieldKind::Integer { .. } => doc
            .rule_integer(family, index, field.key)
            .map(|value| value.to_string())
            .unwrap_or_default(),
        FieldKind::Size => match doc.rule_integers(family, index, field.key).as_deref() {
            Some([width, height]) => format!("{width}x{height}"),
            _ => String::new(),
        },
        FieldKind::Position => match doc.rule_position(family, index, field.key) {
            Some((x, y, Some(anchor))) => format!("{x}, {y}, {anchor}"),
            Some((x, y, None)) => format!("{x}, {y}"),
            None => String::new(),
        },
        FieldKind::List => doc
            .rule_strings(family, index, field.key)
            .map(|values| values.join(", "))
            .unwrap_or_default(),
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
        doc.rule_unset(family, index, field.key);
        return Ok(());
    }
    match &field.kind {
        FieldKind::Text => {
            doc.rule_set_string(family, index, field.key, raw);
        }
        FieldKind::Toggle => match raw {
            "true" | "false" => doc.rule_set_bool(family, index, field.key, raw == "true"),
            _ => return Err(format!("'{raw}' is not true or false")),
        },
        FieldKind::Choice(options) => {
            let Some(choice) = options.iter().find(|option| **option == raw) else {
                return Err(format!("'{raw}' is not one of the accepted values"));
            };
            doc.rule_set_string(family, index, field.key, choice);
        }
        FieldKind::Float { min, max } => {
            let value: f64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a number"))?;
            doc.rule_set_float(family, index, field.key, value.clamp(*min, *max));
        }
        FieldKind::Integer { min, max } => {
            let value: i64 = raw
                .parse()
                .map_err(|_| format!("'{raw}' is not a whole number"))?;
            doc.rule_set_integer(family, index, field.key, value.clamp(*min, *max));
        }
        FieldKind::Size => {
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
            if parts.next().is_some() || width <= 0 || height <= 0 {
                return Err(format!("'{raw}' is not a size like 1920x1080"));
            }
            doc.rule_set_integers(family, index, field.key, &[width, height]);
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
            doc.rule_set_position(family, index, field.key, x, y, anchor);
        }
        FieldKind::List => {
            let values: Vec<String> = raw
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect();
            if values.is_empty() {
                doc.rule_unset(family, index, field.key);
                return Ok(());
            }
            doc.rule_set_strings(family, index, field.key, &values);
        }
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
        if let Some(value) = doc.rule_string(name, index, field.key)
            && !value.is_empty()
        {
            let leaf = field.key.rsplit('.').next().unwrap_or(field.key);
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
        let mut keys: Vec<_> = fields.iter().map(|field| field.key).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        keys.len() == total
    }

    #[test]
    fn field_keys_are_unique_per_list() {
        assert!(unique(WINDOW_MATCH));
        assert!(unique(WINDOW_SETTINGS));
        assert!(unique(LAYER_MATCH));
        assert!(unique(LAYER_SETTINGS));
        assert!(unique(SECURITY_MATCH));
        assert!(unique(SECURITY_SETTINGS));
    }

    #[test]
    fn bool_and_choice_fields_use_their_kinds() {
        assert!(matches!(
            WINDOW_SETTINGS
                .iter()
                .find(|field| field.key == "default_floating")
                .unwrap()
                .kind,
            FieldKind::Toggle
        ));
        assert!(matches!(
            WINDOW_MATCH
                .iter()
                .find(|field| field.key == "match.content_type")
                .unwrap()
                .kind,
            FieldKind::Choice(_)
        ));
    }

    fn field<'a>(family: &str, key: &str) -> &'a Field {
        let (matches, settings) = fields(family);
        matches
            .iter()
            .chain(settings)
            .find(|field| field.key == key)
            .unwrap()
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
             default_workspace = 3\ndefault_size = [1024, 768]\n\
             default_position = { x = 10, y = 20, anchor = \"top_left\" }\n",
        )
        .unwrap();
        doc.add_rule("window_rule");
        let index = doc.rule_count("window_rule") - 1;

        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            field("window_rule", "match.app_id"),
            "firefox",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            field("window_rule", "default_floating"),
            "true",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            field("window_rule", "vrr"),
            "fullscreen",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            field("window_rule", "hdr"),
            "off",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            field("window_rule", "blur_ignore_alpha"),
            "1.5",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            field("window_rule", "default_scrolling_column_order"),
            "-2",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            index,
            field("window_rule", "default_output"),
            "DP-1",
        )
        .unwrap();

        let text = |key: &str| field_text(&doc, "window_rule", index, field("window_rule", key));
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
            field_text(&doc, "window_rule", 0, field("window_rule", "default_size")),
            "1024x768"
        );
        assert_eq!(
            field_text(
                &doc,
                "window_rule",
                0,
                field("window_rule", "default_position")
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
            field("security_context_rule", "allow_globals"),
            "zwlr_layer_shell_v1, zwlr_foreign_toplevel_v1",
        )
        .unwrap();
        assert_eq!(
            field_text(
                &doc,
                "security_context_rule",
                0,
                field("security_context_rule", "allow_globals")
            ),
            "zwlr_layer_shell_v1, zwlr_foreign_toplevel_v1"
        );
        // An all-commas edit is an unset, not an empty write (umbriel
        // rejects those).
        apply_field_text(
            &mut doc,
            "security_context_rule",
            0,
            field("security_context_rule", "allow_globals"),
            " , ",
        )
        .unwrap();
        assert_eq!(
            field_text(
                &doc,
                "security_context_rule",
                0,
                field("security_context_rule", "allow_globals")
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
            field("window_rule", "match.title"),
            "editor",
        )
        .unwrap();
        apply_field_text(
            &mut doc,
            "window_rule",
            0,
            field("window_rule", "match.title"),
            "",
        )
        .unwrap();
        assert_eq!(
            field_text(&doc, "window_rule", 0, field("window_rule", "match.title")),
            ""
        );

        assert!(
            apply_field_text(
                &mut doc,
                "window_rule",
                0,
                field("window_rule", "default_size"),
                "1920x"
            )
            .is_err()
        );
        assert!(
            apply_field_text(
                &mut doc,
                "window_rule",
                0,
                field("window_rule", "default_position"),
                "10, 20, middle"
            )
            .is_err()
        );
        assert!(
            apply_field_text(
                &mut doc,
                "window_rule",
                0,
                field("window_rule", "vrr"),
                "sometimes"
            )
            .is_err()
        );
        assert!(
            apply_field_text(
                &mut doc,
                "window_rule",
                0,
                field("window_rule", "opacity"),
                "opaque"
            )
            .is_err()
        );
        // Nothing was written by the failed parses.
        assert_eq!(doc.text(), "[[window_rule]]\n");
    }
}
