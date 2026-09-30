//! What umbriel says a shader may use, read from its own documentation
//! and config schema so a newer umbriel's additions appear without an
//! app release: each kind's entry point, the `umbriel_*` names a shader
//! sees, and the parameters a preset takes. Umbriel has no command that
//! lists these; its docs (the effects page tables and the sentences that
//! say what each kind adds) and `umbriel config schema --json` are the
//! machine-readable sources.

use super::KINDS;
use crate::config::umbriel_docs;
use crate::config::umbriel_schema::Schema;

/// A `umbriel_*` uniform or helper function.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiName {
    pub name: String,
    /// The parameter list of a function, `(vec2 uv)`; empty for a uniform.
    pub signature: String,
    pub description: String,
    /// The kinds that see it; empty means every kind.
    pub kinds: Vec<String>,
}

/// A key a preset takes besides `kind` and `shader`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Relative to the preset table, so `light.spread` for
    /// `effects.preset.<name>.light.spread`.
    pub key: String,
    /// The kinds that take it; empty means every kind.
    pub kinds: Vec<String>,
    pub default: String,
    pub description: String,
    /// The schema's type (`bool`, `int`, `float`, `string`); empty when
    /// only the docs know the key.
    pub ty: String,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ShaderApi {
    /// `(kind, "vec4 border(vec2 uv)")`, in the docs' order.
    pub entry_points: Vec<(String, String)>,
    pub names: Vec<ApiName>,
    pub params: Vec<Param>,
}

impl ShaderApi {
    /// The API `docs` describe, with types and ranges from `schema`. Docs
    /// that yield nothing (umbriel restructured them) fall back to the
    /// copy bundled in the app.
    pub fn build(docs: &str, schema: Option<&Schema>) -> Self {
        let mut api = from_docs(docs);
        if api.entry_points.is_empty() || api.names.is_empty() {
            api = from_docs(umbriel_docs::BUNDLED);
        }
        if let Some(schema) = schema {
            overlay_schema(&mut api.params, schema);
        }
        api
    }

    /// The names a shader of `kind` sees.
    pub fn names_for<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a ApiName> {
        self.names
            .iter()
            .filter(move |item| takes(&item.kinds, kind))
    }

    /// The preset parameters a shader of `kind` takes.
    pub fn params_for<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a Param> {
        self.params
            .iter()
            .filter(move |item| takes(&item.kinds, kind))
    }
}

fn takes(kinds: &[String], kind: &str) -> bool {
    kinds.is_empty() || kinds.iter().any(|known| known == kind)
}

fn from_docs(docs: &str) -> ShaderApi {
    let page = effects_page(docs);
    let mut api = ShaderApi::default();
    let header = |table: &[Vec<String>], names: &[&str]| {
        table
            .first()
            .is_some_and(|row| row.iter().map(String::as_str).eq(names.iter().copied()))
    };
    // The animation page describes its uniforms in a table of its own;
    // the effects page only lists them.
    let described: Vec<(String, String, String)> = tables(docs)
        .iter()
        .filter(|table| header(table, &["Name", "Meaning"]))
        .flat_map(|table| table.iter().skip(1))
        .filter_map(|row| {
            let (name, signature) = split_signature(row.first()?);
            Some((name, signature, row.get(1)?.clone()))
        })
        .collect();
    let description_of = |name: &str| {
        described
            .iter()
            .find(|(known, ..)| known == name)
            .map_or_else(String::new, |(.., text)| text.clone())
    };
    for table in tables(&page) {
        if header(&table, &["Kind", "Entry point"]) {
            for row in table.iter().skip(1) {
                if let [kind, entry, ..] = row.as_slice() {
                    api.entry_points.push((kind.clone(), entry.clone()));
                }
            }
        } else if header(&table, &["Name", "Meaning"]) {
            for row in table.iter().skip(1) {
                if let Some(first) = row.first() {
                    let (name, signature) = split_signature(first);
                    let description = description_of(&name);
                    add_name(&mut api.names, name, signature, description, None);
                }
            }
        } else if header(&table, &["Key", "Kinds", "Default", "Description"]) {
            for row in table.iter().skip(1) {
                if let [key, kinds, default, description, ..] = row.as_slice()
                    && !["kind", "shader"].contains(&key.as_str())
                {
                    api.params.push(Param {
                        key: key.clone(),
                        kinds: kind_list(kinds),
                        default: default.clone(),
                        description: description.clone(),
                        ty: String::new(),
                        min: None,
                        max: None,
                    });
                }
            }
        }
    }
    // "Borders add `umbriel_border_hole` (...), and ..." names what one
    // kind sees on top of the shared names.
    for (kind, span) in added_names(&page) {
        let (name, signature) = split_signature(&span);
        let description = description_of(&name);
        add_name(&mut api.names, name, signature, description, Some(kind));
    }
    api
}

/// Add `name`, or add `kind` to the kinds of the entry already there.
/// `None` is a name every kind sees.
fn add_name(
    names: &mut Vec<ApiName>,
    name: String,
    signature: String,
    description: String,
    kind: Option<&str>,
) {
    let kinds: Vec<String> = kind.map(str::to_owned).into_iter().collect();
    match names.iter_mut().find(|known| known.name == name) {
        Some(known) => {
            if let Some(kind) = kind
                && !known.kinds.is_empty()
                && !known.kinds.iter().any(|have| have == kind)
            {
                known.kinds.push(kind.to_owned());
            }
        }
        None => names.push(ApiName {
            name,
            signature,
            description,
            kinds,
        }),
    }
}

/// `umbriel_sample(vec2 uv)` as (`umbriel_sample`, `(vec2 uv)`).
fn split_signature(text: &str) -> (String, String) {
    match text.split_once('(') {
        Some((name, rest)) => (name.trim().to_owned(), format!("({rest}")),
        None => (text.trim().to_owned(), String::new()),
    }
}

/// `all` is every kind (an empty list); otherwise `border, cursor`.
fn kind_list(cell: &str) -> Vec<String> {
    cell.split([',', ' '])
        .map(|word| word.trim().to_lowercase())
        .filter(|word| KINDS.contains(&word.as_str()))
        .collect()
}

/// The text of the docs' `# Effects` page.
fn effects_page(docs: &str) -> String {
    let mut page = String::new();
    let (mut inside, mut fenced) = (false, false);
    for line in docs.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with("# ") {
            inside = line.trim_end() == "# Effects";
        }
        if inside {
            page.push_str(line);
            page.push('\n');
        }
    }
    page
}

/// Every markdown table, header row first, separator row dropped, cell
/// backticks stripped.
fn tables(text: &str) -> Vec<Vec<Vec<String>>> {
    let mut found = Vec::new();
    let mut current: Vec<Vec<String>> = Vec::new();
    for line in text.lines().chain([""]) {
        let line = line.trim();
        if line.starts_with('|') {
            let row: Vec<String> = line
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().trim_matches('`').to_owned())
                .collect();
            if !row.iter().all(|cell| cell.starts_with("---")) {
                current.push(row);
            }
        } else if !current.is_empty() {
            found.push(std::mem::take(&mut current));
        }
    }
    found
}

/// `(kind, "`umbriel_x(...)`" span)` for every name a sentence of the
/// form "Borders add `umbriel_border_hole`, ..." gives a kind. Only
/// `umbriel_` spans count, so `uv` in a description isn't a name.
fn added_names(page: &str) -> Vec<(&'static str, String)> {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut fenced = false;
    for line in page.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            fenced = !fenced;
        }
        if fenced || trimmed.is_empty() || trimmed.starts_with(['|', '`', '#']) {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
        } else {
            current.push_str(trimmed);
            current.push(' ');
        }
    }
    paragraphs.push(current);
    let mut found = Vec::new();
    for sentence in paragraphs.iter().flat_map(|text| text.split(". ")) {
        let words: Vec<&str> = sentence.split_whitespace().take(3).collect();
        let Some(add) = words.iter().position(|word| *word == "add") else {
            continue;
        };
        let subject = words[0].to_lowercase();
        let Some(kind) = KINDS.iter().find(|kind| subject.starts_with(*kind)) else {
            continue;
        };
        if add == 0 || add > 2 {
            continue;
        }
        let rest = sentence.split_once(" add ").map_or("", |(_, rest)| rest);
        for span in rest.split('`').skip(1).step_by(2) {
            if span.starts_with("umbriel_") {
                found.push((*kind, span.to_owned()));
            }
        }
    }
    found
}

/// Types and ranges from umbriel's schema for the keys under
/// `effects.preset.<name>.`; a key only the schema has (newer than the
/// docs) is added, for every kind.
fn overlay_schema(params: &mut Vec<Param>, schema: &Schema) {
    for option in &schema.options {
        let Some(key) = option.path.strip_prefix("effects.preset.<name>.") else {
            continue;
        };
        if ["kind", "shader"].contains(&key) || ["table", "map"].contains(&option.kind.as_str()) {
            continue;
        }
        let index = params.iter().position(|param| param.key == key);
        let param = match index {
            Some(index) => &mut params[index],
            None => {
                params.push(Param {
                    key: key.to_owned(),
                    kinds: Vec::new(),
                    default: String::new(),
                    description: String::new(),
                    ty: String::new(),
                    min: None,
                    max: None,
                });
                params.last_mut().expect("just pushed")
            }
        };
        param.ty = option.kind.clone();
        param.min = option.min;
        param.max = option.max;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::umbriel_schema;

    fn bundled() -> ShaderApi {
        ShaderApi::build(umbriel_docs::BUNDLED, None)
    }

    fn name<'a>(api: &'a ShaderApi, name: &str) -> &'a ApiName {
        api.names
            .iter()
            .find(|item| item.name == name)
            .unwrap_or_else(|| panic!("{name} missing"))
    }

    #[test]
    fn the_bundled_docs_describe_every_kind() {
        let api = bundled();
        let kinds: Vec<&str> = api.entry_points.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(kinds, KINDS);
        assert!(
            api.entry_points
                .contains(&("border".to_owned(), "vec4 border(vec2 uv)".to_owned()))
        );
        // Shared names see every kind; each kind's additions only that kind,
        // and `uv` in a description isn't a name.
        let sample = name(&api, "umbriel_sample");
        assert_eq!(sample.signature, "(vec2 uv)");
        assert!(sample.kinds.is_empty());
        assert_eq!(name(&api, "umbriel_progress").kinds, ["animation"]);
        assert_eq!(
            name(&api, "umbriel_progress").description,
            "Eased progress, including overshoot"
        );
        assert_eq!(name(&api, "umbriel_border_distance").kinds, ["border"]);
        assert_eq!(name(&api, "umbriel_pointer").kinds, ["cursor"]);
        assert!(
            api.names
                .iter()
                .all(|item| item.name.starts_with("umbriel_"))
        );
        let cursor: Vec<&str> = api
            .names_for("cursor")
            .map(|item| item.name.as_str())
            .collect();
        assert!(cursor.contains(&"umbriel_sample") && cursor.contains(&"umbriel_pointer"));
        assert!(!cursor.contains(&"umbriel_progress"));
    }

    #[test]
    fn preset_parameters_carry_their_kinds() {
        let api = bundled();
        let key = |key: &str| api.params.iter().find(|param| param.key == key).unwrap();
        assert!(key("palette").kinds.is_empty());
        assert_eq!(key("speed").kinds, ["border"]);
        assert_eq!(key("radius").kinds, ["cursor"]);
        assert_eq!(key("light.spread").default, "80");
        // `kind` and `shader` are the preset's structure, not parameters.
        assert!(
            api.params
                .iter()
                .all(|p| p.key != "kind" && p.key != "shader")
        );
        assert!(api.params_for("window").all(|p| p.key == "palette"));
    }

    #[test]
    fn the_schema_adds_types_ranges_and_newer_keys() {
        let schema = umbriel_schema::parse(umbriel_schema::FIXTURE).unwrap();
        let api = ShaderApi::build(umbriel_docs::BUNDLED, Some(&schema));
        let spread = api.params.iter().find(|p| p.key == "light.spread").unwrap();
        assert_eq!(
            (spread.ty.as_str(), spread.min, spread.max),
            ("int", Some(1.0), Some(256.0))
        );
        let newer = umbriel_schema::parse(
            r#"{"version":1,"options":[
                {"path":"effects.preset.<name>.tilt","type":"float","min":0.0,"max":2.0},
                {"path":"effects.preset.<name>.extra","type":"table"}]}"#,
        )
        .unwrap();
        let api = ShaderApi::build(umbriel_docs::BUNDLED, Some(&newer));
        let tilt = api.params.iter().find(|p| p.key == "tilt").unwrap();
        assert!(tilt.kinds.is_empty());
        assert_eq!((tilt.ty.as_str(), tilt.max), ("float", Some(2.0)));
        assert!(api.params.iter().all(|p| p.key != "extra"));
    }

    #[test]
    fn a_newer_umbriel_shows_up_without_an_app_change() {
        let docs = umbriel_docs::BUNDLED
            .replace(
                "Cursor effects add `umbriel_pointer`,",
                "Cursor effects add `umbriel_pointer`, `umbriel_trail(float t)`,",
            )
            .replace(
                "| `umbriel_palette_count` |",
                "| `umbriel_scene` | The scene id. |\n| `umbriel_palette_count` |",
            );
        let api = ShaderApi::build(&docs, None);
        assert!(name(&api, "umbriel_scene").kinds.is_empty());
        assert_eq!(name(&api, "umbriel_trail").kinds, ["cursor"]);
        assert_eq!(name(&api, "umbriel_trail").signature, "(float t)");
        assert_eq!(name(&api, "umbriel_pointer").kinds, ["cursor"]);
    }

    /// The docs umbriel ships today parse the same way. Skipped where the
    /// sibling checkout is absent (CI).
    #[test]
    fn the_current_umbriel_effects_page_parses() {
        let Some(home) = std::env::var_os("HOME") else {
            return;
        };
        let path = std::path::Path::new(&home).join("Projects/umbriel/docs/user/effects.md");
        let Ok(page) = std::fs::read_to_string(path) else {
            return;
        };
        let api = from_docs(&page);
        let kinds: Vec<&str> = api.entry_points.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(kinds, KINDS);
        assert_eq!(name(&api, "umbriel_border_distance").kinds, ["border"]);
        assert!(
            bundled()
                .names
                .iter()
                .all(|old| api.names.iter().any(|n| n.name == old.name))
        );
        assert!(
            bundled()
                .params
                .iter()
                .all(|old| api.params.iter().any(|p| p.key == old.key))
        );
    }

    #[test]
    fn unreadable_docs_fall_back_to_the_bundled_copy() {
        assert_eq!(ShaderApi::build("# Nothing here\n", None), bundled());
    }
}
