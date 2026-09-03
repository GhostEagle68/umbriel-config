//! Output editor model. Outputs invert the assembled schema's shape: the
//! fields are a fixed vocabulary (mirroring umbriel's parser) under dynamic
//! per-monitor names in `[output."<name>"]` tables.

use super::document::ConfigDocument;

/// Const-friendly fallback value, mirroring umbriel's `OutputRule` defaults.
pub enum DefaultValue {
    Bool(bool),
    Float(f64),
    Text(&'static str),
}
/// One configurable field of an output, in display order.
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    /// Umbriel's fallback (its `OutputRule`), shown when the key is unset.
    pub default: Option<DefaultValue>,
}

pub enum FieldKind {
    Toggle,
    /// Fixed vocabulary the parser accepts, e.g. vrr's `disabled|always`.
    Choice(&'static [&'static str]),
    /// Free text, e.g. mode `"2560x1440@360"`.
    Text,
    /// `[x, y]` integer pair.
    Position,
    Float {
        min: f64,
        max: f64,
    },
    /// Integer count, name list, or `"dynamic"`.
    Workspaces,
}

/// The fields umbriel's output parser accepts (umbriel
/// `src/config/config.cpp`); defaults mirror its `OutputRule`. The nested
/// `layout.scrolling.default_width_fraction` override waits until fields can
/// carry dotted paths; the global copy is covered by the assembled schema.
pub const FIELDS: &[Field] = &[
    Field {
        key: "enabled",
        label: "Enabled",
        kind: FieldKind::Toggle,
        default: Some(DefaultValue::Bool(true)),
    },
    Field {
        key: "mode",
        label: "Mode",
        kind: FieldKind::Text,
        default: None,
    },
    Field {
        key: "position",
        label: "Position",
        kind: FieldKind::Position,
        default: None,
    },
    Field {
        key: "scale",
        label: "Scale",
        kind: FieldKind::Float {
            min: 0.25,
            max: 4.0,
        },
        // The parser leaves unset scale to the compositor (effectively 1.0).
        default: Some(DefaultValue::Float(1.0)),
    },
    Field {
        key: "vrr",
        label: "VRR",
        kind: FieldKind::Choice(&["disabled", "always", "fullscreen"]),
        default: Some(DefaultValue::Text("disabled")),
    },
    Field {
        key: "hdr",
        label: "HDR",
        kind: FieldKind::Choice(&["off", "on", "auto", "fullscreen"]),
        default: Some(DefaultValue::Text("off")),
    },
    Field {
        key: "sdr_white",
        label: "SDR white",
        kind: FieldKind::Float {
            min: 80.0,
            max: 1000.0,
        },
        default: Some(DefaultValue::Float(203.0)),
    },
    Field {
        key: "transform",
        label: "Transform",
        kind: FieldKind::Choice(&[
            "normal",
            "90",
            "180",
            "270",
            "flipped",
            "flipped-90",
            "flipped-180",
            "flipped-270",
        ]),
        default: Some(DefaultValue::Text("normal")),
    },
    Field {
        key: "tearing",
        label: "Tearing",
        kind: FieldKind::Toggle,
        default: Some(DefaultValue::Bool(false)),
    },
    Field {
        key: "direct_scanout",
        label: "Direct scanout",
        kind: FieldKind::Toggle,
        default: Some(DefaultValue::Bool(true)),
    },
    Field {
        key: "workspaces",
        label: "Workspaces",
        kind: FieldKind::Workspaces,
        // Omitted workspaces mean dynamic.
        default: Some(DefaultValue::Text("dynamic")),
    },
];

/// Output names configured in the document, in file order.
pub fn configured(doc: &ConfigDocument) -> Vec<String> {
    doc.table_names(&["output"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const FIXTURE: &str = "\
[output]
broken = 3

[output.\"DP-3\"]
scale = 1

[output.\"eDP-1\"]
mode = \"1920x1080@60\"
";

    #[test]
    fn configured_lists_table_names_in_file_order() {
        let doc = ConfigDocument::from_str(FIXTURE).unwrap();
        assert_eq!(
            configured(&doc),
            vec!["DP-3".to_owned(), "eDP-1".to_owned()]
        );
    }

    #[test]
    fn configured_is_empty_without_output_table() {
        let doc = ConfigDocument::from_str("[general]\nxwayland = true\n").unwrap();
        assert!(configured(&doc).is_empty());
    }

    #[test]
    fn field_keys_are_unique() {
        let mut keys: Vec<_> = FIELDS.iter().map(|field| field.key).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), total);
    }
}
