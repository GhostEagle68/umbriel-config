//! The visual effect builder: a stack of named steps the user composes
//! and reorders, each with its own parameters. The generator emits them
//! as one shader — motion steps (which move where the window's pixels
//! are sampled) run first in list order, then color steps in list
//! order. The output is ordinary shader code the user can keep editing
//! by hand.

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
#[cfg(test)]
mod tests {
    use super::*;

    const GLSL_STR: &str = "vec4 animation(vec2 uv) { return umbriel_sample(uv); }\n";

    #[test]
    fn builder_generates_working_code_for_every_kind() {
        for def in STEP_DEFS {
            let shader = generate_stack(&[def.default_step()]);
            assert!(shader.contains("vec4 animation(vec2 uv)"));
            assert!(
                shader.contains("umbriel_sample(uv)"),
                "{} samples",
                def.label
            );
        }
        // The parameter lands in the code and moves with the slider.
        let small = generate_stack(&[BuilderStep {
            kind: "scale",
            params: [0.5, 0.0, 0.0],
        }]);
        let large = generate_stack(&[BuilderStep {
            kind: "scale",
            params: [1.0, 0.0, 0.0],
        }]);
        assert!(small.contains("mix(0.50, 1.0, vis)"));
        assert!(large.contains("mix(1.00, 1.0, vis)"));
        // Unknown kinds are skipped; out-of-range params clamp.
        let skipped = generate_stack(&[BuilderStep {
            kind: "nope",
            params: [0.0; 3],
        }]);
        assert!(skipped.contains("umbriel_sample(uv)"));
        let clamped = generate_stack(&[BuilderStep {
            kind: "shatter",
            params: [99.0, -5.0, 0.1],
        }]);
        assert!(clamped.contains("floor(uv * 12.0)"), "grid clamps to max");
        assert!(clamped.contains("0.00 * t * t"), "gravity clamps to min");
    }

    #[test]
    fn builder_indents_every_body_line() {
        for def in STEP_DEFS {
            let shader = generate_stack(&[def.default_step()]);
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
        for def in STEP_DEFS {
            let step = def.default_step();
            let code = generate_stack(&[step]);
            let parsed = parse_stack(&code).expect(def.label);
            assert_eq!(parsed.len(), 1, "{}", def.label);
            assert_eq!(parsed[0].kind, def.kind);
            assert_eq!(parsed[0].params, step.params, "{}", def.label);
        }
        // A mixed stack comes back motion-first, which regenerates the
        // same code.
        let stack = [
            BuilderStep {
                kind: "fade",
                params: [0.25, 0.0, 0.0],
            },
            BuilderStep {
                kind: "shatter",
                params: [9.0, 0.35, 0.05],
            },
            BuilderStep {
                kind: "glow",
                params: [0.5, 0.0, 0.0],
            },
            BuilderStep {
                kind: "wobble",
                params: [0.015, 0.0, 0.0],
            },
        ];
        let code = generate_stack(&stack);
        let parsed = parse_stack(&code).unwrap();
        let kinds: Vec<&str> = parsed.iter().map(|step| step.kind).collect();
        assert_eq!(kinds, vec!["shatter", "wobble", "fade", "glow"]);
        assert_eq!(generate_stack(&parsed), code);
        // An empty stack is still builder output.
        assert!(parse_stack(&generate_stack(&[])).unwrap().is_empty());
    }

    #[test]
    fn the_same_effect_can_be_stacked_twice() {
        let shatter = STEP_DEFS
            .iter()
            .find(|def| def.kind == "shatter")
            .unwrap()
            .default_step();
        let code = generate_stack(&[shatter, shatter]);
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
        assert_eq!(parse_stack(&code).unwrap().len(), 2);
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
        let parsed = parse_stack(code).unwrap();
        let kinds: Vec<&str> = parsed.iter().map(|step| step.kind).collect();
        assert_eq!(kinds, vec!["shatter", "fade"]);
        assert_eq!(parsed[0].params, [12.0, 0.4, 0.3]);
        assert_eq!(parsed[1].params[0], 0.25);
        // ...and regenerate with the fixed direction.
        let upgraded = generate_stack(&parsed);
        assert!(upgraded.contains("color *= mix(0.25, 1.0, vis);"));
        assert!(upgraded.contains("clamp(((1.0 - vis) - seed * 0.5) / 0.5"));
        assert_eq!(parse_stack(&upgraded).unwrap(), parsed);
    }

    #[test]
    fn parse_stack_rejects_hand_edited_code() {
        let code = generate_stack(&default_steps());
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
            SCAFFOLD,
        ] {
            assert!(parse_stack(edited).is_none(), "{edited}");
        }
    }

    #[test]
    fn builder_motion_runs_before_color() {
        let shader = generate_stack(&[
            BuilderStep {
                kind: "fade",
                params: [0.5, 0.0, 0.0],
            },
            BuilderStep {
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
}
