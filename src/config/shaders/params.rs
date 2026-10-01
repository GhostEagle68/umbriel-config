//! A preset's parameters (`palette`, `padding`, `speed`, `animated`,
//! `overlay`, `light.*`, `radius`) for the shader editor's strip: which
//! fields a kind has and how each is edited, from [`ShaderApi`] (so a
//! newer Umbriel's keys appear on their own), and how to read and write
//! them in the preset file the app wrote.

use super::api::{Param, ShaderApi};
use std::collections::BTreeMap;

/// A parameter's value as text: `true`/`false`, `80`, `1.5`, or the name
/// in `overlay`. A key that is missing from the map has its default.
pub type Values = BTreeMap<String, String>;

/// The key of the strip's own "Light" switch: Umbriel turns light on by
/// the presence of the `light` table, so it has no key of its own.
pub const LIGHT: &str = "light";

/// The header the app writes on a preset file. Only such a file is
/// rewritten; one without it belongs to someone else.
const HEADER: &str = "# Written by umbriel-config";

/// How the editor offers one parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum Control {
    Switch,
    Number {
        min: f64,
        max: f64,
        decimals: usize,
    },
    /// `overlay`: the name of a window preset, or none.
    Overlay,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub description: String,
    pub control: Control,
    pub default: String,
}

impl Field {
    /// Whether `value` is this field's default (numbers compare as
    /// numbers: `1` is `1.0`), so it needn't be stored.
    pub fn is_default(&self, value: &str) -> bool {
        let ty = match self.control {
            Control::Number { .. } => "float",
            Control::Switch => "bool",
            Control::Overlay => "string",
        };
        is_default(ty, value, &self.default)
    }
}

/// The fields a shader of `kind` has, in the docs' order. The `light.*`
/// keys come after a [`LIGHT`] switch that enables them. Keys of a type
/// the editor can't offer (text other than `overlay`) are left out.
pub fn fields(api: &ShaderApi, kind: &str) -> Vec<Field> {
    let mut out: Vec<Field> = Vec::new();
    for param in api.params_for(kind) {
        let Some(control) = control_of(param) else {
            continue;
        };
        if param.key.starts_with("light.") && !out.iter().any(|field| field.key == LIGHT) {
            out.push(Field {
                key: LIGHT.to_owned(),
                label: "Light".to_owned(),
                description: "Light from the ring spills onto what's around it.".to_owned(),
                control: Control::Switch,
                default: "false".to_owned(),
            });
        }
        out.push(Field {
            key: param.key.clone(),
            label: label_of(&param.key),
            description: param.description.clone(),
            control,
            default: param.default.clone(),
        });
    }
    out
}

/// `light.spread` -> "Spread", `animated` -> "Animated".
fn label_of(key: &str) -> String {
    let last = key.rsplit('.').next().unwrap_or(key).replace('_', " ");
    let mut chars = last.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// The schema's type when it gave one, else what the default looks like.
fn type_of(param: &Param) -> &str {
    if !param.ty.is_empty() {
        return &param.ty;
    }
    match param.default.as_str() {
        "true" | "false" => "bool",
        text if text.parse::<i64>().is_ok() => "int",
        text if text.parse::<f64>().is_ok() => "float",
        _ => "string",
    }
}

fn control_of(param: &Param) -> Option<Control> {
    match type_of(param) {
        "bool" => Some(Control::Switch),
        "string" if param.key == "overlay" => Some(Control::Overlay),
        ty @ ("int" | "float") => {
            let default: f64 = param.default.parse().unwrap_or(0.0);
            let from_text = range_in(&param.description);
            let min = param.min.or(from_text.map(|r| r.0)).unwrap_or(0.0);
            let max = param
                .max
                .or(from_text.map(|r| r.1))
                .unwrap_or_else(|| (default * 4.0).max(1.0));
            let decimals = match ty {
                "int" => 0,
                _ => param
                    .default
                    .split_once('.')
                    .map_or(1, |(_, tail)| tail.len().clamp(1, 2)),
            };
            Some(Control::Number { min, max, decimals })
        }
        _ => None,
    }
}

/// The range a description gives as "0 to 1024" or "0 to 1".
fn range_in(description: &str) -> Option<(f64, f64)> {
    let words: Vec<&str> = description.split_whitespace().collect();
    fn number(word: &str) -> &str {
        word.trim_matches(|c: char| c == ',' || c == '.' || c == ';')
    }
    words.windows(3).find_map(|window| {
        let (low, high) = (
            number(window[0]).parse().ok()?,
            number(window[2]).parse().ok()?,
        );
        (window[1] == "to" && low < high).then_some((low, high))
    })
}

/// Whether the app wrote `text` (a preset file it may rewrite).
pub fn app_written(text: &str) -> bool {
    text.starts_with(HEADER)
}

fn preset_table<'a>(
    doc: &'a mut toml_edit::DocumentMut,
    preset: &str,
) -> Option<&'a mut dyn toml_edit::TableLike> {
    doc.get_mut("effects")?
        .get_mut("preset")?
        .get_mut(preset)?
        .as_table_like_mut()
}

fn text_of(value: &toml_edit::Value) -> Option<String> {
    match value {
        toml_edit::Value::Boolean(flag) => Some(flag.value().to_string()),
        toml_edit::Value::Integer(number) => Some(number.value().to_string()),
        toml_edit::Value::Float(number) => Some(number.value().to_string()),
        toml_edit::Value::String(text) => Some(text.value().clone()),
        _ => None,
    }
}

/// The parameters `preset` sets in `text`, by key. An existing `light`
/// table counts as the [`LIGHT`] switch being on.
pub fn read(text: &str, preset: &str) -> Values {
    let mut values = Values::new();
    let Ok(mut doc) = text.parse::<toml_edit::DocumentMut>() else {
        return values;
    };
    let Some(table) = preset_table(&mut doc, preset) else {
        return values;
    };
    for (key, item) in table.iter() {
        if ["kind", "shader"].contains(&key) {
            continue;
        }
        if key == LIGHT
            && let Some(light) = item.as_table_like()
        {
            values.insert(LIGHT.to_owned(), "true".to_owned());
            for (sub, value) in light.iter() {
                if let Some(text) = value.as_value().and_then(text_of) {
                    values.insert(format!("{LIGHT}.{sub}"), text);
                }
            }
        } else if let Some(text) = item.as_value().and_then(text_of) {
            values.insert(key.to_owned(), text);
        }
    }
    values
}

/// Whether `value` is the same as `default` for a parameter of `ty`
/// (numbers compare as numbers: `1` is `1.0`).
fn is_default(ty: &str, value: &str, default: &str) -> bool {
    match ty {
        "int" | "float" => match (value.parse::<f64>(), default.parse::<f64>()) {
            (Ok(a), Ok(b)) => (a - b).abs() < 1e-9,
            _ => value == default,
        },
        _ => value == default || (value.is_empty() && default == "\"\""),
    }
}

fn toml_value(ty: &str, value: &str) -> Option<toml_edit::Item> {
    Some(match ty {
        "bool" => toml_edit::value(value.parse::<bool>().ok()?),
        "int" => toml_edit::value(value.parse::<f64>().ok()?.round() as i64),
        "float" => toml_edit::value(value.parse::<f64>().ok()?),
        _ => toml_edit::value(value),
    })
}

/// `text` with `preset`'s parameters set to `values` for a shader of
/// `kind`. A value equal to its default removes the key (the file stays
/// minimal), as does a key of another kind; the `light` table exists
/// exactly when the [`LIGHT`] switch is on. Keys the editor doesn't know
/// are left alone.
pub fn apply(
    text: &str,
    preset: &str,
    kind: &str,
    values: &Values,
    api: &ShaderApi,
) -> Result<String, String> {
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|err| format!("the preset file doesn't parse: {err}"))?;
    let table = preset_table(&mut doc, preset)
        .ok_or_else(|| format!("the preset file has no preset named {preset}"))?;
    let light_on = values.get(LIGHT).is_some_and(|value| value == "true");
    if light_on {
        if table
            .get(LIGHT)
            .and_then(toml_edit::Item::as_table_like)
            .is_none()
        {
            table.insert(LIGHT, toml_edit::Item::Table(toml_edit::Table::new()));
        }
    } else {
        table.remove(LIGHT);
    }
    for param in &api.params {
        let applies = api.params_for(kind).any(|own| own.key == param.key);
        let wanted = values
            .get(&param.key)
            .filter(|value| applies && !is_default(type_of(param), value, &param.default));
        let (parent, key) = match param.key.split_once('.') {
            Some((group, key)) => (Some(group), key),
            None => (None, param.key.as_str()),
        };
        let target: Option<&mut dyn toml_edit::TableLike> = match parent {
            None => Some(&mut *table),
            Some(group) => table
                .get_mut(group)
                .and_then(toml_edit::Item::as_table_like_mut),
        };
        let Some(target) = target else { continue };
        match wanted.and_then(|value| toml_value(type_of(param), value)) {
            Some(item) => {
                target.insert(key, item);
            }
            None => {
                target.remove(key);
            }
        }
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{umbriel_docs, umbriel_schema};

    fn api() -> ShaderApi {
        ShaderApi::build(umbriel_docs::BUNDLED, None)
    }

    fn file(body: &str) -> String {
        format!(
            "# Written by umbriel-config: the preset that runs a.glsl.\n\
             [effects.preset.a]\nkind = \"border\"\nshader = \"a.glsl\"\n{body}"
        )
    }

    fn keys(fields: &[Field]) -> Vec<&str> {
        fields.iter().map(|field| field.key.as_str()).collect()
    }

    #[test]
    fn a_kind_gets_its_own_fields() {
        let api = api();
        assert_eq!(
            keys(&fields(&api, "border")),
            [
                "palette",
                "padding",
                "speed",
                "animated",
                "overlay",
                "light",
                "light.spread",
                "light.intensity",
                "light.threshold"
            ]
        );
        assert_eq!(keys(&fields(&api, "window")), ["palette"]);
        assert_eq!(keys(&fields(&api, "animation")), ["palette"]);
        assert_eq!(keys(&fields(&api, "cursor")), ["palette", "radius"]);
    }

    #[test]
    fn a_value_at_its_default_is_recognised() {
        let api = api();
        let border = fields(&api, "border");
        let field = |key: &str| border.iter().find(|f| f.key == key).unwrap();
        assert!(field("speed").is_default("1"));
        assert!(field("speed").is_default("1.0"));
        assert!(!field("speed").is_default("2"));
        assert!(field("palette").is_default("false"));
        assert!(field("overlay").is_default(""));
        assert!(!field("overlay").is_default("scan"));
        assert!(field("light").is_default("false"));
    }

    #[test]
    fn controls_and_ranges_come_from_the_docs_and_the_schema() {
        let api = api();
        let border = fields(&api, "border");
        let field = |key: &str| border.iter().find(|f| f.key == key).unwrap();
        assert_eq!(field("palette").control, Control::Switch);
        assert_eq!(field("overlay").control, Control::Overlay);
        // Ranges read from "0 to 1024" in the description, decimals from the default.
        assert_eq!(
            field("padding").control,
            Control::Number {
                min: 0.0,
                max: 1024.0,
                decimals: 0
            }
        );
        assert_eq!(
            field("speed").control,
            Control::Number {
                min: 0.0,
                max: 10.0,
                decimals: 1
            }
        );
        assert_eq!(field("light.spread").label, "Spread");
        // The schema's typed range wins when Umbriel is installed.
        let schema = umbriel_schema::parse(umbriel_schema::FIXTURE).unwrap();
        let typed = ShaderApi::build(umbriel_docs::BUNDLED, Some(&schema));
        let spread = fields(&typed, "border")
            .into_iter()
            .find(|f| f.key == "light.spread")
            .unwrap();
        assert_eq!(
            spread.control,
            Control::Number {
                min: 1.0,
                max: 256.0,
                decimals: 0
            }
        );
    }

    #[test]
    fn a_newer_umbriels_numeric_key_gets_a_control() {
        let schema = umbriel_schema::parse(
            r#"{"version":1,"options":[
                {"path":"effects.preset.<name>.tilt","type":"float","min":0.0,"max":2.0},
                {"path":"effects.preset.<name>.note","type":"string"}]}"#,
        )
        .unwrap();
        let api = ShaderApi::build(umbriel_docs::BUNDLED, Some(&schema));
        let window = fields(&api, "window");
        // Of every kind (the docs don't say), numeric: offered; text: not.
        assert_eq!(keys(&window), ["palette", "tilt"]);
        assert_eq!(
            window[1].control,
            Control::Number {
                min: 0.0,
                max: 2.0,
                decimals: 1
            }
        );
    }

    #[test]
    fn reading_finds_set_values_and_the_light_switch() {
        let text = file(
            "speed = 2.5\npalette = true\noverlay = \"scan\"\n[effects.preset.a.light]\nspread = 120\n",
        );
        let values = read(&text, "a");
        assert_eq!(values["speed"], "2.5");
        assert_eq!(values["palette"], "true");
        assert_eq!(values["overlay"], "scan");
        assert_eq!(values[LIGHT], "true");
        assert_eq!(values["light.spread"], "120");
        assert!(!values.contains_key("kind") && !values.contains_key("shader"));
        assert!(read(&text, "other").is_empty());
        assert!(read("not toml [", "a").is_empty());
    }

    #[test]
    fn applying_writes_changes_and_drops_defaults() {
        let api = api();
        let mut values = Values::new();
        values.insert("speed".into(), "2.5".into());
        values.insert("padding".into(), "0".into());
        values.insert("animated".into(), "true".into());
        values.insert("palette".into(), "true".into());
        let out = apply(&file("padding = 12\n"), "a", "border", &values, &api).unwrap();
        let back = read(&out, "a");
        assert_eq!(back["speed"], "2.5");
        assert_eq!(back["palette"], "true");
        // Defaults aren't written, and an old non-default one is removed.
        assert!(!back.contains_key("padding") && !back.contains_key("animated"));
        assert!(app_written(&out) && out.contains("kind = \"border\""));
        // Nothing to change: the file is byte-identical.
        let plain = file("");
        assert_eq!(
            apply(&plain, "a", "border", &Values::new(), &api).unwrap(),
            plain
        );
    }

    #[test]
    fn light_exists_exactly_when_it_is_switched_on() {
        let api = api();
        let mut values = Values::new();
        values.insert(LIGHT.into(), "true".into());
        values.insert("light.spread".into(), "120".into());
        values.insert("light.intensity".into(), "1.0".into());
        let out = apply(&file(""), "a", "border", &values, &api).unwrap();
        let back = read(&out, "a");
        assert_eq!(back[LIGHT], "true");
        assert_eq!(back["light.spread"], "120");
        assert!(!back.contains_key("light.intensity"));
        // On with every value at its default: an empty table.
        values.remove("light.spread");
        let bare = apply(&out, "a", "border", &values, &api).unwrap();
        assert!(bare.contains("[effects.preset.a.light]") && !bare.contains("spread"));
        // Off: the whole table goes.
        values.remove(LIGHT);
        let off = apply(&bare, "a", "border", &values, &api).unwrap();
        assert!(!off.contains("light"));
    }

    #[test]
    fn keys_of_another_kind_are_dropped_and_unknown_ones_kept() {
        let api = api();
        let mut values = Values::new();
        values.insert("speed".into(), "2".into());
        values.insert("palette".into(), "true".into());
        let text = file("speed = 2.0\nmy_own = 7\n");
        // Now a window shader: speed is a border key and goes; palette stays.
        let out = apply(&text, "a", "window", &values, &api).unwrap();
        let back = read(&out, "a");
        assert!(!back.contains_key("speed"));
        assert_eq!(back["palette"], "true");
        assert_eq!(back["my_own"], "7");
    }

    #[test]
    fn a_missing_preset_or_broken_file_is_an_error() {
        let api = api();
        assert!(apply(&file(""), "nope", "border", &Values::new(), &api).is_err());
        assert!(apply("[x", "a", "border", &Values::new(), &api).is_err());
        assert!(!app_written("[effects.preset.a]\n"));
    }
}
