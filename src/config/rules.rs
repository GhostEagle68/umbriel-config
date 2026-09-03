//! Window- and layer-rule field vocabulary, mirroring umbriel's parser
//! (`src/config/config.cpp`). Rules are a fixed field set under dynamic
//! `[[window_rule]]` / `[[layer_rule]]` instances, like outputs inverted.

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
}
