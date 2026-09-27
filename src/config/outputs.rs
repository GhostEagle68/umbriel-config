//! Output editor model. Outputs invert the assembled schema's shape: the
//! fields are one vocabulary (umbriel's schema, else a built-in list)
//! under dynamic per-monitor names in `[output."<name>"]` tables.

use super::document::ConfigDocument;
use super::umbriel_schema;

/// Umbriel's fallback for an unset field, mirroring its `OutputRule`.
#[derive(Clone)]
pub enum DefaultValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
}

/// One configurable field of an output, in display order; `key` is dotted
/// within the output table (`"scale"`, `"layout.scrolling.default_extent_fraction"`).
#[derive(Clone)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    /// Umbriel's fallback (its `OutputRule`), shown when the key is unset.
    pub default: Option<DefaultValue>,
}

#[derive(Clone)]
pub enum FieldKind {
    Toggle,
    /// Fixed vocabulary the parser accepts, e.g. vrr's `disabled|always`.
    Choice(Vec<String>),
    /// Free text, e.g. mode `"2560x1440@360"`.
    Text,
    /// `[x, y]` integer pair.
    Position,
    Float {
        min: f64,
        max: f64,
    },
    Integer {
        min: i64,
        max: i64,
    },
    /// Integer count, name list, or `"dynamic"`.
    Workspaces,
    /// Anything else: TOML text as typed, else a string.
    Raw,
}

impl Field {
    /// The document path of this field on output `name`.
    pub fn path<'a>(&'a self, name: &'a str) -> Vec<&'a str> {
        let mut path = vec!["output", name];
        path.extend(self.key.split('.'));
        path
    }
}

fn field(key: &str, label: &str, kind: FieldKind, default: Option<DefaultValue>) -> Field {
    Field {
        key: key.to_owned(),
        label: label.to_owned(),
        kind,
        default,
    }
}

fn choice(values: &[&str]) -> FieldKind {
    FieldKind::Choice(values.iter().map(|value| (*value).to_owned()).collect())
}

/// The fields umbriel's output parser accepts (umbriel
/// `src/config/config.cpp`); defaults mirror its `OutputRule`. The
/// fallback for an umbriel without `umbriel schema`.
fn builtin() -> Vec<Field> {
    let text = |value: &str| Some(DefaultValue::Text(value.to_owned()));
    vec![
        field(
            "enabled",
            "Enabled",
            FieldKind::Toggle,
            Some(DefaultValue::Bool(true)),
        ),
        field("mode", "Mode", FieldKind::Text, None),
        field("position", "Position", FieldKind::Position, None),
        // The parser leaves unset scale to the compositor (effectively 1.0).
        field(
            "scale",
            "Scale",
            FieldKind::Float {
                min: 0.25,
                max: 4.0,
            },
            Some(DefaultValue::Float(1.0)),
        ),
        field(
            "vrr",
            "VRR",
            choice(&["disabled", "always", "fullscreen"]),
            text("disabled"),
        ),
        field(
            "hdr",
            "HDR",
            choice(&["off", "on", "auto", "fullscreen"]),
            text("off"),
        ),
        field(
            "sdr_white",
            "SDR white",
            FieldKind::Float {
                min: 80.0,
                max: 1000.0,
            },
            Some(DefaultValue::Float(203.0)),
        ),
        field(
            "transform",
            "Transform",
            choice(&[
                "normal",
                "90",
                "180",
                "270",
                "flipped",
                "flipped-90",
                "flipped-180",
                "flipped-270",
            ]),
            text("normal"),
        ),
        field(
            "tearing",
            "Tearing",
            FieldKind::Toggle,
            Some(DefaultValue::Bool(false)),
        ),
        field(
            "direct_scanout",
            "Direct scanout",
            FieldKind::Toggle,
            Some(DefaultValue::Bool(true)),
        ),
        // umbriel's schema says 8 to 10, but its parser takes only
        // these two.
        field(
            "bit_depth",
            "Bit depth",
            choice(&["8", "10"]),
            Some(DefaultValue::Integer(8)),
        ),
        // Omitted workspaces mean dynamic.
        field(
            "workspaces",
            "Workspaces",
            FieldKind::Workspaces,
            text("dynamic"),
        ),
    ]
}

/// The output fields from umbriel's schema when there is one, else the
/// built-in list.
pub fn fields(schema: Option<&[umbriel_schema::Key]>) -> Vec<Field> {
    match schema {
        Some(keys) => from_umbriel(keys),
        None => builtin(),
    }
}

/// Every `output.<name>.*` key umbriel reads, typed by its schema. The
/// built-in list still supplies labels, order, and the editors for shapes
/// the schema can't name (position, workspaces, the mode string).
fn from_umbriel(keys: &[umbriel_schema::Key]) -> Vec<Field> {
    let known = builtin();
    let mut fields: Vec<Field> = keys
        .iter()
        .filter_map(|key| {
            let path = key.path.strip_prefix("output.<name>.")?;
            let builtin = known.iter().find(|field| field.key == path);
            let bounded = key.min.is_some() && key.max.is_some();
            let kind = match (key.kind.as_str(), builtin) {
                // Nested tables are edited through their leaves.
                ("table", _) => return None,
                ("bool", _) => FieldKind::Toggle,
                ("enum", _) => FieldKind::Choice(key.values.clone()),
                (_, Some(builtin)) if matches!(builtin.kind, FieldKind::Choice(_)) => {
                    builtin.kind.clone()
                }
                ("float", _) if bounded => FieldKind::Float {
                    min: key.min.unwrap_or_default(),
                    max: key.max.unwrap_or_default(),
                },
                ("int", _) if bounded => FieldKind::Integer {
                    min: key.min.unwrap_or_default() as i64,
                    max: key.max.unwrap_or_default() as i64,
                },
                (_, Some(builtin)) => builtin.kind.clone(),
                ("string", _) => FieldKind::Text,
                _ => FieldKind::Raw,
            };
            let default = match &key.default {
                Some(serde_json::Value::Bool(value)) => Some(DefaultValue::Bool(*value)),
                Some(serde_json::Value::Number(number)) => match kind {
                    FieldKind::Integer { .. } => number.as_i64().map(DefaultValue::Integer),
                    _ => number.as_f64().map(DefaultValue::Float),
                },
                Some(serde_json::Value::String(text)) if !text.is_empty() => {
                    Some(DefaultValue::Text(text.clone()))
                }
                _ => None,
            }
            .or_else(|| builtin.and_then(|builtin| builtin.default.clone()));
            let label = builtin.map_or_else(
                || super::schema::humanize(path.rsplit('.').next().unwrap_or(path)),
                |builtin| builtin.label.clone(),
            );
            Some(Field {
                key: path.to_owned(),
                label,
                kind,
                default,
            })
        })
        .collect();
    // The built-in order first, new keys after.
    fields.sort_by_key(|field| {
        known
            .iter()
            .position(|builtin| builtin.key == field.key)
            .unwrap_or(usize::MAX)
    });
    fields
}

/// Split an `output.<name>.<field>` row key into the output name and its
/// field.
pub fn split_key<'a>(fields: &'a [Field], key: &'a str) -> Option<(&'a str, &'a Field)> {
    let rest = key.strip_prefix("output.")?;
    fields.iter().find_map(|field| {
        let name = rest.strip_suffix(field.key.as_str())?.strip_suffix('.')?;
        (!name.is_empty()).then_some((name, field))
    })
}

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
        let schema = umbriel_schema::parse(umbriel_schema::FIXTURE).unwrap();
        for fields in [fields(None), fields(Some(&schema.options))] {
            let mut keys: Vec<_> = fields.iter().map(|field| field.key.as_str()).collect();
            let total = keys.len();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), total);
        }
    }

    #[test]
    fn schema_fields_cover_what_umbriel_reads() {
        let schema = umbriel_schema::parse(umbriel_schema::FIXTURE).unwrap();
        let fields = fields(Some(&schema.options));
        let kind = |key: &str| &fields.iter().find(|field| field.key == key).unwrap().kind;
        // The built-in editors survive, the built-in order leads.
        assert_eq!(fields[0].key, "enabled");
        assert!(matches!(kind("position"), FieldKind::Position));
        assert!(matches!(kind("workspaces"), FieldKind::Workspaces));
        assert!(matches!(kind("mode"), FieldKind::Text));
        // Keys the built-in list never had.
        assert!(matches!(
            kind("min_workspaces"),
            FieldKind::Integer { min: 1, max: 64 }
        ));
        assert!(matches!(kind("workspace_axis"), FieldKind::Choice(_)));
        assert!(matches!(
            kind("layout.scrolling.default_extent_fraction"),
            FieldKind::Float { .. }
        ));
        assert!(!fields.iter().any(|field| field.key == "layout"));
    }

    #[test]
    fn split_key_handles_dotted_fields() {
        let fields = fields(
            umbriel_schema::parse(umbriel_schema::FIXTURE)
                .as_ref()
                .map(|schema| schema.options.as_slice()),
        );
        let (name, field) = split_key(
            &fields,
            "output.DP-3.layout.scrolling.default_extent_fraction",
        )
        .unwrap();
        assert_eq!(
            (name, field.key.as_str()),
            ("DP-3", "layout.scrolling.default_extent_fraction")
        );
        let (name, field) = split_key(&fields, "output.DP-3.scale").unwrap();
        assert_eq!((name, field.key.as_str()), ("DP-3", "scale"));
        assert!(split_key(&fields, "output.DP-3.nope").is_none());
    }
}
