//! Lossless editing of an Umbriel config file.
//!
//! Built on `toml_edit` so every write preserves comments, ordering, and
//! whitespace. Paths address regular tables only
//! (`["animation", "windows_in", "duration_ms"]`); inline-table keys such as
//! window-rule `default_position` arrive with the rules editor in a later
//! phase.

use std::fs;
use std::path::{Path, PathBuf};

use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid TOML in {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml_edit::TomlError,
    },

    #[error("failed to save {path}: {source}")]
    Save {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl ConfigError {
    /// True when the config simply does not exist yet — a fresh start,
    /// not a broken file.
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            ConfigError::Read { source, .. } if source.kind() == std::io::ErrorKind::NotFound
        )
    }
}

/// A loaded Umbriel config file, editable without losing comments or formatting.
#[derive(Debug)]
pub struct ConfigDocument {
    doc: DocumentMut,
    original: String,
}

/// One `[keybinds]` entry: a chord plus the action it runs. `None` extras
/// mean the plain string form (`"Mod+Q" = "window-close"`); `Some` extras
/// the inline-table form (`"Mod+R" = { action = "...", repeat = false }`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeybindEntry {
    pub chord: String,
    pub action: String,
    pub repeat: Option<bool>,
    pub allow_when_locked: Option<bool>,
    pub submap: Option<String>,
}

impl std::str::FromStr for ConfigDocument {
    type Err = ConfigError;

    fn from_str(text: &str) -> Result<Self, ConfigError> {
        let doc: DocumentMut = text.parse().map_err(|source| ConfigError::Parse {
            path: PathBuf::from("<memory>"),
            source,
        })?;
        Ok(Self {
            doc,
            original: text.to_owned(),
        })
    }
}

impl ConfigDocument {
    /// Load from `path`.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let doc: DocumentMut = text.parse().map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self {
            doc,
            original: text,
        })
    }

    /// Current document text (comments and layout intact).
    pub fn text(&self) -> String {
        self.doc.to_string()
    }

    /// The text as first loaded (or as last saved).
    pub fn original_text(&self) -> &str {
        &self.original
    }

    /// Whether the document differs from what was loaded.
    pub fn is_modified(&self) -> bool {
        self.doc.to_string() != self.original
    }

    /// Atomically write the document to `path`, leaving a one-time backup at
    /// `<path>.bak` holding the pre-GUI content from the first-ever save.
    pub fn save(&mut self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|source| ConfigError::Save {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let backup = backup_path(path);
        if !backup.exists() && path.exists() {
            fs::copy(path, &backup).map_err(|source| ConfigError::Save {
                path: backup.clone(),
                source,
            })?;
        }
        let tmp = path.with_file_name(format!(
            ".{}.{}.tmp",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("config"),
            std::process::id()
        ));
        fs::write(&tmp, self.doc.to_string()).map_err(|source| ConfigError::Save {
            path: path.to_path_buf(),
            source,
        })?;
        fs::rename(&tmp, path).map_err(|source| ConfigError::Save {
            path: path.to_path_buf(),
            source,
        })?;
        self.original = self.doc.to_string();
        Ok(())
    }

    fn item_at<'t>(doc: &'t DocumentMut, path: &[&str]) -> Option<&'t Item> {
        let (last, parents) = path.split_last()?;
        let mut table = doc.as_table();
        for key in parents {
            table = table.get(key)?.as_table()?;
        }
        table.get(last)
    }

    fn table_at_or_create<'t>(doc: &'t mut DocumentMut, path: &[&str]) -> Option<&'t mut Table> {
        let mut table = doc.as_table_mut();
        for key in path {
            if !table.contains_key(key) {
                table.insert(key, Item::Table(Table::new()));
            }
            table = table.get_mut(key)?.as_table_mut()?;
        }
        Some(table)
    }

    /// Replace `key`'s value, transplanting the old value's decoration so
    /// same-line comments and spacing survive; create the key when missing.
    fn store(table: &mut Table, key: &str, mut value: Value) {
        let decor = table
            .get_mut(key)
            .and_then(|item| item.as_value())
            .map(|old| old.decor().clone());
        if let Some(decor) = decor {
            *value.decor_mut() = decor;
        }
        table.insert(key, Item::Value(value));
    }

    fn set_value(doc: &mut DocumentMut, path: &[&str], value: Value) {
        // An empty path or a non-table intermediate blocks the write; callers
        // use schema-valid paths, and tests cover the supported shapes.
        let Some((last, parents)) = path.split_last() else {
            return;
        };
        if let Some(table) = Self::table_at_or_create(doc, parents) {
            Self::store(table, last, value);
        }
    }

    pub fn get_bool(&self, path: &[&str]) -> Option<bool> {
        Self::item_at(&self.doc, path)?.as_bool()
    }

    pub fn get_integer(&self, path: &[&str]) -> Option<i64> {
        Self::item_at(&self.doc, path)?.as_integer()
    }

    pub fn get_float(&self, path: &[&str]) -> Option<f64> {
        Self::item_at(&self.doc, path)?.as_float()
    }

    pub fn get_string(&self, path: &[&str]) -> Option<String> {
        Self::item_at(&self.doc, path)?.as_str().map(str::to_owned)
    }

    pub fn get_integers(&self, path: &[&str]) -> Option<Vec<i64>> {
        let array = Self::item_at(&self.doc, path)?.as_value()?.as_array()?;
        array.iter().map(|value| value.as_integer()).collect()
    }

    pub fn get_strings(&self, path: &[&str]) -> Option<Vec<String>> {
        let array = Self::item_at(&self.doc, path)?.as_value()?.as_array()?;
        array
            .iter()
            .map(|value| value.as_str().map(str::to_owned))
            .collect()
    }

    pub fn get_floats(&self, path: &[&str]) -> Option<Vec<f64>> {
        let array = Self::item_at(&self.doc, path)?.as_value()?.as_array()?;
        array.iter().map(|value| value.as_float()).collect()
    }

    pub fn set_integers(&mut self, path: &[&str], values: &[i64]) {
        let mut array = Array::new();
        for value in values {
            array.push(*value);
        }
        Self::set_value(&mut self.doc, path, Value::Array(array));
    }

    pub fn set_floats(&mut self, path: &[&str], values: &[f64]) {
        let mut array = Array::new();
        for value in values {
            array.push(*value);
        }
        Self::set_value(&mut self.doc, path, Value::Array(array));
    }

    pub fn set_strings(&mut self, path: &[&str], values: &[String]) {
        let mut array = Array::new();
        for value in values {
            array.push(value.as_str());
        }
        Self::set_value(&mut self.doc, path, Value::Array(array));
    }

    /// Keys of the direct child tables of `path`, in file order; scalar and
    /// array-of-table entries are skipped.
    pub fn table_names(&self, path: &[&str]) -> Vec<String> {
        let table = if path.is_empty() {
            self.doc.as_table()
        } else if let Some(table) = Self::item_at(&self.doc, path).and_then(Item::as_table) {
            table
        } else {
            return Vec::new();
        };
        table
            .iter()
            .filter(|(_, item)| item.as_table().is_some())
            .map(|(key, _)| key.to_owned())
            .collect()
    }

    /// Delete the table at `path` (e.g. an `[output."name"]` block).
    /// Returns whether anything was removed.
    pub fn remove_table(&mut self, path: &[&str]) -> bool {
        let Some((last, parents)) = path.split_last() else {
            return false;
        };
        let mut table = self.doc.as_table_mut();
        for key in parents {
            let Some(item) = table.get_mut(key) else {
                return false;
            };
            let Some(child) = item.as_table_mut() else {
                return false;
            };
            table = child;
        }
        table.remove(last).is_some()
    }

    /// Dotted paths of every leaf item: values and arrays, tables recursed,
    /// array-of-tables recorded as a single leaf for their section name.
    pub fn value_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        walk_paths(self.doc.as_table(), &mut Vec::new(), &mut paths);
        paths
    }

    /// Every leaf outside `[keybinds]` as `(dotted path, raw TOML text)`;
    /// binds are diffed as whole entries (see `diff`), so their table is
    /// skipped here. Array-of-tables count as one leaf with a summary.
    pub fn leaf_values(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (key, item) in self.doc.as_table().iter() {
            if key == "keybinds" {
                continue;
            }
            Self::collect_leaves(key, item, &mut out);
        }
        out
    }

    fn collect_leaves(path: &str, item: &Item, out: &mut Vec<(String, String)>) {
        match item {
            Item::Table(table) => {
                for (key, child) in table.iter() {
                    Self::collect_leaves(&format!("{path}.{key}"), child, out);
                }
            }
            Item::Value(value) => out.push((path.to_owned(), value.to_string().trim().to_owned())),
            Item::ArrayOfTables(array) => {
                out.push((path.to_owned(), format!("{{ {} sections }}", array.len())))
            }
            Item::None => {}
        }
    }

    /// Overwrite the leaf at a dotted `leaf_values()` path with raw TOML
    /// value text, creating missing parents. Returns whether the text
    /// parsed and the path was writable.
    pub fn set_leaf_text(&mut self, path: &str, value_text: &str) -> bool {
        let Ok(mut mini) = format!("v = {value_text}").parse::<DocumentMut>() else {
            return false;
        };
        let Some(value) = mini
            .as_table_mut()
            .remove("v")
            .and_then(|item| item.into_value().ok())
        else {
            return false;
        };
        let components: Vec<&str> = path.split('.').collect();
        Self::set_value(&mut self.doc, &components, value);
        true
    }

    pub fn set_bool(&mut self, path: &[&str], value: bool) {
        Self::set_value(&mut self.doc, path, value.into());
    }

    pub fn set_integer(&mut self, path: &[&str], value: i64) {
        Self::set_value(&mut self.doc, path, value.into());
    }

    pub fn set_float(&mut self, path: &[&str], value: f64) {
        Self::set_value(&mut self.doc, path, value.into());
    }

    pub fn set_string(&mut self, path: &[&str], value: &str) {
        Self::set_value(&mut self.doc, path, value.into());
    }

    // --- Rules (`[[window_rule]]` / `[[layer_rule]]` arrays-of-tables) -----

    fn aot(&self, name: &str) -> Option<&ArrayOfTables> {
        self.doc.as_table().get(name)?.as_array_of_tables()
    }

    fn aot_mut(&mut self, name: &str) -> Option<&mut ArrayOfTables> {
        self.doc
            .as_table_mut()
            .get_mut(name)?
            .as_array_of_tables_mut()
    }

    /// Number of rules in the `[[name]]` array-of-tables.
    pub fn rule_count(&self, name: &str) -> usize {
        self.aot(name).map_or(0, ArrayOfTables::len)
    }

    fn rule_table(&self, name: &str, index: usize) -> Option<&Table> {
        self.aot(name)?.iter().nth(index)
    }

    fn rule_table_mut(&mut self, name: &str, index: usize) -> Option<&mut Table> {
        self.aot_mut(name)?.iter_mut().nth(index)
    }

    /// An item inside rule `index`; `key` may be dotted to reach a
    /// sub-table (`"match.app_id"`).
    fn rule_item(&self, name: &str, index: usize, key: &str) -> Option<&Item> {
        let mut table = self.rule_table(name, index)?;
        let mut parts = key.split('.').peekable();
        while let Some(part) = parts.next() {
            let item = table.get(part)?;
            if parts.peek().is_none() {
                return Some(item);
            }
            table = item.as_table()?;
        }
        None
    }

    /// Store a value inside rule `index`, creating intermediate tables for
    /// dotted keys and transplanting any existing value's decor.
    fn rule_store(&mut self, name: &str, index: usize, key: &str, mut value: Value) {
        let mut table = self.rule_table_mut(name, index).unwrap();
        let parts: Vec<&str> = key.split('.').collect();
        let (last, parents) = parts.split_last().expect("non-empty key");
        for parent in parents {
            if !table.contains_key(parent) {
                table.insert(parent, Item::Table(Table::new()));
            }
            table = match table.get_mut(parent).map(Item::as_table_mut) {
                Some(Some(child)) => child,
                _ => return,
            };
        }
        let decor = table
            .get_mut(last)
            .and_then(|item| item.as_value())
            .map(|old| old.decor().clone());
        if let Some(decor) = decor {
            *value.decor_mut() = decor;
        }
        table.insert(last, Item::Value(value));
    }

    pub fn rule_string(&self, name: &str, index: usize, key: &str) -> Option<String> {
        self.rule_item(name, index, key)?
            .as_value()?
            .as_str()
            .map(str::to_owned)
    }

    pub fn rule_bool(&self, name: &str, index: usize, key: &str) -> Option<bool> {
        self.rule_item(name, index, key)?.as_bool()
    }

    pub fn rule_float(&self, name: &str, index: usize, key: &str) -> Option<f64> {
        self.rule_item(name, index, key)?.as_value()?.as_float()
    }

    pub fn rule_integer(&self, name: &str, index: usize, key: &str) -> Option<i64> {
        self.rule_item(name, index, key)?.as_value()?.as_integer()
    }

    pub fn rule_integers(&self, name: &str, index: usize, key: &str) -> Option<Vec<i64>> {
        let array = self.rule_item(name, index, key)?.as_value()?.as_array()?;
        array.iter().map(|value| value.as_integer()).collect()
    }
    pub fn rule_strings(&self, name: &str, index: usize, key: &str) -> Option<Vec<String>> {
        let array = self.rule_item(name, index, key)?.as_value()?.as_array()?;
        array
            .iter()
            .map(|value| value.as_str().map(str::to_owned))
            .collect()
    }

    pub fn rule_set_string(&mut self, name: &str, index: usize, key: &str, value: &str) {
        self.rule_store(name, index, key, value.into());
    }

    pub fn rule_set_bool(&mut self, name: &str, index: usize, key: &str, value: bool) {
        self.rule_store(name, index, key, value.into());
    }

    pub fn rule_set_float(&mut self, name: &str, index: usize, key: &str, value: f64) {
        self.rule_store(name, index, key, value.into());
    }

    pub fn rule_set_integer(&mut self, name: &str, index: usize, key: &str, value: i64) {
        self.rule_store(name, index, key, value.into());
    }

    pub fn rule_set_integers(&mut self, name: &str, index: usize, key: &str, values: &[i64]) {
        let mut array = Array::new();
        for value in values {
            array.push(*value);
        }
        self.rule_store(name, index, key, Value::Array(array));
    }

    pub fn rule_set_strings(&mut self, name: &str, index: usize, key: &str, values: &[String]) {
        let mut array = Array::new();
        for value in values {
            array.push(value.as_str());
        }
        self.rule_store(name, index, key, Value::Array(array));
    }

    /// Inline `{ x, y, anchor }` position; anchor absent means center.
    pub fn rule_position(
        &self,
        name: &str,
        index: usize,
        key: &str,
    ) -> Option<(i64, i64, Option<String>)> {
        let inline = self
            .rule_item(name, index, key)?
            .as_value()?
            .as_inline_table()?;
        let x = inline.get("x")?.as_integer()?;
        let y = inline.get("y")?.as_integer()?;
        let anchor = inline
            .get("anchor")
            .and_then(|value| value.as_str())
            .map(str::to_owned);
        Some((x, y, anchor))
    }

    pub fn rule_set_position(
        &mut self,
        name: &str,
        index: usize,
        key: &str,
        x: i64,
        y: i64,
        anchor: Option<&str>,
    ) {
        let mut inline = InlineTable::new();
        inline.insert("x", x.into());
        inline.insert("y", y.into());
        if let Some(anchor) = anchor {
            inline.insert("anchor", anchor.into());
        }
        self.rule_store(name, index, key, Value::InlineTable(inline));
    }

    /// Append an empty `[[name]]` rule, creating the array when absent.
    pub fn add_rule(&mut self, name: &str) {
        let entry = self.doc.as_table_mut().entry(name);
        let item = entry.or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
        if let Item::ArrayOfTables(rules) = item {
            rules.push(Table::new());
        }
    }

    /// Delete rule `index`; returns whether anything was removed.
    pub fn remove_rule(&mut self, name: &str, index: usize) -> bool {
        match self.aot_mut(name) {
            Some(rules) if index < rules.len() => {
                rules.remove(index);
                true
            }
            _ => false,
        }
    }

    /// Remove one leaf key (dotted, e.g. `"match.app_id"`) from rule
    /// `index`; returns whether anything was removed.
    pub fn rule_unset(&mut self, name: &str, index: usize, key: &str) -> bool {
        let parts: Vec<&str> = key.split('.').collect();
        let Some((last, parents)) = parts.split_last() else {
            return false;
        };
        let Some(mut table) = self.rule_table_mut(name, index) else {
            return false;
        };
        for parent in parents {
            table = match table.get_mut(parent).map(Item::as_table_mut) {
                Some(Some(child)) => child,
                _ => return false,
            };
        }
        table.remove(last).is_some()
    }

    /// All `[keybinds]` entries in file order. Plain string actions have
    /// `None` extras; table-form binds surface theirs. Entries umbriel
    /// would reject (non-string values, missing `action`) are skipped.
    pub fn keybinds(&self) -> Vec<KeybindEntry> {
        let Some(table) = Self::item_at(&self.doc, &["keybinds"]).and_then(Item::as_table) else {
            return Vec::new();
        };
        table
            .iter()
            .filter_map(|(chord, item)| {
                let value = item.as_value()?;
                if let Some(action) = value.as_str() {
                    return Some(KeybindEntry {
                        chord: chord.to_owned(),
                        action: action.to_owned(),
                        repeat: None,
                        allow_when_locked: None,
                        submap: None,
                    });
                }
                let inline = value.as_inline_table()?;
                Some(KeybindEntry {
                    chord: chord.to_owned(),
                    action: inline.get("action")?.as_str()?.to_owned(),
                    repeat: inline.get("repeat").and_then(|v| v.as_bool()),
                    allow_when_locked: inline.get("allow_when_locked").and_then(|v| v.as_bool()),
                    submap: inline
                        .get("submap")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned),
                })
            })
            .collect()
    }

    /// Write one bind under `chord`. `None` extras produce the plain string
    /// form; any `Some` extra the inline-table form. Rewriting an existing
    /// chord replaces it — umbriel's own per-chord override semantics.
    pub fn set_keybind(
        &mut self,
        chord: &str,
        action: &str,
        repeat: Option<bool>,
        allow_when_locked: Option<bool>,
        submap: Option<&str>,
    ) {
        if repeat.is_none() && allow_when_locked.is_none() && submap.is_none() {
            Self::set_value(&mut self.doc, &["keybinds", chord], action.into());
            return;
        }
        let mut inline = InlineTable::new();
        inline.insert("action", action.into());
        if let Some(repeat) = repeat {
            inline.insert("repeat", repeat.into());
        }
        if let Some(locked) = allow_when_locked {
            inline.insert("allow_when_locked", locked.into());
        }
        if let Some(submap) = submap {
            inline.insert("submap", submap.into());
        }
        Self::set_value(
            &mut self.doc,
            &["keybinds", chord],
            Value::InlineTable(inline),
        );
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

fn walk_paths(table: &Table, prefix: &mut Vec<String>, out: &mut Vec<String>) {
    for (key, item) in table.iter() {
        prefix.push(key.to_owned());
        match item {
            Item::Value(_) | Item::ArrayOfTables(_) => out.push(prefix.join(".")),
            Item::Table(nested) => walk_paths(nested, prefix, out),
            Item::None => {}
        }
        prefix.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const SAMPLE: &str = "\
# Umbriel config
[general]
xwayland = true                # restart to change
show_cheatsheet = true

[animation.windows_in]
enabled = true
duration_ms = 250              # 1-10000
curve = \"easeout\"
";

    #[test]
    fn set_preserves_comments_and_formatting() {
        let mut doc = ConfigDocument::from_str(SAMPLE).unwrap();
        doc.set_integer(&["animation", "windows_in", "duration_ms"], 300);
        let expected = "\
# Umbriel config
[general]
xwayland = true                # restart to change
show_cheatsheet = true

[animation.windows_in]
enabled = true
duration_ms = 300              # 1-10000
curve = \"easeout\"
";
        assert_eq!(doc.text(), expected);
    }

    #[test]
    fn set_creates_missing_keys_and_sections() {
        let mut doc = ConfigDocument::from_str(SAMPLE).unwrap();
        // Colors are fully commented out in real configs; the path must be
        // created without disturbing anything else.
        doc.set_string(&["colors", "background"], "#141419FF");
        assert_eq!(
            doc.get_string(&["colors", "background"]).as_deref(),
            Some("#141419FF")
        );
        let text = doc.text();
        assert!(text.contains("[colors]"));
        assert!(text.contains("background = \"#141419FF\""));
        assert!(text.contains("xwayland = true"));
    }

    #[test]
    fn getters_read_typed_values() {
        let doc = ConfigDocument::from_str(SAMPLE).unwrap();
        assert_eq!(doc.get_bool(&["general", "xwayland"]), Some(true));
        assert_eq!(
            doc.get_integer(&["animation", "windows_in", "duration_ms"]),
            Some(250)
        );
        assert_eq!(
            doc.get_string(&["animation", "windows_in", "curve"])
                .as_deref(),
            Some("easeout")
        );
        assert_eq!(doc.get_float(&["general", "drag_opacity"]), None);
        assert_eq!(doc.get_bool(&["general", "missing"]), None);
    }

    #[test]
    fn modified_tracking_and_round_trip() {
        let mut doc = ConfigDocument::from_str(SAMPLE).unwrap();
        assert!(!doc.is_modified());
        doc.set_bool(&["general", "show_cheatsheet"], false);
        assert!(doc.is_modified());
        let reloaded = ConfigDocument::from_str(&doc.text()).unwrap();
        assert_eq!(
            reloaded.get_bool(&["general", "show_cheatsheet"]),
            Some(false)
        );
        assert!(!reloaded.is_modified());
    }

    #[test]
    fn save_writes_atomically_with_one_time_backup() {
        let root = std::env::temp_dir().join(format!("umbriel-document-{}", std::process::id()));
        let config = root.join("config.toml");
        let backup = root.join("config.toml.bak");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&config, SAMPLE).unwrap();

        let mut doc = ConfigDocument::load(&config).unwrap();
        doc.set_integer(&["animation", "windows_in", "duration_ms"], 400);
        doc.save(&config).unwrap();
        assert!(!doc.is_modified());
        assert_eq!(
            doc.get_integer(&["animation", "windows_in", "duration_ms"]),
            Some(400)
        );

        // Backup holds the pre-save content and is not rewritten later.
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), SAMPLE);
        let mut second = ConfigDocument::load(&config).unwrap();
        second.set_bool(&["general", "xwayland"], false);
        second.save(&config).unwrap();
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), SAMPLE);

        // No temp files left behind.
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn save_creates_missing_parent_directory() {
        let root = std::env::temp_dir().join(format!("umbriel-fresh-{}", std::process::id()));
        let path = root.join("config/umbriel/config.toml");
        let mut doc = ConfigDocument::from_str("[general]\nxwayland = true\n").unwrap();
        doc.save(&path).unwrap();
        assert!(path.exists());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn load_flags_missing_file_as_fresh_start() {
        let path =
            std::env::temp_dir().join(format!("umbriel-missing-{}.toml", std::process::id()));
        let err = ConfigDocument::load(&path).unwrap_err();
        assert!(err.is_not_found());
    }

    #[test]
    fn parse_errors_report_the_path() {
        let err = ConfigDocument::from_str("not [valid").unwrap_err();
        assert!(err.to_string().contains("invalid TOML"));
    }

    #[test]
    fn array_accessors_round_trip_and_create_quoted_tables() {
        let mut doc = ConfigDocument::from_str(SAMPLE).unwrap();
        doc.set_integers(&["output", "DP-3", "position"], &[-1920, 0]);
        assert_eq!(
            doc.get_integers(&["output", "DP-3", "position"]),
            Some(vec![-1920, 0])
        );
        doc.set_strings(
            &["output", "DP-3", "workspaces"],
            &["Games".to_owned(), "Chat".to_owned()],
        );
        assert_eq!(
            doc.get_strings(&["output", "DP-3", "workspaces"]),
            Some(vec!["Games".to_owned(), "Chat".to_owned()])
        );
        // Dashes are valid in bare TOML keys, so new tables render unquoted;
        // `[output."DP-3"]` in an existing file parses to the same table and
        // is never rewritten.
        assert!(doc.text().contains("[output.DP-3]"));
    }

    #[test]
    fn remove_table_deletes_only_that_table() {
        let text = "[output.\"DP-3\"]\nscale = 1\n\n[output.\"eDP-1\"]\nscale = 2\n";
        let mut doc = ConfigDocument::from_str(text).unwrap();
        assert!(doc.remove_table(&["output", "DP-3"]));
        assert!(!doc.text().contains("DP-3"));
        assert!(doc.text().contains("eDP-1"));
        assert_eq!(doc.get_integer(&["output", "eDP-1", "scale"]), Some(2));
        assert!(!doc.remove_table(&["output", "missing"]));
    }

    #[test]
    fn value_paths_list_leaves_with_tables_recursed() {
        let text = "[general]\nxwayland = true\n\n[output.\"DP-3\"]\nposition = [0, 0]\n\n[[window_rule]]\nmatch.app_id = \"kitty\"\n";
        let doc = ConfigDocument::from_str(text).unwrap();
        assert_eq!(
            doc.value_paths(),
            vec![
                "general.xwayland".to_owned(),
                "output.DP-3.position".to_owned(),
                // Array-of-tables collapse to their section name.
                "window_rule".to_owned(),
            ]
        );
    }

    #[test]
    fn float_arrays_round_trip() {
        let mut doc = ConfigDocument::from_str("[layout]\nwidth_presets = [0.333, 0.5]\n").unwrap();
        assert_eq!(
            doc.get_floats(&["layout", "width_presets"]),
            Some(vec![0.333, 0.5])
        );
        doc.set_floats(&["layout", "width_presets"], &[0.25, 0.5, 0.75]);
        assert!(doc.text().contains("width_presets = [0.25, 0.5, 0.75]"));
    }

    #[test]
    fn rule_accessors_read_and_write_in_place() {
        let text = "[[window_rule]]\nmatch.app_id = \"^steam$\"\ndefault_workspace = 2\n\n[[window_rule]]\ndefault_floating = true\n";
        let mut doc = ConfigDocument::from_str(text).unwrap();
        assert_eq!(doc.rule_count("window_rule"), 2);
        assert_eq!(
            doc.rule_string("window_rule", 0, "match.app_id").as_deref(),
            Some("^steam$")
        );
        assert_eq!(
            doc.rule_integer("window_rule", 0, "default_workspace"),
            Some(2)
        );
        assert_eq!(
            doc.rule_bool("window_rule", 1, "default_floating"),
            Some(true)
        );

        doc.rule_set_bool("window_rule", 1, "default_floating", false);
        assert!(doc.text().contains("default_floating = false"));
        assert!(doc.text().contains("\"^steam$\""));

        // Dotted set creates the match sub-table on a rule that lacked it.
        doc.rule_set_string("window_rule", 1, "match.title", "^Library$");
        assert_eq!(
            doc.rule_string("window_rule", 1, "match.title").as_deref(),
            Some("^Library$")
        );
    }

    #[test]
    fn add_and_remove_rules() {
        let mut doc = ConfigDocument::from_str("[[layer_rule]]\nblur = true\n").unwrap();
        doc.add_rule("layer_rule");
        assert_eq!(doc.rule_count("layer_rule"), 2);
        assert!(doc.text().contains("[[layer_rule]]"));
        assert!(doc.remove_rule("layer_rule", 1));
        assert_eq!(doc.rule_count("layer_rule"), 1);
        assert!(!doc.remove_rule("layer_rule", 5));
    }

    #[test]
    fn rule_unset_removes_single_keys() {
        let mut doc = ConfigDocument::from_str(SAMPLE).unwrap();
        doc.add_rule("window_rule");
        doc.rule_set_string("window_rule", 0, "match.app_id", "^steam");
        doc.rule_set_string("window_rule", 0, "opacity", "0.9");
        assert!(doc.rule_unset("window_rule", 0, "match.app_id"));
        assert_eq!(doc.rule_string("window_rule", 0, "match.app_id"), None);
        assert_eq!(
            doc.rule_string("window_rule", 0, "opacity").as_deref(),
            Some("0.9")
        );
        assert!(!doc.rule_unset("window_rule", 0, "match.app_id"));
    }

    #[test]
    fn keybinds_round_trip_string_and_table_forms() {
        let mut doc = ConfigDocument::from_str(SAMPLE).unwrap();
        doc.set_keybind("Mod+Q", "window-close", None, None, None);
        doc.set_keybind("Mod+R", "submap:resize", Some(false), None, None);
        let binds = doc.keybinds();
        assert_eq!(binds.len(), 2);
        assert_eq!(binds[0].chord, "Mod+Q");
        assert_eq!(binds[0].action, "window-close");
        assert_eq!(binds[0].repeat, None);
        assert_eq!(binds[1].repeat, Some(false));
        assert!(doc.text().contains("\"Mod+Q\" = \"window-close\""));
        assert!(doc.text().contains("repeat = false"));
        // Rewriting without extras converts back to the string form.
        doc.set_keybind("Mod+R", "config-reload", None, None, None);
        assert_eq!(doc.keybinds()[1].action, "config-reload");
        assert!(!doc.text().contains("repeat = false"));
    }

    #[test]
    fn rule_position_round_trips_inline() {
        let mut doc = ConfigDocument::from_str("[[window_rule]]\n").unwrap();
        doc.rule_set_position(
            "window_rule",
            0,
            "default_position",
            0,
            -40,
            Some("bottom_right"),
        );
        assert!(
            doc.text()
                .contains("default_position = { x = 0, y = -40, anchor = \"bottom_right\" }")
        );
    }

    #[test]
    fn array_getters_reject_mixed_types() {
        let mut doc = ConfigDocument::from_str(SAMPLE).unwrap();
        doc.set_strings(&["output", "DP-3", "workspaces"], &["Games".to_owned()]);
        assert_eq!(doc.get_integers(&["output", "DP-3", "workspaces"]), None);
        assert_eq!(doc.get_strings(&["general", "xwayland"]), None);
    }

    #[test]
    fn rule_string_lists_round_trip() {
        let mut doc = ConfigDocument::from_str("").unwrap();
        doc.add_rule("security_context_rule");
        doc.rule_set_string(
            "security_context_rule",
            0,
            "match.sandbox_engine",
            "org\\.flatpak",
        );
        assert_eq!(
            doc.rule_string("security_context_rule", 0, "match.sandbox_engine")
                .as_deref(),
            Some("org\\.flatpak")
        );
        assert!(
            doc.rule_strings("security_context_rule", 0, "allow_globals")
                .is_none()
        );
        doc.rule_set_strings(
            "security_context_rule",
            0,
            "allow_globals",
            &["ext_data_control_manager_v1".to_owned()],
        );
        assert_eq!(
            doc.rule_strings("security_context_rule", 0, "allow_globals"),
            Some(vec!["ext_data_control_manager_v1".to_owned()])
        );
    }

    #[test]
    fn table_names_reads_the_root_table() {
        let doc = ConfigDocument::from_str(
            "[keybinds]\n\"Mod+T\" = \"window-close\"\n\n[general]\nmod_key = \"Super\"\n",
        )
        .unwrap();
        assert_eq!(
            doc.table_names(&[]),
            vec!["keybinds".to_owned(), "general".to_owned()]
        );
        // Scalars aren't child tables.
        assert_eq!(doc.table_names(&["general"]), Vec::<String>::new());
    }
}
