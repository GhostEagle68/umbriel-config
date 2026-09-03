//! Lossless editing of an Umbriel config file.
//!
//! Built on `toml_edit` so every write preserves comments, ordering, and
//! whitespace. Paths address regular tables only
//! (`["animation", "windows_in", "duration_ms"]`); inline-table keys such as
//! window-rule `default_position` arrive with the rules editor in a later
//! phase.

use std::fs;
use std::path::{Path, PathBuf};

use toml_edit::{Array, DocumentMut, Item, Table, Value};

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

/// A loaded Umbriel config file, editable without losing comments or formatting.
#[derive(Debug)]
pub struct ConfigDocument {
    doc: DocumentMut,
    original: String,
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

    /// Whether the document differs from what was loaded.
    pub fn is_modified(&self) -> bool {
        self.doc.to_string() != self.original
    }

    /// Atomically write the document to `path`, leaving a one-time backup at
    /// `<path>.bak` holding the pre-GUI content from the first-ever save.
    pub fn save(&mut self, path: &Path) -> Result<(), ConfigError> {
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
        let Some(table) = Self::item_at(&self.doc, path).and_then(Item::as_table) else {
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
    fn array_getters_reject_mixed_types() {
        let mut doc = ConfigDocument::from_str(SAMPLE).unwrap();
        doc.set_strings(&["output", "DP-3", "workspaces"], &["Games".to_owned()]);
        assert_eq!(doc.get_integers(&["output", "DP-3", "workspaces"]), None);
        assert_eq!(doc.get_strings(&["general", "xwayland"]), None);
    }
}
