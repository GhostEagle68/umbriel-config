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

/// Which document an assignment edit for `event` belongs in: the one
/// whose value currently wins, or `None` for an unset event (see
/// [`new_assignment_home`]).
pub fn assignment_home(docs: &[&ConfigDocument], event: &str) -> Option<usize> {
    current_assignment(docs, event).map(|(_, index)| index)
}

/// Where a brand-new assignment starts: the document already holding
/// the most shader assignments, where the user keeps them (a tie goes
/// to the later file, so main wins). With none anywhere, an included
/// `shaders.toml`, else main (the last doc). `file_names` parallels
/// `docs`.
pub fn new_assignment_home(docs: &[&ConfigDocument], file_names: &[&str]) -> usize {
    let main = docs.len().saturating_sub(1);
    let count = |doc: &ConfigDocument| {
        EVENTS
            .iter()
            .filter(|event| doc.get_string(&["animation", event, "shader"]).is_some())
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
    if !code.contains("vec4 animation(") {
        problems
            .push("missing \"vec4 animation(vec2 uv)\" — umbriel calls that function".to_owned());
    }
    problems
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
    std::fs::remove_file(path).map_err(|err| format!("could not delete {}: {err}", path.display()))
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
    }

    /// One step kind in the palette.
    pub struct StepDef {
        pub kind: &'static str,
        pub label: &'static str,
        /// Color steps retint the sampled pixels; motion steps move
        /// where they are sampled from.
        pub color: bool,
        pub params: &'static [StepParam],
    }

    /// A step as configured by the user: values are indexed parallel to
    /// the definition's params.
    #[derive(Debug, Clone, Copy)]
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
            }],
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
            }],
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
            }],
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
            }],
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
                },
                StepParam {
                    key: "gravity",
                    label: "Gravity",
                    min: 0.0,
                    max: 0.4,
                    default: 0.2,
                },
                StepParam {
                    key: "scatter",
                    label: "Scatter",
                    min: 0.0,
                    max: 0.3,
                    default: 0.1,
                },
            ],
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
            }],
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

    fn step_value(def: &StepDef, step: &BuilderStep, key: &str) -> f64 {
        def.params
            .iter()
            .enumerate()
            .find(|(_, param)| param.key == key)
            .map(|(index, param)| {
                step.params
                    .get(index)
                    .copied()
                    .unwrap_or(param.default)
                    .clamp(param.min, param.max)
            })
            .unwrap_or(0.0)
    }

    /// Compose the stack into a full shader.
    pub fn generate_stack(steps: &[BuilderStep]) -> String {
        let mut motion = String::new();
        let mut color = String::new();
        for step in steps {
            let Some(def) = step_def(step.kind) else {
                continue;
            };
            let value = |key: &str| step_value(def, step, key);
            let block = match step.kind {
                "fade" => format!("    color *= mix(1.0, {:.2}, vis);\n", value("to")),
                // No `\` line continuation here: it would also eat the
                // first line's indentation.
                "glow" => format!(
                    "    float pulse = {:.2} * sin(3.14159265 * p);
    color = vec4(mix(color.rgb, vec3(1.0, 0.4, 0.1) * color.a, pulse), color.a);
",
                    value("strength")
                ),
                "scale" => format!(
                    "    uv = (uv - 0.5) / mix({:.2}, 1.0, vis) + 0.5;\n",
                    value("from")
                ),
                "slide" => format!(
                    "    uv -= vec2({:.2} * (1.0 - vis), 0.0);\n",
                    value("offset")
                ),
                "shatter" => format!(
                    "    vec2 cell_id = floor(uv * {:.0}.0);
    float seed = fract(sin(dot(cell_id, vec2(12.9898, 78.233)) + umbriel_random_seed.x) * 43758.5453);
    float t = clamp((p - seed * 0.5) / 0.5, 0.0, 1.0);
    uv -= vec2((seed - 0.5) * {:.2} * t, {:.2} * t * t);
",
                    value("grid"),
                    value("scatter"),
                    value("gravity")
                ),
                "wobble" => format!(
                    "    uv.y += {:.3} * sin(uv.x * 12.566 + p * 9.0) * (1.0 - abs(2.0 * p - 1.0));\n",
                    value("amp")
                ),
                _ => String::new(),
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
        let lines: Vec<&str> = body
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let mut motion = Vec::new();
        let mut color = Vec::new();
        let mut index = 0;
        while index < lines.len() {
            let line = lines[index];
            index += 1;
            if matches!(
                line,
                "float p = umbriel_clamped_progress;"
                    | "float vis = umbriel_direction > 0.0 ? p : 1.0 - p;"
                    | "vec4 color = umbriel_sample(uv);"
                    | "return color;"
            ) {
                continue;
            }
            if let Some(to) = between(line, "color *= mix(1.0, ", ", vis);") {
                color.push(step("fade", [to, 0.0, 0.0]));
            } else if let Some(strength) =
                between(line, "float pulse = ", " * sin(3.14159265 * p);")
            {
                // The second glow line is checked by the round trip.
                index += 1;
                color.push(step("glow", [strength, 0.0, 0.0]));
            } else if let Some(from) = between(line, "uv = (uv - 0.5) / mix(", ", 1.0, vis) + 0.5;")
            {
                motion.push(step("scale", [from, 0.0, 0.0]));
            } else if let Some(offset) = between(line, "uv -= vec2(", " * (1.0 - vis), 0.0);") {
                motion.push(step("slide", [offset, 0.0, 0.0]));
            } else if let Some(grid) = between(line, "vec2 cell_id = floor(uv * ", ");") {
                // seed and t lines, then the displacement carrying the
                // other two parameters.
                let last = lines.get(index + 2)?;
                index += 3;
                let (scatter, gravity) =
                    between_str(last, "uv -= vec2((seed - 0.5) * ", " * t * t);")?
                        .split_once(" * t, ")?;
                motion.push(step(
                    "shatter",
                    [grid, gravity.parse().ok()?, scatter.parse().ok()?],
                ));
            } else {
                // The last known shape; anything else isn't builder output.
                let amp = between(
                    line,
                    "uv.y += ",
                    " * sin(uv.x * 12.566 + p * 9.0) * (1.0 - abs(2.0 * p - 1.0));",
                )?;
                motion.push(step("wobble", [amp, 0.0, 0.0]));
            }
        }
        motion.extend(color);
        (normalized(&generate_stack(&motion)) == normalized(code)).then_some(motion)
    }

    fn step(kind: &'static str, params: [f64; 3]) -> BuilderStep {
        BuilderStep { kind, params }
    }

    fn between_str<'a>(line: &'a str, prefix: &str, suffix: &str) -> Option<&'a str> {
        line.strip_prefix(prefix)?.strip_suffix(suffix)
    }

    fn between(line: &str, prefix: &str, suffix: &str) -> Option<f64> {
        between_str(line, prefix, suffix)?.parse().ok()
    }

    /// Code compared by content: indentation and blank lines ignored.
    fn normalized(code: &str) -> Vec<&str> {
        code.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
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
    fn parse_stack_reads_older_unindented_output() {
        // Before the indentation fix, shatter's first line sat at
        // column 0; those saved files still open in the builder.
        let code = builder::generate_stack(&[builder::BuilderStep {
            kind: "shatter",
            params: [12.0, 0.4, 0.3],
        }])
        .replace("    vec2 cell_id", "vec2 cell_id");
        let parsed = builder::parse_stack(&code).unwrap();
        assert_eq!(parsed[0].params, [12.0, 0.4, 0.3]);
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
            ConfigDocument::from_str("[animation.windows_in]\nshader = \"shaders/old.glsl\"\n")
                .unwrap();
        let main = ConfigDocument::from_str("[general]\nxwayland = true\n").unwrap();
        assert_eq!(assignment_home(&[&include, &main], "windows_in"), Some(0));
        assert_eq!(assignment_home(&[&include, &main], "windows_move"), None);
        // Paths are not bare TOML: the typed write must quote them.
        include.set_string(
            &["animation", "windows_move", "shader"],
            "shaders/test.glsl",
        );
        assert!(include.text().contains("shader = \"shaders/test.glsl\""));
        assert_eq!(
            current_assignment(&[&include, &main], "windows_move"),
            Some(("shaders/test.glsl".to_owned(), 0))
        );
    }

    #[test]
    fn new_assignments_start_where_the_others_live() {
        let shaders =
            ConfigDocument::from_str("[animation.windows_in]\nshader = \"shaders/a.glsl\"\n")
                .unwrap();
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
