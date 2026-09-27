//! Umbriel's animation effects. An event runs a shader through a named
//! preset: `[animation.<event>] effect = "<name>"` selects
//! `[effects.preset.<name>]`, whose `shader` is a GLSL file relative to
//! the TOML file defining it; the preset's file must be included. This
//! module scans the places shaders live, validates them by umbriel's
//! rules, maps shaders to their presets, and reads the per-event
//! assignments.

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

/// Umbriel's limits for a usable shader file (docs/user/effects.md).
const MAX_SIZE: u64 = 256 * 1024;

/// One discovered shader file.
pub struct ShaderEntry {
    /// File stem, e.g. `reveal`.
    pub name: String,
    /// Absolute path on disk.
    pub path: PathBuf,
    /// The preset an event selects to run this shader (`effect = "…"`).
    pub preset: String,
    /// The TOML file defining that preset; see [`preset_file_for`]. It
    /// may not exist yet — assigning the shader writes it.
    pub preset_file: PathBuf,
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
    /// Bundled with umbriel (`<data dir>/umbriel/effects/animation/`).
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
/// (which contains the community clone) and umbriel's bundled animation
/// presets, `<data dir>/umbriel/effects/animation/*/effect.toml`, for
/// every XDG data dir. Deduplicated by path, config shaders first.
pub fn scan(config_dir: &Path, data_dirs: &[PathBuf]) -> Vec<ShaderEntry> {
    let mut entries: Vec<ShaderEntry> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();

    let push =
        |path: PathBuf, source: Source, entries: &mut Vec<ShaderEntry>, seen: &mut Vec<PathBuf>| {
            if seen.contains(&path) {
                return;
            }
            seen.push(path.clone());
            entries.push(entry_for(path, source));
        };

    let config_shaders = config_dir.join("shaders");
    for path in glsl_files(&config_shaders) {
        let source = if path.starts_with(config_shaders.join("community")) {
            Source::Community
        } else {
            Source::ConfigDir
        };
        push(path, source, &mut entries, &mut seen);
    }
    for data_dir in data_dirs {
        for path in bundled_shaders(&data_dir.join("umbriel/effects/animation")) {
            push(path, Source::Bundled, &mut entries, &mut seen);
        }
    }
    entries
}

/// The shader of every bundled animation preset under `dir`, in name
/// order. Presets of other kinds, or without a shader, are skipped.
fn bundled_shaders(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = read
        .flatten()
        .filter_map(|item| {
            let preset_file = item.path().join("effect.toml");
            let (_, shader) = read_preset(&preset_file, "animation")?;
            Some(resolve(&shader, &preset_file))
        })
        .collect();
    paths.sort();
    paths
}

/// The preset of `kind` a preset file defines: its name and `shader`
/// value, or `None` for a missing file or one without such a preset.
fn read_preset(preset_file: &Path, kind: &str) -> Option<(String, String)> {
    let doc: ConfigDocument = std::fs::read_to_string(preset_file).ok()?.parse().ok()?;
    doc.table_names(&["effects", "preset"])
        .into_iter()
        .find_map(|name| {
            let found = doc.get_string(&["effects", "preset", &name, "kind"])?;
            let shader = doc.get_string(&["effects", "preset", &name, "shader"])?;
            (found == kind).then_some((name, shader))
        })
}

/// The presets of `kind` (`border`, `window`, …) a selector can name, in
/// order: those the loaded files define, then umbriel's bundled ones
/// (`<data dir>/umbriel/effects/<kind>/*/effect.toml`) with the file to
/// include for each.
pub fn presets(
    docs: &[&ConfigDocument],
    data_dirs: &[PathBuf],
    kind: &str,
) -> Vec<(String, Option<PathBuf>)> {
    let mut found: Vec<(String, Option<PathBuf>)> = Vec::new();
    for doc in docs {
        for name in doc.table_names(&["effects", "preset"]) {
            let defined = doc.get_string(&["effects", "preset", &name, "kind"]);
            if defined.as_deref() == Some(kind) && !found.iter().any(|(known, _)| *known == name) {
                found.push((name, None));
            }
        }
    }
    let mut bundled: Vec<(String, Option<PathBuf>)> = data_dirs
        .iter()
        .filter_map(|dir| std::fs::read_dir(dir.join("umbriel/effects").join(kind)).ok())
        .flat_map(|read| read.flatten())
        .filter_map(|item| {
            let preset_file = item.path().join("effect.toml");
            let (name, _) = read_preset(&preset_file, kind)?;
            Some((name, Some(preset_file)))
        })
        .collect();
    bundled.sort();
    for (name, file) in bundled {
        if !found.iter().any(|(known, _)| *known == name) {
            found.push((name, file));
        }
    }
    found
}

/// Where a shader's preset is defined: its folder's `effect.toml` when
/// there is one (umbriel's bundled layout), else a `<stem>.effect.toml`
/// beside the shader, which the app writes when the shader is assigned.
pub fn preset_file_for(shader: &Path) -> PathBuf {
    let folder = shader.with_file_name("effect.toml");
    if folder.is_file() {
        folder
    } else {
        shader.with_extension("effect.toml")
    }
}

/// The preset file the app writes for `entry`'s shader.
pub fn preset_text(entry: &ShaderEntry) -> String {
    let shader = entry
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Only the preset's own header: its parent tables stay implicit.
    let mut preset = toml_edit::Table::new();
    preset["kind"] = toml_edit::value("animation");
    preset["shader"] = toml_edit::value(shader.as_str());
    let mut presets = toml_edit::Table::new();
    presets.set_implicit(true);
    presets.insert(&entry.preset, toml_edit::Item::Table(preset));
    let mut effects = toml_edit::Table::new();
    effects.set_implicit(true);
    effects.insert("preset", toml_edit::Item::Table(presets));
    let mut doc = toml_edit::DocumentMut::new();
    doc.insert("effects", toml_edit::Item::Table(effects));
    format!(
        "# Written by umbriel-config: the preset that runs {shader}.\n\
         # Select it with [animation.<event>] effect = \"{}\".\n{doc}",
        entry.preset
    )
}

/// Write `entry`'s preset file unless it exists (bundled and community
/// presets, or one written earlier).
pub fn ensure_preset_file(entry: &ShaderEntry) -> Result<(), String> {
    if entry.preset_file.exists() {
        return Ok(());
    }
    std::fs::write(&entry.preset_file, preset_text(entry))
        .map_err(|err| format!("could not write {}: {err}", entry.preset_file.display()))
}

/// Recursively collect `*.glsl` files, depth-limited, skipping hidden
/// directories — one small walk instead of a new dependency. Symlinked
/// folders are followed (dotfile managers link whole trees), each real
/// directory at most once so a link loop can't recurse forever.
fn glsl_files(dir: &Path) -> Vec<PathBuf> {
    let mut visited = Vec::new();
    glsl_walk(dir, 0, &mut visited)
}

fn glsl_walk(dir: &Path, depth: usize, visited: &mut Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if depth > 4 {
        return out;
    }
    if let Ok(real) = dir.canonicalize() {
        if visited.contains(&real) {
            return out;
        }
        visited.push(real);
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return out;
    };
    for item in read.flatten() {
        let path = item.path();
        let Ok(file_type) = item.file_type() else {
            continue;
        };
        // A symlink counts as whatever it points at.
        let is_dir = file_type.is_dir()
            || (file_type.is_symlink() && std::fs::metadata(&path).is_ok_and(|meta| meta.is_dir()));
        if is_dir {
            if !item.file_name().to_string_lossy().starts_with('.') {
                out.extend(glsl_walk(&path, depth + 1, visited));
            }
        } else if path.extension().is_some_and(|ext| ext == "glsl") {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn entry_for(path: PathBuf, source: Source) -> ShaderEntry {
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
    let preset_file = preset_file_for(&path);
    // An existing preset file names the preset; otherwise the shader does.
    let preset =
        read_preset(&preset_file, "animation").map_or_else(|| name.clone(), |(preset, _)| preset);
    let invalid = validate(&path);
    let description = readme_description(&path);
    ShaderEntry {
        label: format!("{name} ({})", source.label()),
        name,
        preset,
        preset_file,
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

/// The preset one event selects: the name from the winning document
/// (main config overrides includes) plus that document's index, or
/// `None` when unset (built-in animation).
pub fn current_assignment(docs: &[&ConfigDocument], event: &str) -> Option<(String, usize)> {
    winning(docs, event, "effect")
}

fn winning(docs: &[&ConfigDocument], event: &str, key: &str) -> Option<(String, usize)> {
    let mut found = None;
    for (index, doc) in docs.iter().enumerate() {
        if let Some(value) = doc.get_string(&["animation", event, key])
            && !value.is_empty()
        {
            found = Some((value, index));
        }
    }
    found
}

/// Events still assigned by the old `shader = "<path>"` key, which
/// umbriel no longer reads: event, path value, and its document index.
pub fn legacy_assignments(docs: &[&ConfigDocument]) -> Vec<(&'static str, String, usize)> {
    EVENTS
        .iter()
        .filter_map(|event| {
            winning(docs, event, "shader").map(|(value, index)| (*event, value, index))
        })
        .collect()
}

/// The library entry replacing an old `shader = "<path>"` assignment
/// (`old` resolved): the same file, or for umbriel's old bundled
/// `…/umbriel/shaders/<name>.glsl` the bundled preset of that name.
pub fn legacy_target<'a>(entries: &'a [ShaderEntry], old: &Path) -> Option<&'a ShaderEntry> {
    let key = file_key(old);
    entries
        .iter()
        .find(|entry| file_key(&entry.path) == key)
        .or_else(|| {
            let stem = old.file_stem()?.to_str()?;
            old.parent()?.ends_with("umbriel/shaders").then_some(())?;
            entries
                .iter()
                .find(|entry| entry.source == Source::Bundled && entry.preset == stem)
        })
}

/// The shader file umbriel runs for preset `name`: the `shader` of the
/// last document defining `[effects.preset.<name>]`, resolved from that
/// document's folder. `paths` parallels `docs`.
pub fn preset_shader(docs: &[&ConfigDocument], paths: &[PathBuf], name: &str) -> Option<PathBuf> {
    docs.iter().zip(paths).rev().find_map(|(doc, path)| {
        let shader = doc.get_string(&["effects", "preset", name, "shader"])?;
        Some(resolve(&shader, path))
    })
}

/// The chain index of a document defining preset `name`, other than the
/// file `own` — umbriel refuses a preset defined in two files.
pub fn preset_clash(
    docs: &[&ConfigDocument],
    paths: &[PathBuf],
    name: &str,
    own: &Path,
) -> Option<usize> {
    docs.iter().zip(paths).position(|(doc, path)| {
        !same_file(path, own)
            && doc
                .table_names(&["effects", "preset"])
                .iter()
                .any(|n| n == name)
    })
}

/// Where umbriel reads a preset's shader from: absolute values as
/// written, relative ones from the declaring file's directory, then
/// lexically normalized (`src/config/effects.cpp`). There is no `~` or
/// `$VAR` expansion — umbriel takes those literally.
pub fn resolve(value: &str, declaring_file: &Path) -> PathBuf {
    let path = Path::new(value);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        declaring_file.parent().unwrap_or(Path::new("")).join(path)
    };
    normalize(&joined)
}

/// `std::filesystem::path::lexically_normal`: drop `.`, fold `..`
/// against the previous component, touch nothing on disk.
fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// A comparable identity for a path: the real path when it exists,
/// else the lexically normalized one. Compute once, compare many times.
pub fn file_key(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| normalize(path))
}

/// Whether two paths name the same file: through symlinks when both
/// exist, lexically otherwise.
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => normalize(a) == normalize(b),
    }
}

/// The value to write so `declaring_file` points at `shader`: relative
/// when the shader sits under that file's directory (the config stays
/// portable), absolute otherwise.
pub fn value_for(shader: &Path, declaring_file: &Path) -> String {
    let dir = declaring_file.parent().unwrap_or(Path::new(""));
    match shader.strip_prefix(dir) {
        Ok(relative) if !dir.as_os_str().is_empty() => relative.to_string_lossy().into_owned(),
        _ => shader.to_string_lossy().into_owned(),
    }
}

/// Why an assignment won't run, if it won't: no included file defines
/// the preset, or its shader file is missing.
pub fn assignment_problem(resolved: Option<&Path>) -> Option<&'static str> {
    match resolved {
        None => Some("no included file defines this preset"),
        Some(path) if !path.is_file() => Some("shader file not found"),
        Some(_) => None,
    }
}

/// Which document an assignment edit for `event` belongs in: the one
/// whose value currently wins, or `None` for an unset event (see
/// [`new_assignment_home`]).
pub fn assignment_home(docs: &[&ConfigDocument], event: &str) -> Option<usize> {
    current_assignment(docs, event).map(|(_, index)| index)
}

/// Where a brand-new assignment starts: the document already holding
/// the most effect assignments, where the user keeps them (a tie goes
/// to the later file, so main wins). With none anywhere, an included
/// `shaders.toml`, else main (the last doc). `file_names` parallels
/// `docs`.
pub fn new_assignment_home(docs: &[&ConfigDocument], file_names: &[&str]) -> usize {
    let main = docs.len().saturating_sub(1);
    let count = |doc: &ConfigDocument| {
        EVENTS
            .iter()
            .filter(|event| doc.get_string(&["animation", event, "effect"]).is_some())
            .count()
    };
    let busiest = (0..docs.len()).max_by_key(|index| count(docs[*index]));
    match busiest {
        Some(index) if count(docs[index]) > 0 => index,
        _ => file_names
            .iter()
            .position(|name| *name == "shaders.toml")
            .unwrap_or(main),
    }
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

// --- The editor ---------------------------------------------------------

/// Where user-created shaders live.
pub fn user_shaders_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("shaders")
}

/// Validate a user-supplied shader file name: trimmed, non-empty, no
/// path tricks, `.glsl` appended when missing. The result can only name
/// a file directly inside the shaders directory — `community/` and
/// subdirectories are unreachable by construction.
pub fn sanitize_shader_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Shader name is empty.".to_owned());
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.starts_with('.') {
        return Err(format!("Invalid shader name: {trimmed:?}"));
    }
    let mut file_name = trimmed.to_owned();
    if !file_name.ends_with(".glsl") {
        file_name.push_str(".glsl");
    }
    Ok(file_name)
}

/// Static checks on shader source, shown live while typing. umbriel
/// refuses a file outright only for the structural rules; the missing
/// `animation()` signature is a warning — umbriel falls back to the
/// built-in animation and the author may simply be mid-typing.
pub fn lint_source(code: &str) -> Vec<String> {
    let mut problems = Vec::new();
    if code.contains('\0') {
        problems.push("contains NUL bytes".to_owned());
    }
    if code.len() as u64 > MAX_SIZE {
        problems.push("larger than 256 KiB".to_owned());
    }
    if code.bytes().all(|byte| byte.is_ascii_whitespace()) {
        problems.push("shader is empty".to_owned());
    }
    if !problems.is_empty() {
        return problems;
    }
    if !declares_animation(code) {
        problems
            .push("missing \"vec4 animation(vec2 uv)\" — umbriel calls that function".to_owned());
    }
    problems
}

/// Whether the code declares `vec4 animation(`, spaced any way GLSL
/// allows (`vec4  animation (`, a line break between, ...).
fn declares_animation(code: &str) -> bool {
    let ident = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    code.match_indices("vec4").any(|(at, _)| {
        if code[..at].chars().next_back().is_some_and(ident) {
            return false;
        }
        let rest = &code[at + "vec4".len()..];
        let after_type = rest.trim_start();
        if after_type.len() == rest.len() {
            return false; // "vec4animation" is one identifier
        }
        after_type
            .strip_prefix("animation")
            .is_some_and(|rest| rest.trim_start().starts_with('('))
    })
}

/// Write shader source atomically to an exact path, creating parent
/// directories when needed. Structural lints block; signature warnings
/// do not.
pub fn write_user_shader(path: &Path, code: &str) -> Result<(), String> {
    let blockers: Vec<String> = lint_source(code)
        .into_iter()
        .filter(|problem| !problem.starts_with("missing"))
        .collect();
    if !blockers.is_empty() {
        return Err(blockers.join("; "));
    }
    let Some(dir) = path.parent() else {
        return Err("shader path has no directory".to_owned());
    };
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("could not create {}: {err}", dir.display()))?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{file_name}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, code).map_err(|err| format!("could not write: {err}"))?;
    std::fs::rename(&tmp, path).map_err(|err| {
        let _ = std::fs::remove_file(&tmp);
        format!("could not save {}: {err}", path.display())
    })
}

/// Sanitize `name` and write a new shader to `shaders/<name>`. An
/// existing file is never replaced — editing goes through
/// [`write_user_shader`] on the exact path instead.
pub fn save_user_shader(config_dir: &Path, name: &str, code: &str) -> Result<PathBuf, String> {
    let file_name = sanitize_shader_name(name)?;
    let path = user_shaders_dir(config_dir).join(&file_name);
    if path.exists() {
        let stem = file_name.trim_end_matches(".glsl");
        return Err(format!(
            "A shader named \"{stem}\" already exists. Pick another name, or use Edit on it."
        ));
    }
    write_user_shader(&path, code)?;
    Ok(path)
}

/// Rename one of the user's shaders in place (same folder, so shaders
/// in linked subfolders stay there). The new name is sanitized like a
/// new shader's; an existing file is never replaced. Returns the new
/// path, which equals `path` when the name didn't change.
pub fn rename_user_shader(
    config_dir: &Path,
    path: &Path,
    new_name: &str,
) -> Result<PathBuf, String> {
    let Ok(relative) = path.strip_prefix(user_shaders_dir(config_dir)) else {
        return Err(format!("{} is not a user shader", path.display()));
    };
    if relative.starts_with("community") {
        return Err(
            "community shaders are managed by the download — fork one to change it".to_owned(),
        );
    }
    let file_name = sanitize_shader_name(new_name)?;
    let target = path.with_file_name(&file_name);
    if target == path {
        return Ok(target);
    }
    if target.exists() {
        let stem = file_name.trim_end_matches(".glsl");
        return Err(format!(
            "A shader named \"{stem}\" already exists. Pick another name."
        ));
    }
    std::fs::rename(path, &target)
        .map_err(|err| format!("could not rename {}: {err}", path.display()))?;
    // A preset file the app wrote follows its shader, renamed with it.
    let preset_file = path.with_extension("effect.toml");
    if preset_file.is_file() {
        let _ = std::fs::remove_file(&preset_file);
        let renamed = entry_for(target.clone(), Source::ConfigDir);
        ensure_preset_file(&renamed)?;
    }
    Ok(target)
}

/// A name for a new shader in `shaders/` that no file uses yet: `base`,
/// else `base-2`, `base-3`, ...
pub fn unused_shader_name(config_dir: &Path, base: &str) -> String {
    let dir = user_shaders_dir(config_dir);
    let free = |name: &str| !dir.join(format!("{name}.glsl")).exists();
    if free(base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|name| free(name))
        .unwrap_or_else(|| base.to_owned())
}

/// Delete a user shader by path. Only files inside the shaders
/// directory qualify — the git-managed community clone is refused.
pub fn delete_user_shader(config_dir: &Path, path: &Path) -> Result<(), String> {
    let Ok(relative) = path.strip_prefix(user_shaders_dir(config_dir)) else {
        return Err(format!("{} is not a user shader", path.display()));
    };
    if relative.starts_with("community") {
        return Err(
            "community shaders are git-managed — update or remove the clone instead".to_owned(),
        );
    }
    std::fs::remove_file(path)
        .map_err(|err| format!("could not delete {}: {err}", path.display()))?;
    // The preset file the app wrote for it goes too.
    let _ = std::fs::remove_file(path.with_extension("effect.toml"));
    Ok(())
}

/// Copy the preset files the app wrote under `old` (the community
/// collection about to be replaced) into `new`, for every shader that
/// is still there. They may be included, and umbriel rejects the whole
/// config when an included file is missing.
pub fn carry_preset_files(old: &Path, new: &Path) {
    for shader in glsl_files(old) {
        let preset_file = shader.with_extension("effect.toml");
        let Ok(relative) = shader.strip_prefix(old) else {
            continue;
        };
        let target = new.join(relative).with_extension("effect.toml");
        if preset_file.is_file() && new.join(relative).is_file() && !target.exists() {
            let _ = std::fs::copy(&preset_file, &target);
        }
    }
}

/// The visual effect builder: a stack of named steps the user composes
/// and reorders, each with its own parameters. The generator emits them
/// as one shader — motion steps (which move where the window's pixels
/// are sampled) run first in list order, then color steps in list
/// order. The output is ordinary shader code the user can keep editing
/// by hand.
pub mod builder {
    /// One step's tunable parameter.
    pub struct StepParam {
        pub key: &'static str,
        pub label: &'static str,
        pub min: f64,
        pub max: f64,
        pub default: f64,
        /// Decimal places the value is written with; the parser reads
        /// numbers back in exactly this shape.
        pub decimals: usize,
    }

    /// One step kind in the palette.
    pub struct StepDef {
        pub kind: &'static str,
        pub label: &'static str,
        /// Color steps retint the sampled pixels; motion steps move
        /// where they are sampled from.
        pub color: bool,
        pub params: &'static [StepParam],
        /// The GLSL the step emits, one statement per line, unindented.
        /// `{0}`..`{2}` stand for the params by index. A template of
        /// more than one line gets its own `{ }` block, so its locals
        /// never clash when the step is stacked twice.
        pub template: &'static str,
        /// An older template this step used to emit. Saved shaders
        /// written with it still open in the builder, and the next
        /// builder change rewrites them with `template`.
        pub legacy: Option<&'static str>,
    }

    /// A step as configured by the user: values are indexed parallel to
    /// the definition's params.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct BuilderStep {
        pub kind: &'static str,
        pub params: [f64; 3],
    }

    pub const STEP_DEFS: &[StepDef] = &[
        StepDef {
            kind: "fade",
            label: "Fade",
            color: true,
            params: &[StepParam {
                key: "to",
                label: "To opacity",
                min: 0.0,
                max: 1.0,
                default: 0.0,
                decimals: 2,
            }],
            // Fully visible once the window has arrived (vis = 1).
            template: "color *= mix({0}, 1.0, vis);",
            legacy: Some("color *= mix(1.0, {0}, vis);"),
        },
        StepDef {
            kind: "glow",
            label: "Glow pulse",
            color: true,
            params: &[StepParam {
                key: "strength",
                label: "Strength",
                min: 0.0,
                max: 1.0,
                default: 0.3,
                decimals: 2,
            }],
            template: concat!(
                "float pulse = {0} * sin(3.14159265 * p);\n",
                "color = vec4(mix(color.rgb, vec3(1.0, 0.4, 0.1) * color.a, pulse), color.a);",
            ),
            legacy: None,
        },
        StepDef {
            kind: "scale",
            label: "Scale",
            color: false,
            params: &[StepParam {
                key: "from",
                label: "From size",
                min: 0.5,
                max: 1.0,
                default: 0.85,
                decimals: 2,
            }],
            template: "uv = (uv - 0.5) / mix({0}, 1.0, vis) + 0.5;",
            legacy: None,
        },
        StepDef {
            kind: "slide",
            label: "Slide",
            color: false,
            params: &[StepParam {
                key: "offset",
                label: "Offset",
                min: -0.5,
                max: 0.5,
                default: -0.3,
                decimals: 2,
            }],
            template: "uv -= vec2({0} * (1.0 - vis), 0.0);",
            legacy: None,
        },
        StepDef {
            kind: "shatter",
            label: "Shatter",
            color: false,
            params: &[
                StepParam {
                    key: "grid",
                    label: "Grid",
                    min: 2.0,
                    max: 12.0,
                    default: 6.0,
                    decimals: 0,
                },
                StepParam {
                    key: "gravity",
                    label: "Gravity",
                    min: 0.0,
                    max: 0.4,
                    default: 0.2,
                    decimals: 2,
                },
                StepParam {
                    key: "scatter",
                    label: "Scatter",
                    min: 0.0,
                    max: 0.3,
                    default: 0.1,
                    decimals: 2,
                },
            ],
            template: concat!(
                "vec2 cell_id = floor(uv * {0}.0);\n",
                "float seed = fract(sin(dot(cell_id, vec2(12.9898, 78.233)) + umbriel_random_seed.x) * 43758.5453);\n",
                // Scattered while hidden, whole once shown (vis = 1).
                "float t = clamp(((1.0 - vis) - seed * 0.5) / 0.5, 0.0, 1.0);\n",
                "uv -= vec2((seed - 0.5) * {2} * t, {1} * t * t);",
            ),
            legacy: Some(concat!(
                "vec2 cell_id = floor(uv * {0}.0);\n",
                "float seed = fract(sin(dot(cell_id, vec2(12.9898, 78.233)) + umbriel_random_seed.x) * 43758.5453);\n",
                "float t = clamp((p - seed * 0.5) / 0.5, 0.0, 1.0);\n",
                "uv -= vec2((seed - 0.5) * {2} * t, {1} * t * t);",
            )),
        },
        StepDef {
            kind: "wobble",
            label: "Wobble",
            color: false,
            params: &[StepParam {
                key: "amp",
                label: "Amplitude",
                min: 0.0,
                max: 0.06,
                default: 0.02,
                decimals: 3,
            }],
            template: "uv.y += {0} * sin(uv.x * 12.566 + p * 9.0) * (1.0 - abs(2.0 * p - 1.0));",
            legacy: None,
        },
        StepDef {
            kind: "rotate",
            label: "Rotate",
            color: false,
            params: &[StepParam {
                key: "angle",
                label: "Angle",
                min: -180.0,
                max: 180.0,
                default: 90.0,
                decimals: 0,
            }],
            template: concat!(
                "float a = radians({0}.0) * (1.0 - vis);\n",
                "vec2 d = (uv - 0.5) * vec2(umbriel_size.x / umbriel_size.y, 1.0);\n",
                "d = vec2(cos(a) * d.x - sin(a) * d.y, sin(a) * d.x + cos(a) * d.y);\n",
                "uv = d / vec2(umbriel_size.x / umbriel_size.y, 1.0) + 0.5;",
            ),
            legacy: None,
        },
        StepDef {
            kind: "swirl",
            label: "Swirl",
            color: false,
            params: &[
                StepParam {
                    key: "strength",
                    label: "Strength",
                    min: -6.0,
                    max: 6.0,
                    default: 3.0,
                    decimals: 1,
                },
                StepParam {
                    key: "radius",
                    label: "Radius",
                    min: 0.1,
                    max: 1.0,
                    default: 0.5,
                    decimals: 2,
                },
            ],
            template: concat!(
                "vec2 d = uv - 0.5;\n",
                "float a = {0} * (1.0 - vis) * max(0.0, 1.0 - length(d) / {1});\n",
                "uv = vec2(cos(a) * d.x - sin(a) * d.y, sin(a) * d.x + cos(a) * d.y) + 0.5;",
            ),
            legacy: None,
        },
        StepDef {
            kind: "ripple",
            label: "Ripple",
            color: false,
            params: &[
                StepParam {
                    key: "amp",
                    label: "Amplitude",
                    min: 0.0,
                    max: 0.05,
                    default: 0.02,
                    decimals: 3,
                },
                StepParam {
                    key: "freq",
                    label: "Frequency",
                    min: 5.0,
                    max: 60.0,
                    default: 30.0,
                    decimals: 0,
                },
            ],
            template: concat!(
                "vec2 d = uv - 0.5;\n",
                "float r = max(length(d), 0.0001);\n",
                "uv += d / r * {0} * sin(r * {1}.0 - p * 12.0) * (1.0 - vis);",
            ),
            legacy: None,
        },
        StepDef {
            kind: "pixelate",
            label: "Pixelate",
            color: false,
            params: &[StepParam {
                key: "block",
                label: "Block size",
                min: 1.0,
                max: 64.0,
                default: 24.0,
                decimals: 0,
            }],
            template: concat!(
                "float b = max(1.0, {0}.0 * (1.0 - vis));\n",
                "uv = (floor(uv * umbriel_size / b) + 0.5) * b / umbriel_size;",
            ),
            legacy: None,
        },
        StepDef {
            kind: "stretch",
            label: "Stretch",
            color: false,
            params: &[
                StepParam {
                    key: "x",
                    label: "From width",
                    min: 0.2,
                    max: 2.0,
                    default: 1.4,
                    decimals: 2,
                },
                StepParam {
                    key: "y",
                    label: "From height",
                    min: 0.2,
                    max: 2.0,
                    default: 0.6,
                    decimals: 2,
                },
            ],
            template: "uv = (uv - 0.5) / mix(vec2({0}, {1}), vec2(1.0), vis) + 0.5;",
            legacy: None,
        },
        StepDef {
            kind: "flip",
            label: "Flip",
            color: false,
            params: &[StepParam {
                key: "turn",
                label: "Turn",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                decimals: 2,
            }],
            template: "uv.x = (uv.x - 0.5) / max(0.001, cos(1.5708 * {0} * (1.0 - vis))) + 0.5;",
            legacy: None,
        },
        StepDef {
            kind: "drop",
            label: "Drop",
            color: false,
            params: &[StepParam {
                key: "offset",
                label: "Offset",
                min: -0.5,
                max: 0.5,
                default: -0.3,
                decimals: 2,
            }],
            template: "uv.y -= {0} * (1.0 - vis);",
            legacy: None,
        },
        StepDef {
            kind: "iris",
            label: "Iris",
            color: true,
            params: &[StepParam {
                key: "soft",
                label: "Softness",
                min: 0.01,
                max: 0.5,
                default: 0.15,
                decimals: 2,
            }],
            template: concat!(
                "float r = length((uv - 0.5) * vec2(umbriel_size.x / umbriel_size.y, 1.0));\n",
                "float edge = vis * (length(vec2(umbriel_size.x / umbriel_size.y, 1.0)) * 0.5 + {0});\n",
                "color *= 1.0 - smoothstep(edge - {0}, edge, r);",
            ),
            legacy: None,
        },
        StepDef {
            kind: "wipe",
            label: "Wipe",
            color: true,
            params: &[
                StepParam {
                    key: "angle",
                    label: "Angle",
                    min: 0.0,
                    max: 360.0,
                    default: 0.0,
                    decimals: 0,
                },
                StepParam {
                    key: "soft",
                    label: "Softness",
                    min: 0.01,
                    max: 0.5,
                    default: 0.1,
                    decimals: 2,
                },
            ],
            template: concat!(
                "vec2 dir = vec2(cos(radians({0}.0)), sin(radians({0}.0)));\n",
                "float t = dot(uv - 0.5, dir) / (abs(dir.x) + abs(dir.y)) + 0.5;\n",
                "color *= 1.0 - smoothstep(vis * (1.0 + {1}) - {1}, vis * (1.0 + {1}), t);",
            ),
            legacy: None,
        },
        StepDef {
            kind: "dissolve",
            label: "Dissolve",
            color: true,
            params: &[
                StepParam {
                    key: "grain",
                    label: "Grain",
                    min: 1.0,
                    max: 32.0,
                    default: 6.0,
                    decimals: 0,
                },
                StepParam {
                    key: "glow",
                    label: "Edge glow",
                    min: 0.0,
                    max: 1.0,
                    default: 0.5,
                    decimals: 2,
                },
            ],
            template: concat!(
                "float n = fract(sin(dot(floor(uv * umbriel_size / {0}.0), vec2(12.9898, 78.233)) + umbriel_random_seed.y) * 43758.5453);\n",
                "float keep = smoothstep(n - 0.08, n, vis * 1.08);\n",
                "float rim = keep * (1.0 - smoothstep(n, n + 0.08, vis * 1.08));\n",
                "color = vec4(mix(color.rgb, vec3(1.0, 0.6, 0.2) * color.a, rim * {1}), color.a) * keep;",
            ),
            legacy: None,
        },
        StepDef {
            kind: "desaturate",
            label: "Desaturate",
            color: true,
            params: &[StepParam {
                key: "amount",
                label: "Amount",
                min: 0.0,
                max: 1.0,
                default: 1.0,
                decimals: 2,
            }],
            template: concat!(
                "float g = dot(color.rgb, vec3(0.299, 0.587, 0.114));\n",
                "color.rgb = mix(color.rgb, vec3(g), {0} * (1.0 - vis));",
            ),
            legacy: None,
        },
        StepDef {
            kind: "tint",
            label: "Tint",
            color: true,
            params: &[
                StepParam {
                    key: "r",
                    label: "Red",
                    min: 0.0,
                    max: 1.0,
                    default: 0.4,
                    decimals: 2,
                },
                StepParam {
                    key: "g",
                    label: "Green",
                    min: 0.0,
                    max: 1.0,
                    default: 0.6,
                    decimals: 2,
                },
                StepParam {
                    key: "b",
                    label: "Blue",
                    min: 0.0,
                    max: 1.0,
                    default: 1.0,
                    decimals: 2,
                },
            ],
            template: "color.rgb = mix(color.rgb, vec3({0}, {1}, {2}) * color.a, 0.6 * (1.0 - vis));",
            legacy: None,
        },
        StepDef {
            kind: "trail",
            label: "Ghost trail (extra GPU cost)",
            color: true,
            params: &[StepParam {
                key: "decay",
                label: "Decay",
                min: 0.0,
                max: 0.95,
                default: 0.8,
                decimals: 2,
            }],
            template: "color = max(color, umbriel_sample_previous(uv) * {0} * (1.0 - vis));",
            legacy: None,
        },
    ];

    pub fn step_def(kind: &str) -> Option<&'static StepDef> {
        STEP_DEFS.iter().find(|def| def.kind == kind)
    }

    impl StepDef {
        /// A new step of this kind with every parameter at its default.
        pub fn default_step(&self) -> BuilderStep {
            let mut params = [0.0; 3];
            for (slot, param) in self.params.iter().enumerate() {
                params[slot] = param.default;
            }
            BuilderStep {
                kind: self.kind,
                params,
            }
        }
    }

    /// The stack new effects start with: grow in, fade in.
    pub fn default_steps() -> Vec<BuilderStep> {
        vec![
            BuilderStep {
                kind: "scale",
                params: [0.85, 0.0, 0.0],
            },
            BuilderStep {
                kind: "fade",
                params: [0.0, 0.0, 0.0],
            },
        ]
    }

    /// A template line with its `{n}` slots filled from the step's
    /// (clamped) params.
    fn fill(line: &str, def: &StepDef, step: &BuilderStep) -> String {
        segments(line)
            .map(|segment| match segment {
                Segment::Literal(text) => text.to_owned(),
                Segment::Slot(index) => {
                    let param = &def.params[index];
                    let value = step.params[index].clamp(param.min, param.max);
                    format!("{value:.*}", param.decimals)
                }
            })
            .collect()
    }

    enum Segment<'a> {
        Literal(&'a str),
        Slot(usize),
    }

    /// Split a template line into literal text and `{n}` slots.
    fn segments(line: &str) -> impl Iterator<Item = Segment<'_>> {
        let mut rest = line;
        std::iter::from_fn(move || {
            if rest.is_empty() {
                return None;
            }
            if let Some(slot) = rest
                .strip_prefix('{')
                .and_then(|after| after.get(..2))
                .filter(|slot| slot.ends_with('}'))
                .and_then(|slot| slot[..1].parse::<usize>().ok())
            {
                rest = &rest[3..];
                return Some(Segment::Slot(slot));
            }
            let end = rest[1..].find('{').map_or(rest.len(), |at| at + 1);
            let (literal, tail) = rest.split_at(end);
            rest = tail;
            Some(Segment::Literal(literal))
        })
    }

    /// Compose the stack into a full shader.
    pub fn generate_stack(steps: &[BuilderStep]) -> String {
        generate_with(steps, &[])
    }

    /// `generate_stack`, but steps flagged in `legacy` use their def's
    /// older template: how the parser proves a saved shader is builder
    /// output before the builder upgrades it.
    fn generate_with(steps: &[BuilderStep], legacy: &[bool]) -> String {
        let mut motion = String::new();
        let mut color = String::new();
        for (index, step) in steps.iter().enumerate() {
            let Some(def) = step_def(step.kind) else {
                continue;
            };
            let template = match def.legacy {
                Some(old) if legacy.get(index) == Some(&true) => old,
                _ => def.template,
            };
            let lines: Vec<String> = template.lines().map(|line| fill(line, def, step)).collect();
            let block = if lines.len() > 1 {
                let body: String = lines
                    .iter()
                    .map(|line| format!("        {line}\n"))
                    .collect();
                format!("    {{\n{body}    }}\n")
            } else {
                format!("    {}\n", lines[0])
            };
            if def.color {
                color.push_str(&block);
            } else {
                motion.push_str(&block);
            }
        }
        format!(
            "\
// Composed with umbriel-config's effect builder.
// p is the animation progress; vis runs 0 -> 1 in the window's own
// direction (opening or closing).
vec4 animation(vec2 uv) {{
    float p = umbriel_clamped_progress;
    float vis = umbriel_direction > 0.0 ? p : 1.0 - p;
{motion}
    vec4 color = umbriel_sample(uv);
{color}
    return color;
}}
"
        )
    }

    /// Read a builder stack back out of shader code, for editing a saved
    /// shader with the builder. Only code the builder itself produced
    /// qualifies: the parsed stack must regenerate the same code (line
    /// by line, ignoring indentation), so a hand edit anywhere, even in
    /// a comment, returns `None` rather than being silently dropped by
    /// the next regeneration.
    pub fn parse_stack(code: &str) -> Option<Vec<BuilderStep>> {
        let body = code
            .split_once("vec4 animation(vec2 uv) {")?
            .1
            .rsplit_once('}')?
            .0;
        let lines: Vec<&str> = normalized(body);
        // Longer templates first, so a multi-line step is never read as
        // a shorter one that happens to share its first line.
        let mut defs: Vec<&StepDef> = STEP_DEFS.iter().collect();
        defs.sort_by_key(|def| std::cmp::Reverse(def.template.lines().count()));
        // Each step with whether it matched its def's legacy template.
        let mut motion: Vec<(BuilderStep, bool)> = Vec::new();
        let mut color: Vec<(BuilderStep, bool)> = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            if matches!(
                lines[index],
                "float p = umbriel_clamped_progress;"
                    | "float vis = umbriel_direction > 0.0 ? p : 1.0 - p;"
                    | "vec4 color = umbriel_sample(uv);"
                    | "return color;"
            ) {
                index += 1;
                continue;
            }
            let variants = defs.iter().flat_map(|def| {
                std::iter::once((*def, def.template, false))
                    .chain(def.legacy.map(|old| (*def, old, true)))
            });
            let mut matched = None;
            for (def, template, legacy) in variants {
                if let Some((params, used)) = match_template(def, template, &lines[index..]) {
                    matched = Some((def, params, used, legacy));
                    break;
                }
            }
            let (def, params, used, legacy) = matched?;
            index += used;
            let step = BuilderStep {
                kind: def.kind,
                params,
            };
            if def.color {
                color.push((step, legacy));
            } else {
                motion.push((step, legacy));
            }
        }
        motion.extend(color);
        let (steps, legacy): (Vec<BuilderStep>, Vec<bool>) = motion.into_iter().unzip();
        (normalized(&generate_with(&steps, &legacy)) == normalized(code)).then_some(steps)
    }

    /// Match a def's template against the upcoming lines: the params it
    /// carries and how many lines it spans.
    fn match_template(def: &StepDef, template: &str, lines: &[&str]) -> Option<([f64; 3], usize)> {
        let template: Vec<&str> = template.lines().collect();
        let mut params = [0.0; 3];
        for (template_line, line) in template.iter().zip(lines.get(..template.len())?) {
            let mut rest = *line;
            for segment in segments(template_line) {
                match segment {
                    Segment::Literal(text) => rest = rest.strip_prefix(text)?,
                    Segment::Slot(index) => {
                        let (value, tail) = read_number(rest, def.params[index].decimals)?;
                        params[index] = value;
                        rest = tail;
                    }
                }
            }
            if !rest.is_empty() {
                return None;
            }
        }
        Some((params, template.len()))
    }

    /// A number written with exactly `decimals` places (`-0.30`, `12`),
    /// and the text after it. Exact, so `{0}.0` after a whole number
    /// still finds its literal `.0`.
    fn read_number(text: &str, decimals: usize) -> Option<(f64, &str)> {
        let sign = usize::from(text.starts_with('-'));
        let digits = text[sign..].bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let mut end = sign + digits;
        if decimals > 0 {
            let fraction = text.get(end..end + 1 + decimals)?;
            if !fraction.starts_with('.') || !fraction[1..].bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            end += 1 + decimals;
        }
        Some((text[..end].parse().ok()?, &text[end..]))
    }

    /// Code compared by content: indentation, blank lines and the lone
    /// braces scoping a step ignored — shaders saved before steps were
    /// scoped still read back.
    fn normalized(code: &str) -> Vec<&str> {
        code.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && *line != "{" && *line != "}")
            .collect()
    }

    /// The do-nothing shader: documents the contract right in the code.
    pub const SCAFFOLD: &str = "\
// umbriel calls animation() for every pixel of the animating window.
// uv runs (0,0) top-left to (1,1) bottom-right. Useful inputs:
//   umbriel_clamped_progress  0.0 -> 1.0 over the animation
//   umbriel_direction         +1 opening, -1 closing
//   umbriel_size              window size in pixels
//   umbriel_random_seed       vec4, changes per transition
//   umbriel_sample(uv)        the window's pixels at uv
vec4 animation(vec2 uv) {
    return umbriel_sample(uv);
}
";
}

/// Code-editor keys the plain text box doesn't handle: Tab indents,
/// Shift+Tab outdents, Enter keeps the line's indentation. Offsets are
/// UTF-8 byte offsets (what Slint's text input reports); every result
/// is `(text, anchor, cursor)` with the selection to restore.
pub mod code_edit {
    const INDENT: usize = 4;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Key {
        Indent,
        Outdent,
        Newline,
    }

    pub fn apply(text: &str, anchor: usize, cursor: usize, key: Key) -> (String, usize, usize) {
        let anchor = boundary(text, anchor);
        let cursor = boundary(text, cursor);
        let (start, end) = (anchor.min(cursor), anchor.max(cursor));
        let multi_line = text[start..end].contains('\n');
        match key {
            Key::Newline => {
                let indent: String = text[line_start(text, start)..]
                    .chars()
                    .take_while(|ch| *ch == ' ' || *ch == '\t')
                    .collect();
                replace(text, start, end, &format!("\n{indent}"))
            }
            // A caret or a selection inside one line: pad to the next
            // tab stop, replacing any selected text.
            Key::Indent if !multi_line => {
                let column = text[line_start(text, start)..start].chars().count();
                replace(text, start, end, &" ".repeat(INDENT - column % INDENT))
            }
            Key::Indent | Key::Outdent => shift_lines(text, anchor, cursor, key == Key::Indent),
        }
    }

    fn replace(text: &str, start: usize, end: usize, with: &str) -> (String, usize, usize) {
        let out = format!("{}{with}{}", &text[..start], &text[end..]);
        let caret = start + with.len();
        (out, caret, caret)
    }

    /// Indent or outdent every line the selection touches. A selection
    /// ending at column 0 leaves that last line alone, like editors do.
    fn shift_lines(
        text: &str,
        anchor: usize,
        cursor: usize,
        indent: bool,
    ) -> (String, usize, usize) {
        let (start, end) = (anchor.min(cursor), anchor.max(cursor));
        let first = line_start(text, start);
        let last_end = if end > start && end == line_start(text, end) {
            end - 1
        } else {
            end
        };
        // (line start in the old text, bytes added, bytes removed)
        let mut edits: Vec<(usize, usize, usize)> = Vec::new();
        let mut at = first;
        loop {
            let line_end = text[at..].find('\n').map_or(text.len(), |i| at + i);
            if indent {
                edits.push((at, INDENT, 0));
            } else {
                let line = &text[at..line_end];
                let removed = if line.starts_with('\t') {
                    1
                } else {
                    line.bytes().take(INDENT).take_while(|b| *b == b' ').count()
                };
                edits.push((at, 0, removed));
            }
            if line_end >= last_end || line_end == text.len() {
                break;
            }
            at = line_end + 1;
        }
        let mut out = String::with_capacity(text.len() + edits.len() * INDENT);
        let mut copied = 0;
        for &(line, added, removed) in &edits {
            out.push_str(&text[copied..line]);
            out.push_str(&" ".repeat(added));
            copied = line + removed;
        }
        out.push_str(&text[copied..]);
        // Shift an old offset by every edit at or before it; a position
        // inside removed whitespace lands at its line's new start.
        let map = |pos: usize| {
            let shift: isize = edits
                .iter()
                .filter(|(line, _, _)| *line <= pos)
                .map(|&(line, added, removed)| added as isize - (pos - line).min(removed) as isize)
                .sum();
            (pos as isize + shift) as usize
        };
        (out, map(anchor), map(cursor))
    }

    /// A code state: text plus selection (byte offsets).
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Snapshot {
        pub text: String,
        pub anchor: usize,
        pub cursor: usize,
    }

    /// Undo/redo for the code pane, kept here rather than in the text
    /// box: the box's own history is byte positions that go stale (and
    /// can crash) once the text is replaced from outside, which Tab,
    /// Enter and every builder change do.
    #[derive(Debug, Default)]
    pub struct History {
        undo: Vec<Snapshot>,
        redo: Vec<Snapshot>,
        current: Option<Snapshot>,
        last_typed: Option<std::time::Instant>,
    }

    /// Keystrokes closer together than this undo as one step.
    const GROUP: std::time::Duration = std::time::Duration::from_millis(800);

    impl History {
        /// Start over from `text` (the editor opened on new code).
        pub fn reset(&mut self, text: &str) {
            *self = History::default();
            self.current = Some(Snapshot {
                text: text.to_owned(),
                anchor: 0,
                cursor: 0,
            });
        }

        /// The code changed to `next`. `typed` edits close together in
        /// time merge into one undo step; other edits (Tab, Enter, a
        /// builder change) are always their own step.
        pub fn record(&mut self, next: Snapshot, typed: bool, now: std::time::Instant) {
            let Some(current) = self.current.take() else {
                self.current = Some(next);
                return;
            };
            if current.text == next.text {
                self.current = Some(next);
                return;
            }
            let merge = typed
                && self
                    .last_typed
                    .is_some_and(|last| now.duration_since(last) < GROUP);
            if !merge {
                self.undo.push(current);
            }
            self.redo.clear();
            self.last_typed = typed.then_some(now);
            self.current = Some(next);
        }

        /// Step back; the state to show, if there was one.
        pub fn undo(&mut self) -> Option<Snapshot> {
            let previous = self.undo.pop()?;
            if let Some(current) = self.current.replace(previous.clone()) {
                self.redo.push(current);
            }
            self.last_typed = None;
            Some(previous)
        }

        /// Step forward again after an undo.
        pub fn redo(&mut self) -> Option<Snapshot> {
            let next = self.redo.pop()?;
            if let Some(current) = self.current.replace(next.clone()) {
                self.undo.push(current);
            }
            self.last_typed = None;
            Some(next)
        }
    }

    fn line_start(text: &str, pos: usize) -> usize {
        text[..pos].rfind('\n').map_or(0, |i| i + 1)
    }

    fn boundary(text: &str, pos: usize) -> usize {
        let mut pos = pos.min(text.len());
        while !text.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const GLSL_STR: &str = "vec4 animation(vec2 uv) { return umbriel_sample(uv); }\n";
    const GLSL: &[u8] = GLSL_STR.as_bytes();

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
        write(
            &data_dir.join("umbriel/effects/animation/squash/shader.glsl"),
            GLSL,
        );
        write(
            &data_dir.join("umbriel/effects/animation/squash/effect.toml"),
            b"[effects.preset.squash]\nkind = \"animation\"\nshader = \"shader.glsl\"\n",
        );
        // Other kinds are not animation shaders.
        write(
            &data_dir.join("umbriel/effects/border/pulse/shader.glsl"),
            GLSL,
        );
        write(
            &config_dir.join("shaders/community/animation/example/README.md"),
            b"# Example\n\nMakes the window shimmer.\n",
        );

        let entries = scan(&config_dir, std::slice::from_ref(&data_dir));
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
        // Sorted by path: the community clone sorts before reveal.glsl.
        assert_eq!(names, vec!["example", "reveal", "squash"]);
        assert_eq!(entries[0].source, Source::Community);
        assert_eq!(entries[1].source, Source::ConfigDir);
        assert_eq!(entries[2].source, Source::Bundled);
        assert!(entries.iter().all(|entry| entry.invalid.is_none()));
        assert_eq!(entries[0].description, "Makes the window shimmer.");
        // Presets: named by the shader until a preset file exists; the
        // bundled one's comes from its effect.toml.
        assert_eq!(entries[1].preset, "reveal");
        assert_eq!(
            entries[1].preset_file,
            config_dir.join("shaders/reveal.effect.toml")
        );
        assert_eq!(entries[2].preset, "squash");
        assert_eq!(
            entries[2].preset_file,
            data_dir.join("umbriel/effects/animation/squash/effect.toml")
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn scan_follows_symlinked_folders_once() {
        let base = std::env::temp_dir().join(format!("umbriel-shaderlink-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let config_dir = base.join("config");
        let elsewhere = base.join("dotfiles/effects");
        write(&elsewhere.join("glow.glsl"), GLSL);
        std::fs::create_dir_all(config_dir.join("shaders")).unwrap();
        // A linked folder (stow-style) and a loop back to shaders/.
        std::os::unix::fs::symlink(&elsewhere, config_dir.join("shaders/mine")).unwrap();
        std::os::unix::fs::symlink(config_dir.join("shaders"), config_dir.join("shaders/loop"))
            .unwrap();

        let entries = scan(&config_dir, &[]);
        let paths: Vec<&Path> = entries.iter().map(|entry| entry.path.as_path()).collect();
        assert_eq!(paths, vec![config_dir.join("shaders/mine/glow.glsl")]);
        assert_eq!(entries[0].source, Source::ConfigDir);
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
            "[animation.windows_in]\neffect = \"reveal\"\n",
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
    fn sanitize_rejects_path_tricks_and_appends_extension() {
        assert_eq!(
            sanitize_shader_name(" my effect ").as_deref(),
            Ok("my effect.glsl")
        );
        assert_eq!(
            sanitize_shader_name("already.glsl").as_deref(),
            Ok("already.glsl"),
        );
        assert!(sanitize_shader_name("  ").is_err());
        assert!(sanitize_shader_name("../evil").is_err());
        assert!(sanitize_shader_name("a/b").is_err());
        assert!(sanitize_shader_name(".hidden").is_err());
    }

    #[test]
    fn lint_flags_structure_and_missing_signature() {
        let good = "vec4 animation(vec2 uv) { return umbriel_sample(uv); }\n";
        assert!(lint_source(good).is_empty());
        // The missing signature is a single warning, not a blocker.
        assert_eq!(lint_source("float x = 1.0;\n").len(), 1);
        assert_eq!(lint_source("   \n\t"), vec!["shader is empty"]);
        // Any GLSL spacing counts; lookalike names don't.
        for spaced in [
            "vec4  animation (vec2 uv) { return vec4(0.0); }",
            "vec4\nanimation(vec2 uv) { return vec4(0.0); }",
        ] {
            assert!(lint_source(spaced).is_empty(), "{spaced:?}");
        }
        for wrong in [
            "vec4 animations(vec2 uv) {}",
            "myvec4 animation(vec2 uv) {}",
        ] {
            assert_eq!(lint_source(wrong).len(), 1, "{wrong:?}");
        }
    }

    #[test]
    fn save_edit_and_delete_user_shaders() {
        let base = std::env::temp_dir().join(format!("umbriel-shaderedit-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let config_dir = base.join("config");
        let saved = save_user_shader(
            &config_dir,
            "my effect",
            "vec4 animation(vec2 uv) { return umbriel_sample(uv); }\n",
        )
        .unwrap();
        assert_eq!(saved, config_dir.join("shaders/my effect.glsl"));
        // A new shader never clobbers an existing one of the same name.
        let clash = save_user_shader(&config_dir, "my effect", GLSL_STR).unwrap_err();
        assert!(clash.contains("already exists"), "{clash}");
        // Editing overwrites in place: exactly one file, no temp behind.
        write_user_shader(
            &saved,
            "vec4 animation(vec2 uv) { return umbriel_sample(uv) * 0.5; }\n",
        )
        .unwrap();
        assert!(std::fs::read_to_string(&saved).unwrap().contains("* 0.5"));
        let entries: Vec<String> = std::fs::read_dir(config_dir.join("shaders"))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["my effect.glsl".to_owned()]);
        // Structural blockers refuse the write.
        assert!(save_user_shader(&config_dir, "broken", "   \n").is_err());
        // Delete works for user files …
        assert!(delete_user_shader(&config_dir, &saved).is_ok());
        // … but never for the community clone or files outside shaders/.
        let community = config_dir.join("shaders/community/x.glsl");
        write(&community, GLSL);
        assert!(delete_user_shader(&config_dir, &community).is_err());
        assert!(delete_user_shader(&config_dir, &config_dir.join("config.toml")).is_err());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn rename_moves_the_file_and_refuses_clashes() {
        let base =
            std::env::temp_dir().join(format!("umbriel-shaderrename-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let config_dir = base.join("config");
        let old = config_dir.join("shaders/old.glsl");
        write(&old, GLSL);
        write(&config_dir.join("shaders/taken.glsl"), GLSL);

        // Same name: nothing happens.
        assert_eq!(rename_user_shader(&config_dir, &old, "old").unwrap(), old);
        // A taken name or a path trick is refused, and the file stays.
        assert!(
            rename_user_shader(&config_dir, &old, "taken")
                .unwrap_err()
                .contains("already exists")
        );
        assert!(rename_user_shader(&config_dir, &old, "../evil").is_err());
        assert!(old.exists());
        // A real rename moves the file.
        let new = rename_user_shader(&config_dir, &old, "fresh").unwrap();
        assert_eq!(new, config_dir.join("shaders/fresh.glsl"));
        assert!(new.exists() && !old.exists());
        // Community shaders are never renamed.
        let community = config_dir.join("shaders/community/x/shader.glsl");
        write(&community, GLSL);
        assert!(rename_user_shader(&config_dir, &community, "mine").is_err());

        // Fork names skip ones already used.
        assert_eq!(
            unused_shader_name(&config_dir, "reveal-fork"),
            "reveal-fork"
        );
        write(&config_dir.join("shaders/reveal-fork.glsl"), GLSL);
        write(&config_dir.join("shaders/reveal-fork-2.glsl"), GLSL);
        assert_eq!(
            unused_shader_name(&config_dir, "reveal-fork"),
            "reveal-fork-3"
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn builder_generates_working_code_for_every_kind() {
        for def in builder::STEP_DEFS {
            let shader = builder::generate_stack(&[def.default_step()]);
            assert!(shader.contains("vec4 animation(vec2 uv)"));
            assert!(
                shader.contains("umbriel_sample(uv)"),
                "{} samples",
                def.label
            );
        }
        // The parameter lands in the code and moves with the slider.
        let small = builder::generate_stack(&[builder::BuilderStep {
            kind: "scale",
            params: [0.5, 0.0, 0.0],
        }]);
        let large = builder::generate_stack(&[builder::BuilderStep {
            kind: "scale",
            params: [1.0, 0.0, 0.0],
        }]);
        assert!(small.contains("mix(0.50, 1.0, vis)"));
        assert!(large.contains("mix(1.00, 1.0, vis)"));
        // Unknown kinds are skipped; out-of-range params clamp.
        let skipped = builder::generate_stack(&[builder::BuilderStep {
            kind: "nope",
            params: [0.0; 3],
        }]);
        assert!(skipped.contains("umbriel_sample(uv)"));
        let clamped = builder::generate_stack(&[builder::BuilderStep {
            kind: "shatter",
            params: [99.0, -5.0, 0.1],
        }]);
        assert!(clamped.contains("floor(uv * 12.0)"), "grid clamps to max");
        assert!(clamped.contains("0.00 * t * t"), "gravity clamps to min");
    }

    #[test]
    fn builder_indents_every_body_line() {
        for def in builder::STEP_DEFS {
            let shader = builder::generate_stack(&[def.default_step()]);
            let body = shader
                .split_once("vec4 animation(vec2 uv) {\n")
                .unwrap()
                .1
                .rsplit_once("\n}")
                .unwrap()
                .0;
            for line in body.lines().filter(|line| !line.is_empty()) {
                assert!(line.starts_with("    "), "{}: {line:?}", def.label);
            }
        }
    }

    #[test]
    fn parse_stack_round_trips_builder_output() {
        // Every kind alone, at defaults and at off-default values.
        for def in builder::STEP_DEFS {
            let step = def.default_step();
            let code = builder::generate_stack(&[step]);
            let parsed = builder::parse_stack(&code).expect(def.label);
            assert_eq!(parsed.len(), 1, "{}", def.label);
            assert_eq!(parsed[0].kind, def.kind);
            assert_eq!(parsed[0].params, step.params, "{}", def.label);
        }
        // A mixed stack comes back motion-first, which regenerates the
        // same code.
        let stack = [
            builder::BuilderStep {
                kind: "fade",
                params: [0.25, 0.0, 0.0],
            },
            builder::BuilderStep {
                kind: "shatter",
                params: [9.0, 0.35, 0.05],
            },
            builder::BuilderStep {
                kind: "glow",
                params: [0.5, 0.0, 0.0],
            },
            builder::BuilderStep {
                kind: "wobble",
                params: [0.015, 0.0, 0.0],
            },
        ];
        let code = builder::generate_stack(&stack);
        let parsed = builder::parse_stack(&code).unwrap();
        let kinds: Vec<&str> = parsed.iter().map(|step| step.kind).collect();
        assert_eq!(kinds, vec!["shatter", "wobble", "fade", "glow"]);
        assert_eq!(builder::generate_stack(&parsed), code);
        // An empty stack is still builder output.
        assert!(
            builder::parse_stack(&builder::generate_stack(&[]))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn the_same_effect_can_be_stacked_twice() {
        let shatter = builder::STEP_DEFS
            .iter()
            .find(|def| def.kind == "shatter")
            .unwrap()
            .default_step();
        let code = builder::generate_stack(&[shatter, shatter]);
        // Each copy declares its locals inside its own block, never at
        // the function's top level.
        let body = code.split_once("vec4 animation(vec2 uv) {").unwrap().1;
        let mut depth = 0;
        for line in body.lines() {
            if line.contains("vec2 cell_id") {
                assert_eq!(depth, 1, "cell_id must sit in a step block");
            }
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
        }
        // And the doubled stack still reads back.
        assert_eq!(builder::parse_stack(&code).unwrap().len(), 2);
    }

    #[test]
    fn parse_stack_reads_older_unindented_output() {
        // A file saved by earlier versions: shatter unscoped, its first
        // line at column 0. Those still open in the builder.
        let code = "\
// Composed with umbriel-config's effect builder.
// p is the animation progress; vis runs 0 -> 1 in the window's own
// direction (opening or closing).
vec4 animation(vec2 uv) {
    float p = umbriel_clamped_progress;
    float vis = umbriel_direction > 0.0 ? p : 1.0 - p;
vec2 cell_id = floor(uv * 12.0);
    float seed = fract(sin(dot(cell_id, vec2(12.9898, 78.233)) + umbriel_random_seed.x) * 43758.5453);
    float t = clamp((p - seed * 0.5) / 0.5, 0.0, 1.0);
    uv -= vec2((seed - 0.5) * 0.30 * t, 0.40 * t * t);

    vec4 color = umbriel_sample(uv);
    color *= mix(1.0, 0.25, vis);

    return color;
}
";
        // Old Fade and Shatter still read back...
        let parsed = builder::parse_stack(code).unwrap();
        let kinds: Vec<&str> = parsed.iter().map(|step| step.kind).collect();
        assert_eq!(kinds, vec!["shatter", "fade"]);
        assert_eq!(parsed[0].params, [12.0, 0.4, 0.3]);
        assert_eq!(parsed[1].params[0], 0.25);
        // ...and regenerate with the fixed direction.
        let upgraded = builder::generate_stack(&parsed);
        assert!(upgraded.contains("color *= mix(0.25, 1.0, vis);"));
        assert!(upgraded.contains("clamp(((1.0 - vis) - seed * 0.5) / 0.5"));
        assert_eq!(builder::parse_stack(&upgraded).unwrap(), parsed);
    }

    #[test]
    fn parse_stack_rejects_hand_edited_code() {
        let code = builder::generate_stack(&builder::default_steps());
        // An extra statement, a tweaked constant, an edited comment,
        // or code from elsewhere entirely.
        let extra = code.replace("return color;", "color.rgb *= 0.9;\n    return color;");
        let tweaked = code.replace("float vis", "float vis = 0.5; float unused");
        let comment = code.replace("effect builder.", "effect builder, then tuned.");
        for edited in [
            extra.as_str(),
            tweaked.as_str(),
            comment.as_str(),
            GLSL_STR,
            builder::SCAFFOLD,
        ] {
            assert!(builder::parse_stack(edited).is_none(), "{edited}");
        }
    }

    #[test]
    fn builder_motion_runs_before_color() {
        let shader = builder::generate_stack(&[
            builder::BuilderStep {
                kind: "fade",
                params: [0.5, 0.0, 0.0],
            },
            builder::BuilderStep {
                kind: "slide",
                params: [0.2, 0.0, 0.0],
            },
        ]);
        let sample = shader.find("umbriel_sample(uv)").unwrap();
        let slide = shader.find("uv -= vec2(").unwrap();
        let fade = shader.find("color *= mix(").unwrap();
        assert!(slide < sample, "motion precedes the sample");
        assert!(sample < fade, "color follows the sample");
    }

    #[test]
    fn assignment_writes_land_quoted_in_the_winning_document() {
        let mut include =
            ConfigDocument::from_str("[animation.windows_in]\neffect = \"old\"\n").unwrap();
        let main = ConfigDocument::from_str("[general]\nxwayland = true\n").unwrap();
        assert_eq!(assignment_home(&[&include, &main], "windows_in"), Some(0));
        assert_eq!(assignment_home(&[&include, &main], "windows_move"), None);
        // Names with spaces are not bare TOML: the typed write quotes them.
        include.set_string(&["animation", "windows_move", "effect"], "my effect");
        assert!(include.text().contains("effect = \"my effect\""));
        assert_eq!(
            current_assignment(&[&include, &main], "windows_move"),
            Some(("my effect".to_owned(), 0))
        );
    }

    #[test]
    fn new_assignments_start_where_the_others_live() {
        let shaders = ConfigDocument::from_str("[animation.windows_in]\neffect = \"a\"\n").unwrap();
        let keybinds = ConfigDocument::from_str("[keybinds]\n").unwrap();
        let main = ConfigDocument::from_str("[general]\nxwayland = true\n").unwrap();
        let names = ["keybinds.toml", "shaders.toml", "config.toml"];
        assert_eq!(
            new_assignment_home(&[&keybinds, &shaders, &main], &names),
            1
        );
        // No assignments anywhere: an included shaders.toml still wins.
        let empty = ConfigDocument::from_str("").unwrap();
        assert_eq!(new_assignment_home(&[&keybinds, &empty, &main], &names), 1);
        // Neither: the main config.
        assert_eq!(
            new_assignment_home(&[&keybinds, &main], &["keybinds.toml", "config.toml"]),
            1
        );
    }

    #[test]
    fn tab_pads_to_the_next_tab_stop() {
        use code_edit::{Key, apply};
        // Caret at column 0 and column 2 of "ab".
        assert_eq!(apply("ab", 0, 0, Key::Indent), ("    ab".into(), 4, 4));
        assert_eq!(apply("ab", 2, 2, Key::Indent), ("ab  ".into(), 4, 4));
        // A one-line selection is replaced by the padding.
        assert_eq!(apply("abcd", 1, 3, Key::Indent), ("a   d".into(), 4, 4));
    }

    #[test]
    fn tab_and_shift_tab_shift_every_selected_line() {
        use code_edit::{Key, apply};
        let text = "a\n  b\nc";
        // Select from inside line 1 to inside line 2.
        let (out, anchor, cursor) = apply(text, 0, 4, Key::Indent);
        assert_eq!(out, "    a\n      b\nc");
        assert_eq!((anchor, cursor), (4, 12));
        let (back, anchor, cursor) = apply(&out, anchor, cursor, Key::Outdent);
        assert_eq!(back, "a\n  b\nc");
        assert_eq!((anchor, cursor), (0, 4));
        // A selection ending at column 0 leaves that line alone.
        let (out, _, _) = apply("a\nb\n", 0, 2, Key::Indent);
        assert_eq!(out, "    a\nb\n");
        // Shift+Tab on a caret outdents its line; a tab counts as one step.
        assert_eq!(apply("      x", 7, 7, Key::Outdent), ("  x".into(), 3, 3));
        assert_eq!(apply("\tx", 1, 1, Key::Outdent), ("x".into(), 0, 0));
        // A caret inside the removed spaces lands at the line start.
        assert_eq!(apply("    x", 2, 2, Key::Outdent), ("x".into(), 0, 0));
    }

    #[test]
    fn code_history_undoes_typing_in_groups_and_other_edits_singly() {
        use code_edit::{History, Snapshot};
        use std::time::{Duration, Instant};
        let snap = |text: &str| Snapshot {
            text: text.to_owned(),
            anchor: text.len(),
            cursor: text.len(),
        };
        let t0 = Instant::now();
        let mut history = History::default();
        history.reset("a");
        // Quick typing merges into one step...
        history.record(snap("ab"), true, t0);
        history.record(snap("abc"), true, t0 + Duration::from_millis(100));
        // ...an indent (or builder change) is a step of its own.
        history.record(snap("    abc"), false, t0 + Duration::from_millis(200));
        assert_eq!(history.undo().unwrap().text, "abc");
        assert_eq!(history.undo().unwrap().text, "a");
        assert!(history.undo().is_none(), "nothing before the opened code");
        assert_eq!(history.redo().unwrap().text, "abc");
        assert_eq!(history.redo().unwrap().text, "    abc");
        assert!(history.redo().is_none());
        // A new edit after an undo drops the redo branch.
        history.undo();
        history.record(snap("abcd"), true, t0 + Duration::from_secs(5));
        assert!(history.redo().is_none());
        // Typing after a pause starts a new step.
        history.record(snap("abcde"), true, t0 + Duration::from_secs(10));
        assert_eq!(history.undo().unwrap().text, "abcd");
    }

    #[test]
    fn enter_keeps_the_line_indentation() {
        use code_edit::{Key, apply};
        assert_eq!(
            apply("    a = 1;", 10, 10, Key::Newline),
            ("    a = 1;\n    ".into(), 15, 15)
        );
        // Non-ASCII before the caret: offsets stay on char boundaries.
        let text = "  é";
        let (out, caret, _) = apply(text, text.len(), text.len(), Key::Newline);
        assert_eq!(out, "  é\n  ");
        assert_eq!(caret, out.len());
    }

    #[test]
    fn shader_values_resolve_like_umbriel() {
        let main = Path::new("/home/u/.config/umbriel/config.toml");
        let nested = Path::new("/home/u/.config/umbriel/conf.d/shaders.toml");
        let target = PathBuf::from("/home/u/.config/umbriel/shaders/test.glsl");
        // Relative to the declaring file, with ./ and .. folded.
        assert_eq!(resolve("shaders/test.glsl", main), target);
        assert_eq!(resolve("./shaders/test.glsl", main), target);
        assert_eq!(resolve("../shaders/test.glsl", nested), target);
        assert_eq!(
            resolve("/home/u/.config/umbriel/shaders/test.glsl", nested),
            target
        );
        // No ~ expansion: it stays a literal (relative) component.
        assert_eq!(
            resolve("~/x.glsl", main),
            PathBuf::from("/home/u/.config/umbriel/~/x.glsl")
        );
        // Writing: relative under the file's folder, absolute elsewhere.
        assert_eq!(value_for(&target, main), "shaders/test.glsl");
        assert_eq!(value_for(&target, nested), target.to_string_lossy());
        let bundled = Path::new("/usr/share/umbriel/shaders/reveal.glsl");
        assert_eq!(value_for(bundled, main), bundled.to_string_lossy());
        // Round trip: what we write resolves back to the shader.
        assert_eq!(resolve(&value_for(&target, nested), nested), target);
    }

    #[test]
    fn assignment_problems_name_the_cause() {
        let base = std::env::temp_dir().join(format!("umbriel-shaderpath-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let file = base.join("shaders/a.glsl");
        write(&file, GLSL);
        assert_eq!(assignment_problem(Some(&file)), None);
        assert_eq!(
            assignment_problem(Some(&base.join("shaders/b.glsl"))),
            Some("shader file not found")
        );
        assert_eq!(
            assignment_problem(None),
            Some("no included file defines this preset")
        );
        // Different spellings of one file match; different files don't.
        assert!(same_file(&file, &base.join("shaders/./a.glsl")));
        assert!(!same_file(&file, &base.join("shaders/b.glsl")));
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn current_assignment_takes_the_winning_document() {
        let include = ConfigDocument::from_str(
            "[animation.windows_in]\neffect = \"old\"\n\n\
             [animation.windows_move]\nshader = \"shaders/wobble.glsl\"\n",
        )
        .unwrap();
        let main =
            ConfigDocument::from_str("[animation.windows_out]\neffect = \"reveal\"\n").unwrap();
        let docs = [&include, &main];
        assert_eq!(
            current_assignment(&docs, "windows_in"),
            Some(("old".to_owned(), 0))
        );
        assert_eq!(
            current_assignment(&docs, "windows_out"),
            Some(("reveal".to_owned(), 1))
        );
        // The old key is no assignment any more, only a legacy one.
        assert_eq!(current_assignment(&docs, "windows_move"), None);
        assert_eq!(
            legacy_assignments(&docs),
            vec![("windows_move", "shaders/wobble.glsl".to_owned(), 0)]
        );
        assert_eq!(current_assignment(&docs, "workspaces"), None);
    }

    #[test]
    fn presets_resolve_from_the_file_defining_them() {
        let preset = ConfigDocument::from_str(
            "[effects.preset.reveal]\nkind = \"animation\"\nshader = \"shader.glsl\"\n",
        )
        .unwrap();
        let main =
            ConfigDocument::from_str("[animation.windows_in]\neffect = \"reveal\"\n").unwrap();
        let paths = [
            PathBuf::from("/usr/share/umbriel/effects/animation/reveal/effect.toml"),
            PathBuf::from("/home/u/.config/umbriel/config.toml"),
        ];
        let docs = [&preset, &main];
        assert_eq!(
            preset_shader(&docs, &paths, "reveal"),
            Some(PathBuf::from(
                "/usr/share/umbriel/effects/animation/reveal/shader.glsl"
            ))
        );
        assert_eq!(preset_shader(&docs, &paths, "squash"), None);
        // Defining it again anywhere else would clash; its own file doesn't.
        assert_eq!(preset_clash(&docs, &paths, "reveal", &paths[0]), None);
        assert_eq!(
            preset_clash(&docs, &paths, "reveal", Path::new("/elsewhere.toml")),
            Some(0)
        );
    }

    #[test]
    fn selectors_offer_loaded_presets_then_bundled_ones() {
        let base = std::env::temp_dir().join(format!("umbriel-presets-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        for (kind, name) in [
            ("border", "pulse"),
            ("border", "mine"),
            ("window", "scanlines"),
        ] {
            write(
                &base.join(format!("umbriel/effects/{kind}/{name}/effect.toml")),
                format!("[effects.preset.{name}]\nkind = \"{kind}\"\nshader = \"shader.glsl\"\n")
                    .as_bytes(),
            );
        }
        let own = ConfigDocument::from_str(
            "[effects.preset.mine]\nkind = \"border\"\nshader = \"mine.glsl\"\n\n\
             [effects.preset.tint]\nkind = \"window\"\nshader = \"tint.glsl\"\n",
        )
        .unwrap();
        let found = presets(&[&own], std::slice::from_ref(&base), "border");
        // Yours needs no include; a bundled one of the same name is hidden.
        assert_eq!(
            found,
            vec![
                ("mine".to_owned(), None),
                (
                    "pulse".to_owned(),
                    Some(base.join("umbriel/effects/border/pulse/effect.toml"))
                ),
            ]
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn a_community_update_keeps_preset_files_for_surviving_shaders() {
        let base = std::env::temp_dir().join(format!("umbriel-carry-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let (old, new) = (base.join("community"), base.join(".staging"));
        for name in ["kept", "gone"] {
            write(&old.join(format!("animation/{name}/shader.glsl")), GLSL);
            write(
                &old.join(format!("animation/{name}/shader.effect.toml")),
                b"# preset\n",
            );
        }
        write(&new.join("animation/kept/shader.glsl"), GLSL);
        carry_preset_files(&old, &new);
        assert!(new.join("animation/kept/shader.effect.toml").is_file());
        assert!(!new.join("animation/gone").exists());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn preset_files_are_written_renamed_and_deleted_with_their_shader() {
        let base = std::env::temp_dir().join(format!("umbriel-presetfile-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let config_dir = base.join("config");
        let shader = config_dir.join("shaders/wobble.glsl");
        write(&shader, GLSL);
        let entry = entry_for(shader.clone(), Source::ConfigDir);
        ensure_preset_file(&entry).unwrap();
        // Just the preset's own table, no empty parent headers.
        let text = std::fs::read_to_string(&entry.preset_file).unwrap();
        assert!(
            text.ends_with(
                "[effects.preset.wobble]\nkind = \"animation\"\nshader = \"wobble.glsl\"\n"
            ),
            "{text}"
        );
        // What the app writes is the preset umbriel reads back.
        assert_eq!(
            read_preset(&entry.preset_file, "animation"),
            Some(("wobble".to_owned(), "wobble.glsl".to_owned()))
        );
        let renamed = rename_user_shader(&config_dir, &shader, "jelly").unwrap();
        assert!(!entry.preset_file.exists());
        assert_eq!(
            read_preset(&config_dir.join("shaders/jelly.effect.toml"), "animation"),
            Some(("jelly".to_owned(), "jelly.glsl".to_owned()))
        );
        delete_user_shader(&config_dir, &renamed).unwrap();
        assert!(!config_dir.join("shaders/jelly.effect.toml").exists());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn legacy_paths_map_to_their_presets() {
        let base = std::env::temp_dir().join(format!("umbriel-legacy-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let config_dir = base.join("config");
        let data_dir = base.join("data");
        write(&config_dir.join("shaders/test.glsl"), GLSL);
        write(
            &data_dir.join("umbriel/effects/animation/reveal/shader.glsl"),
            GLSL,
        );
        write(
            &data_dir.join("umbriel/effects/animation/reveal/effect.toml"),
            b"[effects.preset.reveal]\nkind = \"animation\"\nshader = \"shader.glsl\"\n",
        );
        let entries = scan(&config_dir, std::slice::from_ref(&data_dir));
        // Your own file maps to itself.
        let own = legacy_target(&entries, &config_dir.join("shaders/test.glsl")).unwrap();
        assert_eq!(own.preset, "test");
        // Umbriel's old bundled path maps to the bundled preset.
        let old_bundled = data_dir.join("umbriel/shaders/reveal.glsl");
        assert_eq!(
            legacy_target(&entries, &old_bundled).unwrap().preset,
            "reveal"
        );
        // Anything else can't be converted automatically.
        assert!(legacy_target(&entries, Path::new("/tmp/nowhere.glsl")).is_none());
        std::fs::remove_dir_all(&base).ok();
    }
}
