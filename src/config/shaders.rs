//! Shader discovery for umbriel's custom animation shaders. Every
//! `animation.<event>.shader` key holds a file path (never inline GLSL);
//! this module scans the places shaders live, validates them by
//! umbriel's rules, and reads the current per-event assignments.

use super::document::ConfigDocument;
use std::path::{Path, PathBuf};

/// The animation events umbriel's docs list as shader-capable, in
/// display order.
pub const EVENTS: &[&str] = &[
    "windows_in",
    "windows_out",
    "windows_move",
    "workspaces",
    "overview",
    "scratchpad",
    "border",
    "dim_unfocused",
    "layers",
];

/// Umbriel's limits for a usable shader file (docs/user/animation.md).
const MAX_SIZE: u64 = 256 * 1024;

/// One discovered shader file.
pub struct ShaderEntry {
    /// File stem, e.g. `reveal`.
    pub name: String,
    /// Absolute path on disk.
    pub path: PathBuf,
    /// How the value is written into a config: relative for files under
    /// the config directory (umbriel resolves relative paths from the
    /// declaring file's directory), absolute otherwise.
    pub value: String,
    /// Human label for dropdowns and cards.
    pub label: String,
    /// Where it was found.
    pub source: Source,
    /// Why umbriel would reject the file (missing, empty, too large,
    /// NUL bytes), if it would.
    pub invalid: Option<String>,
    /// First descriptive line of the effect's README, when present.
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Bundled with umbriel (`<data dir>/umbriel/shaders/`).
    Bundled,
    /// Under the config directory's `shaders/`.
    ConfigDir,
    /// The community repository clone (`shaders/community/…`).
    Community,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Bundled => "bundled with umbriel",
            Source::ConfigDir => "in your config directory",
            Source::Community => "community shaders",
        }
    }
}

/// Scan the well-known shader locations: `<config dir>/shaders/**`
/// (which contains the community clone) and `<data dir>/umbriel/shaders`
/// for every XDG data dir. Deduplicated by path, config shaders first.
pub fn scan(config_dir: &Path, data_dirs: &[PathBuf]) -> Vec<ShaderEntry> {
    let mut entries: Vec<ShaderEntry> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();

    let push =
        |path: PathBuf, source: Source, entries: &mut Vec<ShaderEntry>, seen: &mut Vec<PathBuf>| {
            if seen.contains(&path) {
                return;
            }
            seen.push(path.clone());
            entries.push(entry_for(path, source, config_dir));
        };

    let config_shaders = config_dir.join("shaders");
    for path in glsl_files(&config_shaders, 0) {
        let source = if path.starts_with(config_shaders.join("community")) {
            Source::Community
        } else {
            Source::ConfigDir
        };
        push(path, source, &mut entries, &mut seen);
    }
    for data_dir in data_dirs {
        for path in glsl_files(&data_dir.join("umbriel").join("shaders"), 0) {
            push(path, Source::Bundled, &mut entries, &mut seen);
        }
    }
    entries
}

/// Recursively collect `*.glsl` files, depth-limited, skipping hidden
/// directories — one small walk instead of a new dependency.
fn glsl_files(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if depth > 4 {
        return out;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for item in read.flatten() {
        let path = item.path();
        let Ok(file_type) = item.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if !item.file_name().to_string_lossy().starts_with('.') {
                out.extend(glsl_files(&path, depth + 1));
            }
        } else if path.extension().is_some_and(|ext| ext == "glsl") {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn entry_for(path: PathBuf, source: Source, config_dir: &Path) -> ShaderEntry {
    let mut name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();
    // Community effects are all `shader.glsl` inside their effect
    // folder — the folder carries the name.
    if name == "shader"
        && let Some(parent) = path.parent().and_then(|dir| dir.file_name())
    {
        name = parent.to_string_lossy().to_string();
    }
    let value = match path.strip_prefix(config_dir) {
        Ok(relative) => relative.to_string_lossy().to_string(),
        Err(_) => path.to_string_lossy().to_string(),
    };
    let invalid = validate(&path);
    let description = readme_description(&path);
    ShaderEntry {
        label: format!("{name} ({})", source.label()),
        name,
        value,
        path,
        source,
        invalid,
        description,
    }
}

/// umbriel's acceptance rules: a regular, nonblank GLSL file without
/// NUL bytes, at most 256 KiB.
fn validate(path: &Path) -> Option<String> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Some("file not found".to_owned());
    };
    if !meta.is_file() {
        return Some("not a regular file".to_owned());
    }
    if meta.len() > MAX_SIZE {
        return Some("larger than 256 KiB".to_owned());
    }
    let Ok(text) = std::fs::read(path) else {
        return Some("could not be read".to_owned());
    };
    if text.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Some("file is empty".to_owned());
    }
    if text.contains(&0) {
        return Some("contains NUL bytes".to_owned());
    }
    None
}

/// Community effects ship a README whose prose describes the effect;
/// its first non-heading, non-empty line makes a good one-liner.
fn readme_description(shader_path: &Path) -> String {
    let Some(dir) = shader_path.parent() else {
        return String::new();
    };
    let Ok(text) = std::fs::read_to_string(dir.join("README.md")) else {
        return String::new();
    };
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or_default()
        .to_owned()
}

/// The current shader assignment for one event: the value from the
/// winning document (main config overrides includes) plus that
/// document's index, or `None` when unset (built-in animation).
pub fn current_assignment(docs: &[&ConfigDocument], event: &str) -> Option<(String, usize)> {
    let mut found = None;
    for (index, doc) in docs.iter().enumerate() {
        if let Some(value) = doc.get_string(&["animation", event, "shader"])
            && !value.is_empty()
        {
            found = Some((value, index));
        }
    }
    found
}

/// The include gap: a `shaders.toml` sits next to the main config, but
/// no include directive names it — umbriel never reads the file. Returns
/// the entry to append (`shaders.toml`) when so.
pub fn missing_include(main: &ConfigDocument, main_path: &Path) -> Option<String> {
    const FILE: &str = "shaders.toml";
    let target = main_path.parent()?.join(FILE);
    if !target.is_file() {
        return None;
    }
    if super::includes::listed_paths(main, main_path).contains(&target) {
        return None;
    }
    Some(FILE.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const GLSL: &[u8] = b"vec4 animation(vec2 uv) { return umbriel_sample(uv); }\n";

    fn write(path: &Path, contents: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn scan_finds_config_and_bundled_shaders_with_sources() {
        let base = std::env::temp_dir().join(format!("umbriel-shaders-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let config_dir = base.join("config");
        let data_dir = base.join("data");
        write(
            &config_dir.join("shaders/community/animation/example/shader.glsl"),
            GLSL,
        );
        write(&config_dir.join("shaders/reveal.glsl"), GLSL);
        write(&data_dir.join("umbriel/shaders/squash.glsl"), GLSL);
        write(
            &config_dir.join("shaders/community/animation/example/README.md"),
            b"# Example\n\nMakes the window shimmer.\n",
        );

        let entries = scan(&config_dir, &[data_dir]);
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        // Sorted by path: the community clone sorts before reveal.glsl.
        assert_eq!(names, vec!["example", "reveal", "squash"]);
        assert_eq!(entries[0].source, Source::Community);
        assert_eq!(entries[1].source, Source::ConfigDir);
        assert_eq!(entries[2].source, Source::Bundled);
        assert!(entries.iter().all(|entry| entry.invalid.is_none()));
        assert_eq!(entries[0].description, "Makes the window shimmer.");
        // Config-dir values are relative to the config dir; bundled are
        // absolute, matching umbriel's path resolution.
        assert_eq!(entries[1].value, "shaders/reveal.glsl");
        assert!(entries[2].value.starts_with('/'));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn validation_flags_umbriels_rejection_rules() {
        let base = std::env::temp_dir().join(format!("umbriel-shaderval-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let config_dir = base.join("config");
        write(&config_dir.join("shaders/empty.glsl"), b"  \n\t");
        write(&config_dir.join("shaders/broken.glsl"), b"no\x00glsl");

        let entries = scan(&config_dir, &[]);
        let invalid: Vec<Option<&str>> = entries
            .iter()
            .map(|entry| entry.invalid.as_deref())
            .collect();
        // Sorted by path: broken.glsl sorts before empty.glsl.
        assert_eq!(
            invalid,
            vec![Some("contains NUL bytes"), Some("file is empty")]
        );
        assert_eq!(
            validate(&config_dir.join("shaders/absent.glsl")).as_deref(),
            Some("file not found")
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn missing_include_flags_a_dormant_shaders_toml() {
        let base = std::env::temp_dir().join(format!("umbriel-shaderinc-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let dir = base.join("config");
        std::fs::create_dir_all(&dir).unwrap();
        let main_path = dir.join("config.toml");
        std::fs::write(
            dir.join("shaders.toml"),
            "[animation.windows_in]\nshader = \"shaders/reveal.glsl\"\n",
        )
        .unwrap();

        let dormant = ConfigDocument::from_str("[general]\nxwayland = true\n").unwrap();
        assert_eq!(
            missing_include(&dormant, &main_path).as_deref(),
            Some("shaders.toml")
        );

        let listed =
            ConfigDocument::from_str("[include]\nfiles = [\"keybinds.toml\", \"shaders.toml\"]\n")
                .unwrap();
        assert_eq!(missing_include(&listed, &main_path), None);

        // Nothing to fix when the file doesn't exist at all.
        std::fs::remove_file(dir.join("shaders.toml")).unwrap();
        assert_eq!(missing_include(&listed, &main_path), None);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn current_assignment_takes_the_winning_document() {
        let include =
            ConfigDocument::from_str("[animation.windows_in]\nshader = \"shaders/old.glsl\"\n")
                .unwrap();
        let main = ConfigDocument::from_str(
            "[animation.windows_out]\nshader = \"/usr/share/umbriel/shaders/reveal.glsl\"\n",
        )
        .unwrap();
        let docs = [&include, &main];
        assert_eq!(
            current_assignment(&docs, "windows_in"),
            Some(("shaders/old.glsl".to_owned(), 0))
        );
        assert_eq!(
            current_assignment(&docs, "windows_out"),
            Some(("/usr/share/umbriel/shaders/reveal.glsl".to_owned(), 1))
        );
        assert_eq!(current_assignment(&docs, "workspaces"), None);
    }
}
