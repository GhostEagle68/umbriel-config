//! umbriel's settings, found by asking the installed umbriel when it can't
//! list them itself (`umbriel schema --json`). Candidate keys go into a
//! scratch config and `umbriel validate` judges each one: `unknown key`
//! drops it, `ignoring K (expected integer)` types it, silence means a
//! table, and out-of-range values clamp to its bounds. Candidates are the
//! docs' keys, the built-in rule and output fields, and every word in the
//! umbriel binary, so keys the docs miss are found too. The result is
//! cached per `umbriel --version`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::umbriel_schema::{Key, Schema};
use super::{discovery, outputs, rules, schema};

/// Tables that hold lists of rules, outputs, and the like, in the
/// schema's notation: `[]` marks an array of tables, `<name>` a
/// user-named table.
const CONTAINERS: &[&str] = &[
    "window_rule[]",
    "window_rule[].match",
    "layer_rule[]",
    "layer_rule[].match",
    "security_context_rule[]",
    "security_context_rule[].match",
    "output.<name>",
    "workspace[]",
    "input.device[]",
];

/// Sections left alone: file lists, binds, and GPU selection.
const SKIPPED: &[&str] = &["include", "keybinds", "drm"];

/// A section taking this share of the binary's words is a map of
/// user-named keys (`environment`, `animation.beziers`), not settings.
const MAP_SHARE: f64 = 0.3;

#[derive(Serialize, Deserialize)]
struct Cache {
    version: String,
    options: Vec<Key>,
}

/// The installed umbriel's settings, probed or from the cache; `None`
/// without umbriel, or when probing found nothing.
pub fn load(env: &discovery::Env, docs: &[schema::Entry]) -> Option<Schema> {
    let binary = on_path("umbriel")?;
    let version = version(&binary)?;
    let dir = discovery::state_dir(env);
    let cache_path = dir.join("probe.json");
    let cached = std::fs::read_to_string(&cache_path)
        .ok()
        .and_then(|text| serde_json::from_str::<Cache>(&text).ok())
        .filter(|cache| cache.version == version);
    let options = match cached {
        Some(cache) => cache.options,
        None => {
            std::fs::create_dir_all(&dir).ok()?;
            let words = std::fs::read(&binary).map(|bytes| words(&bytes));
            let options = probe(
                &binary,
                &dir.join("probe.toml"),
                &candidates(docs, &words.unwrap_or_default()),
            )?;
            let cache = Cache {
                version: version.clone(),
                options,
            };
            if let Ok(text) = serde_json::to_string(&cache) {
                let _ = std::fs::write(&cache_path, text);
            }
            cache.options
        }
    };
    Some(Schema {
        // `umbriel 0.1.0 (11f6b725c8a9)` → the build in parentheses.
        revision: Some(
            version
                .rsplit_once('(')
                .map_or(version.as_str(), |(_, build)| build.trim_end_matches(')'))
                .to_owned(),
        ),
        options,
    })
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

fn version(binary: &Path) -> Option<String> {
    let output = Command::new(binary).arg("--version").output().ok()?;
    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .to_owned();
    (!line.is_empty()).then_some(line)
}

/// Every word in `bytes` that could be a key: the printable runs (as
/// `strings` finds them) split on anything but `[a-z0-9_]`, 3 to 40
/// characters long.
fn words(bytes: &[u8]) -> BTreeSet<String> {
    bytes
        .split(|byte| !byte.is_ascii_graphic())
        .filter(|run| run.len() >= 3)
        .flat_map(|run| {
            run.split(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_'))
        })
        .filter(|word| (3..=40).contains(&word.len()))
        .map(|word| String::from_utf8_lossy(word).into_owned())
        .collect()
}

/// Candidate keys per section. `words` are tried in every section; the
/// share each section accepts tells maps from settings.
struct Candidates {
    known: BTreeMap<String, BTreeSet<String>>,
    words: BTreeSet<String>,
}

fn candidates(docs: &[schema::Entry], words: &BTreeSet<String>) -> Candidates {
    let mut known: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut add = |dotted: &str| {
        if let Some((section, key)) = dotted.rsplit_once('.')
            && !SKIPPED.contains(&section.split('.').next().unwrap_or_default())
        {
            // The docs write arrays of tables plainly (`input.device`).
            let section = CONTAINERS
                .iter()
                .filter_map(|container| container.strip_suffix("[]"))
                .find_map(|plain| {
                    let rest = section.strip_prefix(plain)?;
                    (rest.is_empty() || rest.starts_with('.')).then(|| format!("{plain}[]{rest}"))
                })
                .unwrap_or_else(|| section.to_owned());
            known.entry(section).or_default().insert(key.to_owned());
        }
    };
    for entry in docs {
        add(&entry.dotted());
    }
    for family in rules::families(None) {
        for field in family.matches.iter().chain(&family.settings) {
            add(&format!("{}[].{}", family.name, field.key));
        }
    }
    for field in outputs::fields(None) {
        add(&format!("output.<name>.{}", field.key));
    }
    for container in CONTAINERS {
        known.entry((*container).to_owned()).or_default();
    }
    Candidates {
        known,
        words: words.clone(),
    }
}

/// One line of the scratch file: a section header or a key under one.
#[derive(Clone)]
enum Line {
    Header(String),
    Key {
        section: String,
        key: String,
        word: bool,
    },
}

/// The scratch file: each section's keys set to `value(section, key)`,
/// plus what each line holds (index = line number - 1). A key naming a
/// sub-section is left out: writing it would redefine that table.
fn scratch(
    sections: &BTreeMap<String, Vec<(String, bool)>>,
    value: impl Fn(&str, &str) -> Option<String>,
) -> (String, Vec<Line>) {
    let mut text = String::new();
    let mut lines = Vec::new();
    for (section, keys) in sections {
        let table = section.replace("[]", "").replace("<name>", "PROBE");
        text.push_str(&if section.ends_with("[]") {
            format!("[[{table}]]\n")
        } else {
            format!("[{table}]\n")
        });
        lines.push(Line::Header(section.clone()));
        for (key, word) in keys {
            let child = format!("{section}.{key}").replace("[]", "");
            let is_table = sections.keys().any(|other| {
                let other = other.replace("[]", "");
                other == child || other.starts_with(&format!("{child}."))
            });
            let Some(value) = value(section, key).filter(|_| !is_table) else {
                continue;
            };
            text.push_str(&format!("{key} = {value}\n"));
            lines.push(Line::Key {
                section: section.clone(),
                key: key.clone(),
                word: *word,
            });
        }
    }
    (text, lines)
}

/// `umbriel validate`'s message per line of `file`.
fn messages(stderr: &str, file: &Path) -> BTreeMap<usize, String> {
    let prefix = format!("{}:", file.display());
    stderr
        .lines()
        .filter(|line| line.starts_with("warning: ") || line.starts_with("error: "))
        .filter_map(|line| {
            let (_, rest) = line.split_once(&prefix)?;
            let (number, rest) = rest.split_once(':')?;
            let (_, message) = rest.split_once(": ")?;
            Some((number.parse().ok()?, message.to_owned()))
        })
        .collect()
}

fn validate(binary: &Path, file: &Path, text: &str) -> Option<BTreeMap<usize, String>> {
    std::fs::write(file, text).ok()?;
    let output = Command::new(binary)
        .arg("validate")
        .arg("-c")
        .arg(file)
        .output()
        .ok();
    let _ = std::fs::remove_file(file);
    Some(messages(&String::from_utf8_lossy(&output?.stderr), file))
}

/// The schema type and enum values a pass-one message gives a key set
/// to `{}`, named the way `umbriel schema --json` names them: `None` for
/// an unknown key. `{}` is a table, so silence, or a complaint about its
/// contents (`x and y are required integers`), means one.
fn kind(message: Option<&str>) -> Option<(&'static str, Vec<String>)> {
    let Some(message) = message else {
        return Some(("table", Vec::new()));
    };
    if message.starts_with("unknown key") {
        return None;
    }
    // `ignoring K (detail)`, or the detail alone (`K must be a string`).
    let detail = message
        .strip_prefix("ignoring ")
        .and_then(|rest| rest.strip_suffix(')'))
        .and_then(|rest| rest.split_once(" ("))
        .map_or(message, |(_, detail)| detail);
    let named = |kind: &'static str| Some((kind, Vec::new()));
    let Some(expected) = detail.strip_prefix("expected ") else {
        return if let Some((_, values)) = detail.split_once(" must be a string (") {
            // `must be a string ("scrolling", "dwindle", or "master")`.
            let values: Vec<String> = values
                .split('"')
                .skip(1)
                .step_by(2)
                .map(str::to_owned)
                .collect();
            Some(("enum", values))
        } else if detail.contains(" must be ") && detail.ends_with("string") {
            named("string")
        } else if detail.contains(" must be ") {
            named("raw")
        } else {
            named("table")
        };
    };
    match expected {
        "integer" => named("int"),
        "number" => named("float"),
        "boolean" => named("bool"),
        "string" | "non-empty string" => named("string"),
        "color string" => named("color"),
        "array of strings" => named("string_array"),
        // `off|on|auto`: the values.
        values if values.contains('|') && !values.contains(' ') => {
            Some(("enum", values.split('|').map(str::to_owned).collect()))
        }
        // A shape with no widget (`8 or 10`, a number or a name): the
        // docs' or the built-in editor, else a raw row.
        _ => named("raw"),
    }
}

/// The bound a clamp message names (`… out of range, clamped to 240`).
fn clamp(message: Option<&str>) -> Option<f64> {
    message?.rsplit_once("clamped to ")?.1.trim().parse().ok()
}

fn probe(binary: &Path, file: &Path, candidates: &Candidates) -> Option<Vec<Key>> {
    // Pass one: every candidate as `{}`, which only a table accepts.
    let sections: BTreeMap<String, Vec<(String, bool)>> = candidates
        .known
        .iter()
        .map(|(section, keys)| {
            let mut all: Vec<(String, bool)> =
                keys.iter().map(|key| (key.clone(), false)).collect();
            all.extend(
                candidates
                    .words
                    .iter()
                    .filter(|word| !keys.contains(*word))
                    .map(|word| (word.clone(), true)),
            );
            (section.clone(), all)
        })
        .collect();
    let (text, lines) = scratch(&sections, |_, _| Some("{}".to_owned()));
    let found = validate(binary, file, &text)?;
    let message = |at: usize| found.get(&(at + 1)).map(String::as_str);

    // Sections umbriel doesn't know, and maps (most words accepted).
    let mut dropped: BTreeSet<&str> = BTreeSet::new();
    let mut accepted: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for (at, line) in lines.iter().enumerate() {
        match line {
            Line::Header(section) if message(at).is_some_and(|m| m.starts_with("unknown key")) => {
                dropped.insert(section);
            }
            Line::Key {
                section,
                word: true,
                ..
            } => {
                let (taken, tried) = accepted.entry(section).or_default();
                *tried += 1;
                if kind(message(at)).is_some() {
                    *taken += 1;
                }
            }
            _ => {}
        }
    }
    for (section, (taken, tried)) in &accepted {
        if *taken as f64 > *tried as f64 * MAP_SHARE {
            dropped.insert(section);
        }
    }

    let mut keys: Vec<Key> = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Line::Key { section, key, .. } = line else {
            continue;
        };
        if dropped.contains(section.as_str()) {
            continue;
        }
        if let Some((kind, values)) = kind(message(at)) {
            keys.push(Key {
                path: format!("{section}.{key}"),
                kind: kind.to_owned(),
                min: None,
                max: None,
                default: None,
                values,
                format: None,
            });
        }
    }
    if keys.is_empty() {
        return None;
    }

    // Passes two and three: numbers far out of range, clamped to their
    // bounds.
    let numeric: BTreeMap<String, Vec<(String, bool)>> = keys
        .iter()
        .filter(|key| key.kind == "int" || key.kind == "float")
        .filter_map(|key| key.path.rsplit_once('.'))
        .fold(BTreeMap::new(), |mut sections, (section, key)| {
            sections
                .entry(section.to_owned())
                .or_insert_with(Vec::new)
                .push((key.to_owned(), false));
            sections
        });
    for (value, max) in [("2147483647", true), ("-2147483648", false)] {
        let (text, lines) = scratch(&numeric, |_, _| Some(value.to_owned()));
        let found = validate(binary, file, &text)?;
        for (at, line) in lines.iter().enumerate() {
            let Line::Key { section, key, .. } = line else {
                continue;
            };
            let path = format!("{section}.{key}");
            if let (Some(bound), Some(entry)) = (
                clamp(found.get(&(at + 1)).map(String::as_str)),
                keys.iter_mut().find(|entry| entry.path == path),
            ) {
                if max {
                    entry.max = Some(bound);
                } else {
                    entry.min = Some(bound);
                }
            }
        }
    }
    Some(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_map_to_their_lines_and_types() {
        let file = Path::new("/state/probe.toml");
        let stderr = "12:00 [WRN] noise\n\
            warning: /state/probe.toml:2:1: ignoring animation.duration_ms (expected integer)\n\
            warning: /state/probe.toml:3:1: unknown key animation.bogus\n\
            warning: /state/probe.toml:4:1: animation.curve must be a string\n\
            warning: /state/probe.toml:6:1: ignoring output.PROBE.bit_depth (expected 8 or 10)\n\
            warning: /state/probe.toml:7:1: effects.max_fps = 2147483647 out of range, clamped to 240\n\
            configuration invalid";
        let found = messages(stderr, file);
        let at = |line: usize| found.get(&line).map(String::as_str);
        let named = |line| kind(at(line)).map(|(kind, _)| kind);
        assert_eq!(named(2), Some("int"));
        assert_eq!(named(3), None);
        assert_eq!(named(4), Some("string"));
        assert_eq!(named(5), Some("table"));
        assert_eq!(named(6), Some("raw"));
        assert_eq!(clamp(at(7)), Some(240.0));
        assert_eq!(
            kind(Some(
                "ignoring window_rule.vrr (expected disabled|always|fullscreen)"
            )),
            Some((
                "enum",
                vec![
                    "disabled".to_owned(),
                    "always".to_owned(),
                    "fullscreen".to_owned()
                ]
            ))
        );
        assert_eq!(
            kind(Some(
                "layout.mode must be a string (\"scrolling\", \"dwindle\", or \"master\")"
            )),
            Some((
                "enum",
                vec![
                    "scrolling".to_owned(),
                    "dwindle".to_owned(),
                    "master".to_owned()
                ]
            ))
        );
        // A table whose contents fall short is still a table.
        assert_eq!(
            kind(Some(
                "ignoring window_rule.default_position (x and y are required integers)"
            )),
            Some(("table", Vec::new()))
        );
        assert_eq!(
            kind(Some("input.device[0].name must be a non-empty string")),
            Some(("string", Vec::new()))
        );
        assert_eq!(
            kind(Some(
                "output.PROBE.workspaces must be a count, a name array, or \"dynamic\""
            )),
            Some(("raw", Vec::new()))
        );
    }

    #[test]
    fn scratch_files_skip_keys_that_name_sub_sections() {
        let sections: BTreeMap<String, Vec<(String, bool)>> = [
            (
                "window_rule[]".to_owned(),
                vec![("match".to_owned(), true), ("opacity".to_owned(), false)],
            ),
            (
                "window_rule[].match".to_owned(),
                vec![("app_id".to_owned(), false)],
            ),
            (
                "output.<name>".to_owned(),
                vec![("scale".to_owned(), false)],
            ),
        ]
        .into();
        let (text, lines) = scratch(&sections, |_, _| Some("{}".to_owned()));
        assert_eq!(
            text,
            "[output.PROBE]\nscale = {}\n\
             [[window_rule]]\nopacity = {}\n\
             [window_rule.match]\napp_id = {}\n"
        );
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn binary_words_are_key_shaped() {
        let found = words(b"\x00animation.duration_ms\x01\x02Ok\x00_ZN7Manager14tick\xff");
        assert!(found.contains("animation") && found.contains("duration_ms"));
        // Mangled names split on their capitals.
        assert!(found.contains("anager14tick") && !found.contains("Ok"));
    }

    /// Against the installed umbriel, when there is one.
    #[test]
    fn probing_the_installed_umbriel_finds_its_keys() {
        let Some(binary) = on_path("umbriel") else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("umbriel-probe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let docs = schema::assemble_docs(super::super::umbriel_docs::BUNDLED);
        let words = words(&std::fs::read(&binary).unwrap());
        let keys = probe(&binary, &dir.join("probe.toml"), &candidates(&docs, &words)).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        let find = |path: &str| keys.iter().find(|key| key.path == path);
        let duration = find("animation.duration_ms").expect("duration_ms");
        assert_eq!(
            (duration.kind.as_str(), duration.min, duration.max),
            ("int", Some(1.0), Some(10000.0))
        );
        assert_eq!(
            find("effects.cursor").map(|key| key.kind.as_str()),
            Some("string")
        );
        assert!(find("animation.windows_in.shader").is_none());
        assert!(find("window_rule[].opacity").is_some());
        assert!(find("window_rule[].match.is_floating").is_some());
        assert!(find("output.<name>.cyclic_workspaces").is_some());
        let mode = find("layout.mode").expect("layout.mode");
        assert!(mode.kind == "enum" && mode.values.contains(&"master".to_owned()));
        let vrr = find("window_rule[].vrr").expect("vrr");
        assert_eq!(vrr.kind, "enum");
        assert!(vrr.values.contains(&"fullscreen".to_owned()));
        // Maps of user-named keys are not settings.
        assert!(!keys.iter().any(|key| key.path.starts_with("environment.")));
    }
}
