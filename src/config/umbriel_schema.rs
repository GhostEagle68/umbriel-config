//! umbriel's own description of its config, from `umbriel config schema --json`:
//! every key its parser reads, with type, range and default. When the
//! installed umbriel has the command, it is the source of the settings
//! pages and rule editors; otherwise the docs are (see
//! [`super::umbriel_docs`]).

use std::process::Command;

use serde::{Deserialize, Serialize};

/// One key umbriel reads. `path` is dotted; arrays of tables appear as
/// `window_rule[].opacity` and user-named tables as `output.<name>.scale`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Key {
    pub path: String,
    /// `bool`, `int`, `float`, `string`, `color`, `enum`, `table`, `map`,
    /// `array_of_tables`, `*_array`, or a compound such as `int_or_string`.
    #[serde(rename = "type")]
    pub kind: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub values: Vec<String>,
    /// What a string means: `curve`, `action`, `path`, `regex`, ….
    pub format: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Schema {
    /// The umbriel build that printed it (`810711bc8009`).
    pub revision: Option<String>,
    pub options: Vec<Key>,
}

/// The installed umbriel's schema; `None` when umbriel is missing or too
/// old to have the command.
pub fn load() -> Option<Schema> {
    let output = Command::new("umbriel")
        .args(["config", "schema", "--json"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    parse(&String::from_utf8_lossy(&output.stdout))
}

pub fn parse(json: &str) -> Option<Schema> {
    serde_json::from_str::<Schema>(json)
        .ok()
        .filter(|schema| !schema.options.is_empty())
}

/// A real `umbriel config schema --json` output, for tests across the crate.
#[cfg(test)]
pub const FIXTURE: &str = include_str!("testdata/umbriel-schema.json");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_fixture_and_ignores_unknown_fields() {
        let keys = parse(FIXTURE).expect("fixture parses").options;
        let key = keys
            .iter()
            .find(|key| key.path == "appearance.border_width")
            .expect("border_width");
        assert_eq!(key.kind, "int");
        assert_eq!((key.min, key.max), (Some(0.0), Some(100.0)));
        assert!(parse(r#"{"options":[{"path":"a.b","type":"bool","new":1}]}"#).is_some());
    }

    #[test]
    fn junk_or_empty_output_is_no_schema() {
        assert!(parse("usage: umbriel …").is_none());
        assert!(parse(r#"{"options":[]}"#).is_none());
    }
}
