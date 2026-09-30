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
    "effects.border",
    "effects.window",
    "effects.screen",
    "effects.cursor",
];

/// The kinds of effect a shader can be, in display order. Only
/// `animation` is chosen per event; the others by `[effects] <kind>`.
pub const KINDS: &[&str] = &["animation", "border", "window", "screen", "cursor"];

/// The config keys a slot (an [`EVENTS`] entry) is assigned through.
pub fn slot_key(slot: &str) -> Vec<&str> {
    match slot.strip_prefix("effects.") {
        Some(kind) => vec!["effects", kind],
        None => vec!["animation", slot, "effect"],
    }
}

/// The kind of shader a slot takes.
pub fn slot_kind(slot: &str) -> &str {
    slot.strip_prefix("effects.").unwrap_or("animation")
}

/// The kind a shader's entry point (`vec4 cursor(`) names, if it defines
/// one. Borders, windows, screens and cursors win over animation.
pub fn entry_kind(code: &str) -> Option<&'static str> {
    KINDS[1..]
        .iter()
        .chain(&KINDS[..1])
        .find(|kind| declares_entry(code, kind))
        .copied()
}

/// A shader's kind, from the function it defines; animation when it
/// defines none.
pub fn kind_of(code: &str) -> &'static str {
    entry_kind(code).unwrap_or(KINDS[0])
}

/// The do-nothing starting shader for `kind`, documenting its contract
/// right in the code. Animation shaders normally start from the builder.
pub fn scaffold(kind: &str) -> &'static str {
    match kind {
        "border" => {
            "\
// umbriel calls border() for every pixel of the focused window's ring.
// uv runs (0,0) top-left to (1,1) bottom-right over the ring. Useful inputs:
//   umbriel_sample(uv)           the native ring's pixels at uv
//   umbriel_time                 seconds on the animation clock, times the preset's speed
//   umbriel_border_distance(uv)  pixels to the window, negative inside it
//   umbriel_palette_at(t)        accent colors, when the preset sets palette = true
vec4 border(vec2 uv) {
    return umbriel_sample(uv);
}
"
        }
        "window" => {
            "\
// umbriel calls window() for every pixel of each window it runs on.
// uv runs (0,0) top-left to (1,1) bottom-right. Useful inputs:
//   umbriel_sample(uv)  what is on screen at uv
//   umbriel_size        window size in pixels
//   umbriel_time        seconds on the animation clock
vec4 window(vec2 uv) {
    return umbriel_sample(uv);
}
"
        }
        "screen" => {
            "\
// umbriel calls screen() for every pixel of the output.
// uv runs (0,0) top-left to (1,1) bottom-right. Useful inputs:
//   umbriel_sample(uv)  what is on screen at uv
//   umbriel_size        output size in pixels
//   umbriel_time        seconds on the animation clock
vec4 screen(vec2 uv) {
    return umbriel_sample(uv);
}
"
        }
        "cursor" => {
            "\
// umbriel calls cursor() for every pixel around the pointer.
// uv runs (0,0) top-left to (1,1) bottom-right. Useful inputs:
//   umbriel_sample(uv)  what is on screen at uv
//   umbriel_pointer     the pointer position in uv
//   umbriel_time        seconds on the animation clock
vec4 cursor(vec2 uv) {
    return umbriel_sample(uv);
}
"
        }
        _ => builder::SCAFFOLD,
    }
}

/// Umbriel's limits for a usable shader file (docs/user/effects.md).
const MAX_SIZE: u64 = 256 * 1024;

/// One discovered shader file.
pub struct ShaderEntry {
    /// File stem, e.g. `reveal`.
    pub name: String,
    /// Absolute path on disk.
    pub path: PathBuf,
    /// What the shader draws: one of [`KINDS`].
    pub kind: &'static str,
    /// The preset an event selects to run this shader (`effect = "…"`).
    pub preset: String,
    /// The TOML file defining that preset; see [`preset_file_for`]. It
    /// may not exist yet — assigning the shader writes it.
    pub preset_file: PathBuf,
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
    /// The downloaded community collection (`shaders/community/…`).
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
        for kind in KINDS {
            for path in bundled_shaders(&data_dir.join("umbriel/effects").join(kind), kind) {
                push(path, Source::Bundled, &mut entries, &mut seen);
            }
        }
    }
    entries
}

/// The shader of every bundled preset of `kind` under `dir`, in name
/// order. Presets of other kinds, or without a shader, are skipped.
fn bundled_shaders(dir: &Path, kind: &str) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = read
        .flatten()
        .filter_map(|item| {
            let preset_file = item.path().join("effect.toml");
            let (_, shader) = read_preset(&preset_file, kind)?;
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
    preset["kind"] = toml_edit::value(entry.kind);
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
         # Select it by the name \"{}\".\n{doc}",
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

/// Make the preset that runs `shader` say what the shader's entry point
/// says: an edit can turn a border shader into a window one, and umbriel
/// builds the program from the preset's `kind`. Nothing to do without a
/// preset file yet.
pub fn sync_preset_kind(shader: &Path, kind: &str) -> Result<(), String> {
    let preset_file = preset_file_for(shader);
    let Ok(text) = std::fs::read_to_string(&preset_file) else {
        return Ok(());
    };
    let Ok(mut doc) = text.parse::<toml_edit::DocumentMut>() else {
        return Ok(());
    };
    let file_name = shader.file_name().and_then(|name| name.to_str());
    let mut changed = false;
    let presets = doc
        .get_mut("effects")
        .and_then(|effects| effects.get_mut("preset"))
        .and_then(toml_edit::Item::as_table_mut);
    for (_, item) in presets.into_iter().flat_map(|presets| presets.iter_mut()) {
        let Some(preset) = item.as_table_mut() else {
            continue;
        };
        let text_of = |key| preset.get(key).and_then(toml_edit::Item::as_str);
        if text_of("shader") == file_name && text_of("kind") != Some(kind) {
            preset["kind"] = toml_edit::value(kind);
            changed = true;
        }
    }
    if changed {
        std::fs::write(&preset_file, doc.to_string())
            .map_err(|err| format!("could not write {}: {err}", preset_file.display()))?;
    }
    Ok(())
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
    let kind = std::fs::read_to_string(&path).map_or(KINDS[0], |code| kind_of(&code));
    let preset = read_preset(&preset_file, kind).map_or_else(|| name.clone(), |(preset, _)| preset);
    let invalid = validate(&path);
    let description = readme_description(&path);
    ShaderEntry {
        name,
        kind,
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
    winning(docs, &slot_key(event))
}

fn winning(docs: &[&ConfigDocument], key: &[&str]) -> Option<(String, usize)> {
    let mut found = None;
    for (index, doc) in docs.iter().enumerate() {
        if let Some(value) = doc.get_string(key)
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
        .filter(|event| slot_kind(event) == KINDS[0])
        .filter_map(|event| {
            winning(docs, &["animation", event, "shader"])
                .map(|(value, index)| (*event, value, index))
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
            .filter(|event| doc.get_string(&slot_key(event)).is_some())
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
/// refuses a file outright only for the structural rules; a missing
/// entry point is a warning — umbriel falls back to the built-in
/// effect and the author may simply be mid-typing.
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
    if entry_kind(code).is_none() {
        problems.push(
            "missing an entry point — define \"vec4 animation(vec2 uv)\" (or border, window, \
             screen, cursor); umbriel calls that function"
                .to_owned(),
        );
    }
    problems
}

/// Whether the code declares `vec4 <name>(`, spaced any way GLSL allows
/// (`vec4  border (`, a line break between, ...). A `//` comment earlier
/// on the line hides it.
fn declares_entry(code: &str, name: &str) -> bool {
    let ident = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    code.match_indices("vec4").any(|(at, _)| {
        if code[..at].chars().next_back().is_some_and(ident) {
            return false;
        }
        let line_start = code[..at].rfind('\n').map_or(0, |newline| newline + 1);
        if code[line_start..at].contains("//") {
            return false;
        }
        let rest = &code[at + "vec4".len()..];
        let after_type = rest.trim_start();
        if after_type.len() == rest.len() {
            return false; // "vec4border" is one identifier
        }
        after_type
            .strip_prefix(name)
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
    // A symlinked shader (a dotfiles checkout) is written through, so the
    // link stays a link, like the config file itself.
    let linked = path
        .is_symlink()
        .then(|| std::fs::canonicalize(path).ok())
        .flatten();
    let path = linked.as_deref().unwrap_or(path);
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
    // Synced before the rename so a crash can't leave an empty shader; any
    // failure removes the temp file.
    let written = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut file, code.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, path)
    }();
    written.map_err(|err| {
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
    if let Ok(text) = std::fs::read_to_string(&preset_file) {
        let renamed = entry_for(target.clone(), Source::ConfigDir);
        let old_name = path.file_name().and_then(|name| name.to_str());
        let carried =
            old_name.and_then(|old| carry_preset(&text, old, &file_name, &renamed.preset));
        // Written before the old file goes, so a failure loses nothing;
        // a file already at the new name is never replaced.
        if !renamed.preset_file.exists() {
            let text = carried.unwrap_or_else(|| preset_text(&renamed));
            std::fs::write(&renamed.preset_file, text).map_err(|err| {
                format!("could not write {}: {err}", renamed.preset_file.display())
            })?;
        }
        let _ = std::fs::remove_file(&preset_file);
    }
    Ok(target)
}

/// `text`, a preset file, with the preset that ran shader file `old` now
/// running `new` under the name `preset`: every other key, table and
/// comment stays, so parameters added by hand survive a rename. `None`
/// when the text doesn't parse or has no preset for `old`.
fn carry_preset(text: &str, old: &str, new: &str, preset: &str) -> Option<String> {
    let mut doc = text.parse::<toml_edit::DocumentMut>().ok()?;
    let presets = doc.get_mut("effects")?.get_mut("preset")?.as_table_mut()?;
    let mut ran_old: Vec<String> = Vec::new();
    for (name, item) in presets.iter_mut() {
        let Some(table) = item.as_table_like_mut() else {
            continue;
        };
        if table.get("shader").and_then(toml_edit::Item::as_str) == Some(old) {
            table.insert("shader", toml_edit::value(new));
            ran_old.push(name.to_owned());
        }
    }
    let first = ran_old.first()?.clone();
    for name in ran_old {
        if name != preset
            && !presets.contains_key(preset)
            && let Some(item) = presets.remove(&name)
        {
            presets.insert(preset, item);
        }
    }
    // The header the app writes names the shader and the preset.
    let header = |file: &str, name: &str| {
        format!(
            "# Written by umbriel-config: the preset that runs {file}.\n\
             # Select it by the name \"{name}\".\n"
        )
    };
    Some(
        doc.to_string()
            .replacen(&header(old, &first), &header(new, preset), 1),
    )
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
/// directory qualify — the downloaded community collection is refused.
pub fn delete_user_shader(config_dir: &Path, path: &Path) -> Result<(), String> {
    let Ok(relative) = path.strip_prefix(user_shaders_dir(config_dir)) else {
        return Err(format!("{} is not a user shader", path.display()));
    };
    if relative.starts_with("community") {
        return Err(
            "community shaders are managed by the download — fork one to change it".to_owned(),
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

pub mod api;

pub mod builder;

pub mod code_edit;

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
        // Sorted by path: the community folder sorts before reveal.glsl.
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
    fn a_failed_shader_write_leaves_no_temp_file() {
        let base = std::env::temp_dir().join(format!("umbriel-shader-fail-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        // Renaming a file onto a directory fails, after the temp file exists.
        let target = base.join("glow.glsl");
        std::fs::create_dir_all(&target).unwrap();
        let err = write_user_shader(&target, GLSL_STR).unwrap_err();
        assert!(err.contains("could not save"), "{err}");
        let leftovers: Vec<_> = std::fs::read_dir(&base)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn writing_a_symlinked_shader_keeps_the_link() {
        let base = std::env::temp_dir().join(format!("umbriel-shader-link-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let real = base.join("dotfiles/glow.glsl");
        let link = base.join("config/shaders/glow.glsl");
        write(&real, GLSL);
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let edited = "vec4 animation(vec2 uv) { return umbriel_sample(uv) * 0.5; }\n";
        write_user_shader(&link, edited).unwrap();
        assert!(link.is_symlink());
        assert_eq!(std::fs::read_to_string(&real).unwrap(), edited);
        std::fs::remove_dir_all(&base).ok();
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
    fn a_kind_change_updates_the_preset_and_keeps_the_rest() {
        let base = std::env::temp_dir().join(format!("umbriel-presetkind-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let shader = base.join("glow.glsl");
        // No preset file yet: nothing to do, nothing created.
        write(&shader, GLSL);
        sync_preset_kind(&shader, "border").unwrap();
        assert!(!shader.with_extension("effect.toml").exists());
        // A hand-tuned preset keeps its comment and parameters.
        let preset_file = shader.with_extension("effect.toml");
        let tuned = "# mine\n[effects.preset.glow]\nkind = \"animation\"\nshader = \"glow.glsl\"\npalette = true\n\n[effects.preset.other]\nkind = \"window\"\nshader = \"other.glsl\"\n";
        write(&preset_file, tuned.as_bytes());
        sync_preset_kind(&shader, "border").unwrap();
        assert_eq!(
            std::fs::read_to_string(&preset_file).unwrap(),
            tuned.replacen("animation", "border", 1)
        );
        // Already right: the file isn't rewritten.
        std::fs::remove_file(&preset_file).ok();
        write(
            &preset_file,
            tuned.replacen("animation", "border", 1).as_bytes(),
        );
        let before = std::fs::metadata(&preset_file).unwrap().modified().unwrap();
        sync_preset_kind(&shader, "border").unwrap();
        assert_eq!(
            std::fs::metadata(&preset_file).unwrap().modified().unwrap(),
            before
        );
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn every_kinds_entry_point_lints_clean_and_names_its_kind() {
        for kind in KINDS {
            let code = format!("vec4  {kind} (vec2 uv) {{ return umbriel_sample(uv); }}\n");
            assert!(lint_source(&code).is_empty(), "{kind}");
            assert_eq!(kind_of(&code), *kind);
            let scaffold = scaffold(kind);
            assert!(lint_source(scaffold).is_empty(), "{kind} scaffold");
            assert_eq!(kind_of(scaffold), *kind, "{kind} scaffold");
        }
        // A commented-out signature is not an entry point.
        let commented = "// vec4 window(vec2 uv)\nvec4 animation(vec2 uv) { return vec4(0.0); }\n";
        assert_eq!(kind_of(commented), "animation");
        assert_eq!(entry_kind("float x = 1.0;"), None);
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
    fn effect_slots_use_the_effects_table_and_their_kind() {
        assert_eq!(
            slot_key("windows_in"),
            ["animation", "windows_in", "effect"]
        );
        assert_eq!(slot_key("effects.cursor"), ["effects", "cursor"]);
        assert_eq!(slot_kind("effects.screen"), "screen");
        assert_eq!(slot_kind("layers"), "animation");
        assert_eq!(
            kind_of("vec4 cursor(vec2 uv) { return vec4(0.0); }"),
            "cursor"
        );
        assert_eq!(
            kind_of("vec4 animation(vec2 uv) { return vec4(0.0); }"),
            "animation"
        );
        let doc = ConfigDocument::from_str("[effects]\ncursor = \"glow\"\n").unwrap();
        assert_eq!(
            current_assignment(&[&doc], "effects.cursor"),
            Some(("glow".to_owned(), 0))
        );
        assert_eq!(current_assignment(&[&doc], "effects.screen"), None);
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
    fn renaming_keeps_the_parameters_added_to_the_preset_file() {
        let base = std::env::temp_dir().join(format!("umbriel-renamekeep-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        let config_dir = base.join("config");
        let shader = config_dir.join("shaders/pulse.glsl");
        write(
            &shader,
            b"vec4 border(vec2 uv) { return umbriel_sample(uv); }\n",
        );
        ensure_preset_file(&entry_for(shader.clone(), Source::ConfigDir)).unwrap();
        // A hand-tuned file: parameters, a light table, another preset.
        let preset_file = config_dir.join("shaders/pulse.effect.toml");
        let text = std::fs::read_to_string(&preset_file).unwrap();
        let tuned = format!(
            "{text}speed = 2.0\npalette = true\n\n[effects.preset.pulse.light]\nspread = 120\n\n\
             [effects.preset.other]\nkind = \"window\"\nshader = \"other.glsl\"\n"
        );
        write(&preset_file, tuned.as_bytes());

        rename_user_shader(&config_dir, &shader, "glow").unwrap();
        let renamed = std::fs::read_to_string(config_dir.join("shaders/glow.effect.toml")).unwrap();
        assert!(!preset_file.exists());
        // The preset runs the new file under the new name, keeping every
        // other key and the untouched preset, and the header says so.
        assert_eq!(
            renamed,
            tuned
                .replace("pulse.glsl", "glow.glsl")
                .replace("\"pulse\"", "\"glow\"")
                .replace("preset.pulse", "preset.glow")
        );
        let doc: ConfigDocument = renamed.parse().unwrap();
        let key = |key: &str| doc.get_string(&["effects", "preset", "glow", key]);
        assert_eq!(key("kind").as_deref(), Some("border"));
        assert_eq!(key("shader").as_deref(), Some("glow.glsl"));
        assert_eq!(key("speed"), None);
        assert!(renamed.contains("speed = 2.0") && renamed.contains("spread = 120"));
        // Text with no preset for the file can't be carried.
        assert_eq!(carry_preset("# nothing\n", "a.glsl", "b.glsl", "b"), None);
        assert_eq!(carry_preset("not = [toml", "a.glsl", "b.glsl", "b"), None);
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
