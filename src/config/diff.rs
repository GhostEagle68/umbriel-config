//! Option-level diff for the review-changes view: one row per changed
//! option between the loaded file and the in-memory document. Each bind
//! in `[keybinds]` is one option (chord plus its extras); every other
//! leaf key is one option.

use std::collections::BTreeMap;

use super::document::{ConfigDocument, KeybindEntry};

/// One changed option. `before` is None when the edits added the option,
/// `after` None when they removed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionChange {
    /// A whole bind in `[keybinds]`.
    Bind {
        chord: String,
        before: Option<KeybindEntry>,
        after: Option<KeybindEntry>,
    },
    /// Any other leaf: dotted path and raw TOML value text.
    Leaf {
        path: String,
        before: Option<String>,
        after: Option<String>,
    },
}

impl OptionChange {
    /// Text for the option column.
    pub fn option_label(&self) -> String {
        match self {
            Self::Bind { chord, .. } => chord.clone(),
            Self::Leaf { path, .. } => path.clone(),
        }
    }
}

/// Compare two documents and list what changed, sorted by option label.
pub fn option_changes(old: &ConfigDocument, new: &ConfigDocument) -> Vec<OptionChange> {
    let mut changes = Vec::new();

    let old_binds: BTreeMap<String, KeybindEntry> = old
        .keybinds()
        .into_iter()
        .map(|bind| (bind.chord.to_ascii_lowercase(), bind))
        .collect();
    let new_binds: BTreeMap<String, KeybindEntry> = new
        .keybinds()
        .into_iter()
        .map(|bind| (bind.chord.to_ascii_lowercase(), bind))
        .collect();
    for (key, before) in &old_binds {
        match new_binds.get(key) {
            Some(after) if after != before => changes.push(OptionChange::Bind {
                chord: before.chord.clone(),
                before: Some(before.clone()),
                after: Some(after.clone()),
            }),
            None => changes.push(OptionChange::Bind {
                chord: before.chord.clone(),
                before: Some(before.clone()),
                after: None,
            }),
            Some(_) => {}
        }
    }
    for (key, after) in &new_binds {
        if !old_binds.contains_key(key) {
            changes.push(OptionChange::Bind {
                chord: after.chord.clone(),
                before: None,
                after: Some(after.clone()),
            });
        }
    }

    let old_leaves: BTreeMap<String, String> = old.leaf_values().into_iter().collect();
    let new_leaves: BTreeMap<String, String> = new.leaf_values().into_iter().collect();
    for (path, before) in &old_leaves {
        match new_leaves.get(path) {
            Some(after) if after != before => changes.push(OptionChange::Leaf {
                path: path.clone(),
                before: Some(before.clone()),
                after: Some(after.clone()),
            }),
            None => changes.push(OptionChange::Leaf {
                path: path.clone(),
                before: Some(before.clone()),
                after: None,
            }),
            Some(_) => {}
        }
    }
    for (path, after) in &new_leaves {
        if !old_leaves.contains_key(path) {
            changes.push(OptionChange::Leaf {
                path: path.clone(),
                before: None,
                after: Some(after.clone()),
            });
        }
    }

    changes.sort_by(|a, b| {
        a.option_label()
            .to_lowercase()
            .cmp(&b.option_label().to_lowercase())
    });
    changes
}

/// `action` plus short markers for the non-default extras.
pub fn bind_text(entry: &KeybindEntry) -> String {
    let mut text = entry.action.clone();
    if entry.repeat == Some(false) {
        text.push_str(" ·no-repeat");
    }
    if entry.allow_when_locked == Some(true) {
        text.push_str(" ·locked");
    }
    if let Some(submap) = &entry.submap {
        text.push_str(&format!(" ·submap {submap}"));
    }
    text
}

/// Raw TOML value text for display: strings lose their quotes.
pub fn leaf_display(text: &str) -> String {
    let trimmed = text.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(trimmed)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn changed_added_and_removed_binds_and_leaves_are_listed() {
        let old = ConfigDocument::from_str(
            "[keybinds]\n\"Mod+T\" = \"window-close\"\n\"Mod+X\" = \"window-close\"\n\n[general]\nmod_key = \"Super\"\n",
        )
        .unwrap();
        let new = ConfigDocument::from_str(
            "[keybinds]\n\"Mod+T\" = \"config-reload\"\n\"Mod+N\" = \"window-close\"\n\n[general]\nmod_key = \"Alt\"\nextra = true\n",
        )
        .unwrap();
        let changes = option_changes(&old, &new);
        assert_eq!(changes.len(), 5);
        let mut labels: Vec<String> = changes.iter().map(|c| c.option_label()).collect();
        labels.sort();
        assert_eq!(
            labels,
            [
                "Mod+N",
                "Mod+T",
                "Mod+X",
                "general.extra",
                "general.mod_key"
            ]
        );

        let changed = changes
            .iter()
            .find(|c| c.option_label() == "Mod+T")
            .unwrap();
        let OptionChange::Bind { before, after, .. } = changed else {
            panic!("bind expected");
        };
        assert_eq!(before.as_ref().unwrap().action, "window-close");
        assert_eq!(after.as_ref().unwrap().action, "config-reload");

        let removed = changes
            .iter()
            .find(|c| c.option_label() == "Mod+X")
            .unwrap();
        let OptionChange::Bind { before, after, .. } = removed else {
            panic!("bind expected");
        };
        assert!(before.is_some() && after.is_none());

        let added = changes
            .iter()
            .find(|c| c.option_label() == "general.extra")
            .unwrap();
        let OptionChange::Leaf { before, after, .. } = added else {
            panic!("leaf expected");
        };
        assert!(before.is_none());
        assert_eq!(leaf_display(after.as_deref().unwrap()), "true");
    }

    #[test]
    fn identical_content_with_different_decor_is_no_change() {
        let old =
            ConfigDocument::from_str("[general]\nmod_key = \"Super\"\nflag = true\n").unwrap();
        assert!(option_changes(&old, &old).is_empty());
        // Same content, different key spacing and value padding.
        let new =
            ConfigDocument::from_str("[general]\nmod_key   =   \"Super\"\nflag = true\n").unwrap();
        assert!(option_changes(&old, &new).is_empty());
    }

    #[test]
    fn set_leaf_text_round_trips_scalars() {
        let mut doc = ConfigDocument::from_str(
            "[general]\nmod_key = \"Super\"\n\n[keybinds]\n\"Mod+T\" = \"window-close\"\n",
        )
        .unwrap();
        assert!(doc.set_leaf_text("general.mod_key", "\"Alt\""));
        assert_eq!(
            doc.get_string(&["general", "mod_key"]).as_deref(),
            Some("Alt")
        );
        assert!(doc.set_leaf_text("general.flag", "true"));
        assert_eq!(doc.get_bool(&["general", "flag"]), Some(true));
        // Not TOML value text: rejected, document untouched.
        assert!(!doc.set_leaf_text("general.mod_key", "not toml }{"));
        assert_eq!(
            doc.get_string(&["general", "mod_key"]).as_deref(),
            Some("Alt")
        );
    }
}
