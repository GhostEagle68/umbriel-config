//! Umbriel's animation timing, so the shader preview plays an event the
//! way the compositor would: the easing curves, the cubic-Bézier solver
//! and spring physics from umbriel's `src/core/animation.cpp`, and the
//! per-event curve/duration resolution from `src/config/config.{h,cpp}`
//! (built-in defaults, then `[animation]`, then the event's own table).

use super::document::ConfigDocument;

/// An easing curve as umbriel evaluates it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    Linear,
    Named(Easing),
    /// Cubic Bézier control points `x1, y1, x2, y2` (x in 0..=1).
    Bezier([f64; 4]),
    /// A spring from rest at 0 to 1; mass is always 1 from config.
    Spring {
        damping: f64,
        stiffness: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    InSine,
    OutSine,
    InOutSine,
    InQuad,
    OutQuad,
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    InQuart,
    OutQuart,
    InOutQuart,
    InQuint,
    OutQuint,
    InOutQuint,
    InExpo,
    OutExpo,
    InOutExpo,
    InCirc,
    OutCirc,
    InOutCirc,
    InBack,
    OutBack,
    InOutBack,
    InElastic,
    OutElastic,
    InOutElastic,
    InBounce,
    OutBounce,
    InOutBounce,
}

/// Umbriel's fallback curve (`[animation]` with nothing set).
pub const EASE_OUT: Curve = Curve::Named(Easing::OutCubic);

/// A resolved event timeline: its curve and how long it runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Timeline {
    pub curve: Curve,
    /// The configured length for duration-based curves; springs ignore
    /// it (see [`Timeline::length_ms`]).
    pub duration_ms: u32,
}

impl Timeline {
    /// How long the event actually runs: a spring's own settle time,
    /// otherwise the configured duration.
    pub fn length_ms(&self) -> u32 {
        match self.curve {
            Curve::Spring { damping, stiffness } => spring_duration_ms(damping, stiffness),
            _ => self.duration_ms,
        }
    }
}

/// Umbriel's registry names, compared after `normalize`.
fn registry(name: &str) -> Option<Curve> {
    use Easing::*;
    let spring = |damping, stiffness| Curve::Spring { damping, stiffness };
    Some(match name {
        "linear" => Curve::Linear,
        "easeinsine" => Curve::Named(InSine),
        "easeoutsine" => Curve::Named(OutSine),
        "easeinoutsine" => Curve::Named(InOutSine),
        "easeinquad" => Curve::Named(InQuad),
        "easeoutquad" | "quad" => Curve::Named(OutQuad),
        "easeinoutquad" => Curve::Named(InOutQuad),
        "easeincubic" | "easein" => Curve::Named(InCubic),
        "easeoutcubic" | "cubic" | "ease" | "easeout" => Curve::Named(OutCubic),
        "easeinoutcubic" | "easeinout" => Curve::Named(InOutCubic),
        "easeinquart" => Curve::Named(InQuart),
        "easeoutquart" | "quart" => Curve::Named(OutQuart),
        "easeinoutquart" => Curve::Named(InOutQuart),
        "easeinquint" => Curve::Named(InQuint),
        // Umbriel re-registers this name as a Bézier after the quint set.
        "easeoutquint" => Curve::Bezier([0.23, 1.0, 0.32, 1.0]),
        "quint" => Curve::Named(OutQuint),
        "easeinoutquint" => Curve::Named(InOutQuint),
        "easeinexpo" => Curve::Named(InExpo),
        "easeoutexpo" | "expo" => Curve::Named(OutExpo),
        "easeinoutexpo" => Curve::Named(InOutExpo),
        "easeincirc" => Curve::Named(InCirc),
        "easeoutcirc" | "circ" => Curve::Named(OutCirc),
        "easeinoutcirc" => Curve::Named(InOutCirc),
        "easeinback" => Curve::Named(InBack),
        "easeoutback" | "back" | "overshoot" => Curve::Named(OutBack),
        "easeinoutback" => Curve::Named(InOutBack),
        "easeinelastic" => Curve::Named(InElastic),
        "easeoutelastic" | "elastic" => Curve::Named(OutElastic),
        "easeinoutelastic" => Curve::Named(InOutElastic),
        "easeinbounce" => Curve::Named(InBounce),
        "easeoutbounce" | "bounce" => Curve::Named(OutBounce),
        "easeinoutbounce" => Curve::Named(InOutBounce),
        "snappy" | "default" => Curve::Bezier([0.05, 0.9, 0.1, 1.05]),
        "defaultspring" => spring(0.75, 100.0),
        "bouncy" => spring(0.5, 120.0),
        "smooth" => spring(0.9, 90.0),
        "stiff" => spring(0.8, 200.0),
        _ => return None,
    })
}

/// Registry lookups ignore case, `_`, `-` and spaces.
fn normalize(name: &str) -> String {
    name.chars()
        .filter(|ch| !matches!(ch, '_' | '-' | ' '))
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

/// Named curves from `[animation.beziers]` / `[animation.springs]`.
#[derive(Debug, Default, Clone)]
pub struct CurveTables {
    beziers: Vec<(String, [f64; 4])>,
    springs: Vec<(String, (f64, f64))>,
}

/// Parse a `curve` string the way umbriel does: the user's named
/// Béziers and springs first, then the registry, then `x1,y1,x2,y2`,
/// then `spring:damping,stiffness`. `None` for anything umbriel rejects.
pub fn parse(text: &str, tables: &CurveTables) -> Option<Curve> {
    let lower = text.to_lowercase();
    if let Some((_, points)) = tables
        .beziers
        .iter()
        .find(|(name, _)| name.to_lowercase() == lower)
    {
        return Some(Curve::Bezier(*points));
    }
    if let Some((_, (damping, stiffness))) = tables
        .springs
        .iter()
        .find(|(name, _)| name.to_lowercase() == lower)
    {
        return Some(Curve::Spring {
            damping: *damping,
            stiffness: *stiffness,
        });
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(curve) = registry(&normalize(trimmed)) {
        return Some(curve);
    }
    if let Some(points) = numbers::<4>(trimmed).filter(|p| valid_bezier(*p)) {
        return Some(Curve::Bezier(points));
    }
    let [damping, stiffness] = numbers::<2>(trimmed.strip_prefix("spring:")?)?;
    valid_spring(damping, stiffness).then_some(Curve::Spring { damping, stiffness })
}

/// Exactly `N` comma-separated finite numbers.
fn numbers<const N: usize>(text: &str) -> Option<[f64; N]> {
    let parts: Vec<f64> = text
        .split(',')
        .map(|part| part.trim().parse::<f64>().ok().filter(|n| n.is_finite()))
        .collect::<Option<_>>()?;
    parts.try_into().ok()
}

fn valid_bezier([x1, _, x2, _]: [f64; 4]) -> bool {
    (0.0..=1.0).contains(&x1) && (0.0..=1.0).contains(&x2)
}

fn valid_spring(damping: f64, stiffness: f64) -> bool {
    (0.01..=5.0).contains(&damping) && (1.0..=10000.0).contains(&stiffness)
}

/// The eased value at linear progress `t` (clamped to 0..=1), with any
/// overshoot kept — what umbriel feeds `umbriel_progress`.
pub fn ease(curve: Curve, t: f64) -> f64 {
    use Easing::*;
    use std::f64::consts::PI;
    let x = t.clamp(0.0, 1.0);
    let easing = match curve {
        Curve::Linear => return x,
        Curve::Bezier([x1, y1, x2, y2]) => return solve_bezier(x1, y1, x2, y2, x),
        Curve::Spring { damping, stiffness } => {
            let seconds = x * f64::from(spring_duration_ms(damping, stiffness)) / 1000.0;
            return spring_position(damping, stiffness, seconds).0;
        }
        Curve::Named(easing) => easing,
    };
    let out_bounce = |mut t: f64| {
        const N1: f64 = 7.5625;
        const D1: f64 = 2.75;
        if t < 1.0 / D1 {
            N1 * t * t
        } else if t < 2.0 / D1 {
            t -= 1.5 / D1;
            N1 * t * t + 0.75
        } else if t < 2.5 / D1 {
            t -= 2.25 / D1;
            N1 * t * t + 0.9375
        } else {
            t -= 2.625 / D1;
            N1 * t * t + 0.984375
        }
    };
    let rest = 1.0 - x;
    match easing {
        InSine => 1.0 - (x * PI / 2.0).cos(),
        OutSine => (x * PI / 2.0).sin(),
        InOutSine => -((PI * x).cos() - 1.0) / 2.0,
        InQuad => x * x,
        OutQuad => 1.0 - rest * rest,
        InOutQuad => {
            if x < 0.5 {
                2.0 * x * x
            } else {
                1.0 - (-2.0 * x + 2.0).powi(2) / 2.0
            }
        }
        InCubic => x.powi(3),
        OutCubic => 1.0 - rest.powi(3),
        InOutCubic => {
            if x < 0.5 {
                4.0 * x.powi(3)
            } else {
                1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
            }
        }
        InQuart => x.powi(4),
        OutQuart => 1.0 - rest.powi(4),
        InOutQuart => {
            if x < 0.5 {
                8.0 * x.powi(4)
            } else {
                1.0 - (-2.0 * x + 2.0).powi(4) / 2.0
            }
        }
        InQuint => x.powi(5),
        OutQuint => 1.0 - rest.powi(5),
        InOutQuint => {
            if x < 0.5 {
                16.0 * x.powi(5)
            } else {
                1.0 - (-2.0 * x + 2.0).powi(5) / 2.0
            }
        }
        InExpo => {
            if x <= 0.0 {
                0.0
            } else {
                2f64.powf(10.0 * x - 10.0)
            }
        }
        OutExpo => {
            if x >= 1.0 {
                1.0
            } else {
                1.0 - 2f64.powf(-10.0 * x)
            }
        }
        InOutExpo => {
            if x <= 0.0 {
                0.0
            } else if x >= 1.0 {
                1.0
            } else if x < 0.5 {
                2f64.powf(20.0 * x - 10.0) / 2.0
            } else {
                (2.0 - 2f64.powf(-20.0 * x + 10.0)) / 2.0
            }
        }
        InCirc => 1.0 - (1.0 - x * x).sqrt(),
        OutCirc => (1.0 - (x - 1.0).powi(2)).sqrt(),
        InOutCirc => {
            if x < 0.5 {
                (1.0 - (1.0 - (2.0 * x).powi(2)).sqrt()) / 2.0
            } else {
                ((1.0 - (-2.0 * x + 2.0).powi(2)).sqrt() + 1.0) / 2.0
            }
        }
        InBack => {
            const C1: f64 = 1.70158;
            (C1 + 1.0) * x.powi(3) - C1 * x * x
        }
        OutBack => {
            const C1: f64 = 1.70158;
            let t = x - 1.0;
            1.0 + (C1 + 1.0) * t.powi(3) + C1 * t * t
        }
        InOutBack => {
            const C2: f64 = 1.70158 * 1.525 + 1.0;
            if x < 0.5 {
                (2.0 * x).powi(2) * ((C2 + 1.0) * 2.0 * x - C2) / 2.0
            } else {
                ((2.0 * x - 2.0).powi(2) * ((C2 + 1.0) * (2.0 * x - 2.0) + C2) + 2.0) / 2.0
            }
        }
        InElastic | OutElastic | InOutElastic if x <= 0.0 => 0.0,
        InElastic | OutElastic | InOutElastic if x >= 1.0 => 1.0,
        InElastic => -2f64.powf(10.0 * x - 10.0) * ((x * 10.0 - 10.75) * (2.0 * PI / 3.0)).sin(),
        OutElastic => 2f64.powf(-10.0 * x) * ((x * 10.0 - 0.75) * (2.0 * PI / 3.0)).sin() + 1.0,
        InOutElastic => {
            let c5 = 2.0 * PI / 4.5;
            if x < 0.5 {
                -(2f64.powf(20.0 * x - 10.0) * ((20.0 * x - 11.125) * c5).sin()) / 2.0
            } else {
                2f64.powf(-20.0 * x + 10.0) * ((20.0 * x - 11.125) * c5).sin() / 2.0 + 1.0
            }
        }
        InBounce => 1.0 - out_bounce(1.0 - x),
        OutBounce => out_bounce(x),
        InOutBounce => {
            if x < 0.5 {
                (1.0 - out_bounce(1.0 - 2.0 * x)) / 2.0
            } else {
                (1.0 + out_bounce(2.0 * x - 1.0)) / 2.0
            }
        }
    }
}

/// Umbriel's `solveCubicBezier`: Newton–Raphson, then bisection.
fn solve_bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let (x1, x2) = (x1.clamp(0.0, 1.0), x2.clamp(0.0, 1.0));
    let cx = 3.0 * x1;
    let bx = 3.0 * (x2 - x1) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * y1;
    let by = 3.0 * (y2 - y1) - cy;
    let ay = 1.0 - cy - by;
    let eval_x = |t: f64| ((ax * t + bx) * t + cx) * t;
    let eval_y = |t: f64| ((ay * t + by) * t + cy) * t;
    let eval_dx = |t: f64| (3.0 * ax * t + 2.0 * bx) * t + cx;

    let mut t = x;
    for _ in 0..8 {
        let current = eval_x(t) - x;
        if current.abs() < 1e-7 {
            return eval_y(t);
        }
        let dx = eval_dx(t);
        if dx.abs() < 1e-6 {
            break;
        }
        let next = t - current / dx;
        if !(0.0..=1.0).contains(&next) {
            break;
        }
        t = next;
    }
    let (mut low, mut high) = (0.0, 1.0);
    t = x;
    for _ in 0..12 {
        let guess = eval_x(t);
        if (guess - x).abs() < 1e-7 {
            return eval_y(t);
        }
        if x > guess {
            low = t;
        } else {
            high = t;
        }
        t = 0.5 * (low + high);
    }
    eval_y(t)
}

/// Umbriel's `solveSpringPhysics` for the unit step from rest: position
/// and velocity after `seconds`.
fn spring_position(damping: f64, stiffness: f64, seconds: f64) -> (f64, f64) {
    if seconds <= 0.0 {
        return (0.0, 0.0);
    }
    let w0 = stiffness.max(1e-4).sqrt();
    let zeta = damping.max(0.0);
    let beta = zeta * w0;
    let x0 = -1.0; // from 0 toward 1
    let t = seconds;
    let env = (-beta * t).exp();
    let (offset, velocity) = if (zeta - 1.0).abs() < 1e-5 {
        (env * (x0 + w0 * x0 * t), env * (-(w0 * x0) * w0 * t))
    } else if zeta < 1.0 {
        let wd = w0 * (1.0 - zeta * zeta).sqrt();
        let c2 = beta * x0 / wd;
        (
            env * (x0 * (wd * t).cos() + c2 * (wd * t).sin()),
            env * (-(x0 * wd + beta * c2) * (wd * t).sin()),
        )
    } else {
        let wd = w0 * (zeta * zeta - 1.0).sqrt();
        let c2 = beta * x0 / wd;
        (
            env * (x0 * (wd * t).cosh() + c2 * (wd * t).sinh()),
            env * ((x0 * wd - beta * c2) * (wd * t).sinh()),
        )
    };
    if offset.abs() < 1e-4 && velocity.abs() < 1e-4 {
        return (1.0, 0.0);
    }
    (1.0 + offset, velocity)
}

/// Umbriel's `springDurationMs`: when the unit step's remaining energy
/// drops under 1e-4, found by doubling then bisecting; 1..=10000 ms.
pub fn spring_duration_ms(damping: f64, stiffness: f64) -> u32 {
    const MAX_SECONDS: f64 = 10.0;
    const EPSILON: f64 = 1e-4;
    let stiffness = stiffness.max(1e-4);
    let remaining = |seconds: f64| {
        let (position, velocity) = spring_position(damping, stiffness, seconds);
        (1.0 - position).hypot(velocity * (1.0 / stiffness).sqrt())
    };
    let mut settled = (1.0 / stiffness).sqrt().min(MAX_SECONDS);
    while settled < MAX_SECONDS && remaining(settled) > EPSILON {
        settled = (settled * 2.0).min(MAX_SECONDS);
    }
    let mut ms = 10_000;
    if remaining(settled) <= EPSILON {
        let mut unsettled = 0.0;
        for _ in 0..40 {
            let middle = 0.5 * (unsettled + settled);
            if remaining(middle) > EPSILON {
                unsettled = middle;
            } else {
                settled = middle;
            }
        }
        ms = (settled * 1000.0).ceil() as u32;
    }
    ms.clamp(1, 10_000)
}

/// An event's built-in timeline (umbriel's `config.h` defaults).
fn event_default(event: &str) -> Curve {
    let spring = |stiffness| Curve::Spring {
        damping: 1.0,
        stiffness,
    };
    match event {
        "windows_in" | "windows_move" | "border" => spring(900.0),
        "windows_out" => spring(1400.0),
        "workspaces" | "overview" | "scratchpad" => spring(800.0),
        _ => EASE_OUT, // dim_unfocused, layers
    }
}

/// The value from the document that wins (the last one in chain order
/// that sets it; `docs` is includes first, main last).
fn winning<T>(docs: &[&ConfigDocument], read: impl Fn(&ConfigDocument) -> Option<T>) -> Option<T> {
    docs.iter().rev().find_map(|doc| read(doc))
}

/// `[animation.beziers]` and `[animation.springs]` across the chain;
/// later documents override earlier ones by name.
pub fn tables(docs: &[&ConfigDocument]) -> CurveTables {
    let mut tables = CurveTables::default();
    for doc in docs {
        for name in doc.table_keys(&["animation", "beziers"]) {
            let points = doc
                .get_numbers(&["animation", "beziers", &name])
                .and_then(|values| <[f64; 4]>::try_from(values).ok())
                .filter(|points| points.iter().all(|n| n.is_finite()) && valid_bezier(*points));
            if let Some(points) = points {
                tables.beziers.retain(|(existing, _)| *existing != name);
                tables.beziers.push((name, points));
            }
        }
        for name in doc.table_keys(&["animation", "springs"]) {
            let read = |key| doc.get_number(&["animation", "springs", &name, key]);
            if doc.table_keys(&["animation", "springs", &name]).len() != 2 {
                continue;
            }
            if let (Some(damping), Some(stiffness)) = (read("damping"), read("stiffness"))
                && valid_spring(damping, stiffness)
            {
                tables.springs.retain(|(existing, _)| *existing != name);
                tables.springs.push((name, (damping, stiffness)));
            }
        }
    }
    tables
}

/// The timeline umbriel runs `event` with: its built-in default, then
/// `[animation]`'s `curve`/`duration_ms` (which apply to every event),
/// then the event's own. Invalid values are skipped, as umbriel does.
pub fn event_timeline(docs: &[&ConfigDocument], event: &str) -> Timeline {
    let tables = tables(docs);
    let duration = |path: &[&str]| {
        winning(docs, |doc| doc.get_integer(path))
            .filter(|ms| (1..=10_000).contains(ms))
            .map(|ms| ms as u32)
    };
    let curve = |path: &[&str]| {
        winning(docs, |doc| doc.get_string(path)).and_then(|text| parse(&text, &tables))
    };
    let mut timeline = Timeline {
        curve: event_default(event),
        duration_ms: 250,
    };
    if let Some(ms) = duration(&["animation", "duration_ms"]) {
        timeline.duration_ms = ms;
    }
    if let Some(global) = curve(&["animation", "curve"]) {
        timeline.curve = global;
    }
    if let Some(ms) = duration(&["animation", event, "duration_ms"]) {
        timeline.duration_ms = ms;
    }
    if let Some(own) = curve(&["animation", event, "curve"]) {
        timeline.curve = own;
    }
    timeline
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn named_curves_match_umbriels_formulas() {
        let none = CurveTables::default();
        let at = |name: &str, t| ease(parse(name, &none).unwrap(), t);
        assert!(close(at("linear", 0.3), 0.3));
        assert!(close(at("easeout", 0.5), 0.875));
        // Names ignore case, dashes and underscores.
        assert!(close(at("Ease-Out", 0.5), 0.875));
        assert!(close(at("ease_in_out_quad", 0.25), 0.125));
        assert!(close(at("bounce", 1.0), 1.0));
        // Back overshoots past 1 before settling.
        assert!((0.0..1.0).any_over(|t| at("back", t) > 1.0));
        // Ends are exact for every named curve.
        for name in [
            "easeinelastic",
            "easeoutexpo",
            "easeinoutcirc",
            "snappy",
            "quint",
        ] {
            assert!(close(at(name, 0.0), 0.0), "{name}");
            assert!(close(at(name, 1.0), 1.0), "{name}");
        }
    }

    trait AnyOver {
        fn any_over(self, test: impl Fn(f64) -> bool) -> bool;
    }
    impl AnyOver for std::ops::Range<f64> {
        fn any_over(self, test: impl Fn(f64) -> bool) -> bool {
            (0..100)
                .map(|i| self.start + (self.end - self.start) * i as f64 / 100.0)
                .any(test)
        }
    }

    #[test]
    fn bezier_and_spring_strings_parse_with_umbriels_limits() {
        let none = CurveTables::default();
        assert_eq!(
            parse(" 0.05, 0.9, 0.1, 1.05 ", &none),
            Some(Curve::Bezier([0.05, 0.9, 0.1, 1.05]))
        );
        assert_eq!(parse("1.5,0,0,1", &none), None, "x must stay in 0..=1");
        assert_eq!(
            parse("spring:1,900", &none),
            Some(Curve::Spring {
                damping: 1.0,
                stiffness: 900.0
            })
        );
        assert_eq!(parse("spring:9,900", &none), None, "damping capped at 5");
        assert_eq!(parse("wobbly", &none), None);
        // Snappy's Bézier: monotone and near-complete by the middle.
        let snappy = parse("snappy", &none).unwrap();
        assert!(ease(snappy, 0.5) > 0.9);
    }

    #[test]
    fn springs_settle_and_derive_their_length() {
        let stiff = Curve::Spring {
            damping: 1.0,
            stiffness: 900.0,
        };
        let loose = Curve::Spring {
            damping: 1.0,
            stiffness: 100.0,
        };
        let timeline = |curve| Timeline {
            curve,
            duration_ms: 250,
        };
        // Stiffer settles faster; the configured duration is ignored.
        assert!(timeline(stiff).length_ms() < timeline(loose).length_ms());
        assert!((200..=1000).contains(&timeline(stiff).length_ms()));
        assert!(close(ease(stiff, 0.0), 0.0));
        // Like umbriel, a spring ends within its settle threshold of 1.
        assert!((ease(stiff, 1.0) - 1.0).abs() < 1e-3);
        // An underdamped spring overshoots.
        let bouncy = Curve::Spring {
            damping: 0.4,
            stiffness: 200.0,
        };
        assert!((0.0..1.0).any_over(|t| ease(bouncy, t) > 1.01));
    }

    #[test]
    fn event_timelines_layer_defaults_global_and_event() {
        let empty = ConfigDocument::from_str("").unwrap();
        // Built-in: windows_in is a spring, layers ease-out over 250 ms.
        assert!(matches!(
            event_timeline(&[&empty], "windows_in").curve,
            Curve::Spring { .. }
        ));
        assert_eq!(
            event_timeline(&[&empty], "layers"),
            Timeline {
                curve: EASE_OUT,
                duration_ms: 250
            }
        );
        // [animation] applies to every event; the event's own wins; an
        // include's value loses to the main file's.
        let include = ConfigDocument::from_str(
            "[animation]\ncurve = \"linear\"\nduration_ms = 400\n[animation.beziers]\nmine = [0.2, 0.0, 0.3, 1]\n",
        )
        .unwrap();
        let main = ConfigDocument::from_str(
            "[animation]\nduration_ms = 500\n[animation.workspaces]\ncurve = \"mine\"\n[animation.springs]\nsoft = { damping = 0.6, stiffness = 150 }\n[animation.overview]\ncurve = \"soft\"\n",
        )
        .unwrap();
        let docs = [&include, &main];
        assert_eq!(
            event_timeline(&docs, "layers"),
            Timeline {
                curve: Curve::Linear,
                duration_ms: 500
            }
        );
        assert_eq!(
            event_timeline(&docs, "workspaces").curve,
            Curve::Bezier([0.2, 0.0, 0.3, 1.0])
        );
        assert_eq!(
            event_timeline(&docs, "overview").curve,
            Curve::Spring {
                damping: 0.6,
                stiffness: 150.0
            }
        );
    }
}
