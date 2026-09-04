//! The `[include]` chain of a config: umbriel parses included files
//! first, in list order (later files override earlier ones), then the
//! main file overrides all of them. Missing or broken files are
//! non-fatal warnings, exactly like the compositor's own loader.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::document::ConfigDocument;

/// One loaded config file, 'label' is the file name for display
#[derive(Debug)]
pub struct IncludedDoc {
    pub path: PathBuf,
    pub label: String,
    pub doc: ConfigDocument,
}

/// The config's include chain plus anything that went wrong loading it.
#[derive(Debug, Default)]
pub struct IncludeChain {
    /// Loaded include documents, in list order (earlier first).
    pub docs: Vec<IncludedDoc>,
    /// Non-fatal problems: missing files, parse errors, skipped nesting.
    pub notes: Vec<String>,
}

/// Load the include chain declared by the main document. Files resolve
/// like umbriel's loader: `~` and `$VAR`/`${VAR}` expand, then relative
/// paths join the config's directory.
pub fn load_chain(main: &ConfigDocument, main_path: &Path) -> IncludeChain {
    let mut chain = IncludeChain::default();
    let Some(base_dir) = main_path.parent() else {
        return chain;
    };
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    let Some(files) = main.get_strings(&["include", "files"]) else {
        return chain;
    };
    for raw in files {
        let path = expand_path(&raw, base_dir, home.as_deref());
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| raw.clone());
        if chain.docs.iter().any(|doc| doc.path == path) {
            chain
                .notes
                .push(format!("include skipped (duplicate): {}", path.display()));
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => {
                chain
                    .notes
                    .push(format!("include not found: {}", path.display()));
                continue;
            }
        };
        let doc = match ConfigDocument::from_str(&text) {
            Ok(doc) => doc,
            Err(err) => {
                chain
                    .notes
                    .push(format!("include has errors: {}: {err}", path.display()));
                continue;
            }
        };
        if doc.get_strings(&["include", "files"]).is_some() {
            chain.notes.push(format!(
                "nested includes are not loaded: {}",
                path.display()
            ));
        }
        chain.docs.push(IncludedDoc { path, label, doc });
    }
    chain
}

/// umbriel-style path expansion: `~`/`~/` against the home directory,
/// `$VAR` and `${VAR}` from the environment, relative paths joined to the
/// including file's directory.
fn expand_path(raw: &str, base_dir: &Path, home: Option<&Path>) -> PathBuf {
    let mut expanded = raw.to_owned();
    if expanded == "~" {
        if let Some(home) = home {
            expanded = home.display().to_string();
        }
    } else if let Some(rest) = expanded.strip_prefix("~/")
        && let Some(home) = home
    {
        expanded = home.join(rest).display().to_string();
    }
    let mut budget = 8;
    while budget > 0 {
        budget -= 1;
        let Some(start) = expanded.find('$') else {
            break;
        };
        let rest = &expanded[start + 1..];
        let (name, span) = if let Some(close) = rest.strip_prefix('{').and_then(|r| r.find('}')) {
            // span covers `$`, the braces, and the name: close + 3 bytes.
            (rest[1..1 + close].to_owned(), close + 3)
        } else if rest.starts_with('{') {
            // Unterminated ${ — leave the rest of the string alone.
            break;
        } else {
            let len = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .count();
            (rest[..len].to_owned(), 1 + len)
        };
        if name.is_empty() {
            break;
        }
        let value = std::env::var(&name).unwrap_or_default();
        expanded.replace_range(start..start + span, &value);
    }
    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_path_joins_resolves_and_expands_tilde() {
        let base = Path::new("/home/tester/.config/umbriel");
        let home = Path::new("/home/tester");
        assert_eq!(
            expand_path("keybinds.toml", base, Some(home)),
            PathBuf::from("/home/tester/.config/umbriel/keybinds.toml")
        );
        assert_eq!(
            expand_path("/etc/umbriel/extra.toml", base, Some(home)),
            PathBuf::from("/etc/umbriel/extra.toml")
        );
        assert_eq!(
            expand_path("~/shared.toml", base, Some(home)),
            PathBuf::from("/home/tester/shared.toml")
        );

        // SAFETY: process-global env mutation is fine here — no other test
        // reads these uniquely named variables.
        unsafe { std::env::set_var("UMBRIEL_CONFIG_TEST_DIR", "/etc/umbriel.d") };
        assert_eq!(
            expand_path("$UMBRIEL_CONFIG_TEST_DIR/extra.toml", base, Some(home)),
            PathBuf::from("/etc/umbriel.d/extra.toml")
        );
        assert_eq!(
            expand_path("${UMBRIEL_CONFIG_TEST_DIR}/extra.toml", base, Some(home)),
            PathBuf::from("/etc/umbriel.d/extra.toml")
        );
        // Unset variables expand to nothing, like umbriel's loader.
        assert_eq!(
            expand_path("$UMBRIEL_CONFIG_TEST_UNSET/extra.toml", base, Some(home)),
            PathBuf::from("/extra.toml")
        );
    }
}
