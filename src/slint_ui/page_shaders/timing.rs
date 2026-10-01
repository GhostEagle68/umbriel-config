//! The timing dialog: the curve and duration of one animation event,
//! opened from its row on the Assignments tab, beside a preview of what
//! runs on that event. They belong to the event (Umbriel has no per-shader
//! timing), and are written straight to the config as they are edited
//! (like "Use for"): a drag previews live and writes on release.

use super::super::common::*;
use super::super::*;
use super::{library::*, preview::*};
use umbriel_config::config::curves::{self, Curve, CurveTables, Timeline};

/// The plot's size in pixels: the card draws it at exactly this size.
const PLOT: (f64, f64) = (220.0, 150.0);
/// Where a Bézier's control points may go (the plot's fixed range).
const BEZIER_Y: (f32, f32) = (-0.5, 1.5);
const MODE_PRESET: i32 = 0;
const MODE_BEZIER: i32 = 1;
const MODE_SPRING: i32 = 2;
/// Starting points when a mode is chosen with nothing of its kind to edit.
const BEZIER_START: [f64; 4] = [0.2, 0.8, 0.2, 1.0];
const SPRING_START: (f64, f64) = (1.0, 900.0);

/// The animation event the preview plays as; none for the effect slots.
fn timing_event(shell: &Shell) -> Option<&'static str> {
    let event = *shaders::EVENTS.get(shell.shader_preview_event)?;
    (shaders::slot_kind(event) == shaders::KINDS[0]).then_some(event)
}

/// What the card shows, from the config (or an edit in progress).
struct State {
    event: &'static str,
    text: String,
    curve: Curve,
    mode: i32,
    preset: i32,
    duration_ms: u32,
    note: String,
}

fn preset_names(tables: &CurveTables) -> Vec<String> {
    let builtin = umbriel_config::config::schema::BUILTIN_CURVES
        .iter()
        .map(|name| (*name).to_owned());
    builtin.chain(tables.names()).collect()
}

fn state_of(shell: &Shell, event: &'static str) -> State {
    let docs = chain_docs(shell);
    let tables = curves::tables(&docs);
    let text = curves::curve_text(&docs, event);
    let curve = curves::parse(&text, &tables).unwrap_or(curves::EASE_OUT);
    // `x1,y1,x2,y2` and `spring:d,s` are edited inline; anything else is
    // a name, built in or registered.
    let mode = match (text.contains(','), curve) {
        (true, Curve::Bezier(_)) => MODE_BEZIER,
        (true, Curve::Spring { .. }) => MODE_SPRING,
        _ => MODE_PRESET,
    };
    let wanted = curves::normalize(&text);
    let preset = preset_names(&tables)
        .iter()
        .position(|name| curves::normalize(name) == wanted)
        .map_or(-1, |at| at as i32);
    let mut note = String::new();
    if mode == MODE_PRESET
        && tables
            .names()
            .iter()
            .any(|name| curves::normalize(name) == wanted)
    {
        note = format!(
            "This is your registered curve \u{201c}{text}\u{201d}. Pick Bézier or Spring to edit a copy on this event."
        );
    }
    // Umbriel warns about a duration on a spring, which sets its own.
    if matches!(curve, Curve::Spring { .. })
        && curves::defining_doc(&docs, &["animation", event, "duration_ms"]).is_some()
    {
        note.push_str(" This event sets duration_ms, which its spring ignores. Reset removes it.");
    }
    let enabled = |path: &[&str]| docs.iter().rev().find_map(|doc| doc.get_bool(path));
    if !enabled(&["animation", event, "enabled"])
        .or_else(|| enabled(&["animation", "enabled"]))
        .unwrap_or(true)
    {
        note.push_str(" Animation is off for this event in your config; the preview plays anyway.");
    }
    State {
        event,
        text,
        curve,
        mode,
        preset,
        duration_ms: curves::event_timeline(&docs, event).duration_ms,
        note: note.trim().to_owned(),
    }
}

fn view_of(state: &State) -> TimingView {
    let points = match state.curve {
        Curve::Bezier(points) => points,
        _ => BEZIER_START,
    };
    let (damping, stiffness) = match state.curve {
        Curve::Spring { damping, stiffness } => (damping, stiffness),
        _ => SPRING_START,
    };
    let plot = curves::plot(state.curve, PLOT.0, PLOT.1);
    TimingView {
        event: common::slot_label(state.event).into(),
        text: state.text.clone().into(),
        mode: state.mode,
        preset: state.preset,
        x1: points[0] as f32,
        y1: points[1] as f32,
        x2: points[2] as f32,
        y2: points[3] as f32,
        damping: damping as f32,
        stiffness: stiffness as f32,
        duration_ms: state.duration_ms as i32,
        timed: !matches!(state.curve, Curve::Spring { .. }),

        settle_ms: curves::spring_duration_ms(damping, stiffness) as i32,
        path: plot.path.into(),
        ymin: plot.ymin as f32,
        ymax: plot.ymax as f32,
        note: state.note.clone().into(),
    }
}

/// Show the card for the event the preview plays as (or hide it).
pub(super) fn refresh_timing(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let shell = shell.borrow();
    let names: Vec<slint::SharedString> = preset_names(&curves::tables(&chain_docs(&shell)))
        .into_iter()
        .map(Into::into)
        .collect();
    app.set_shader_timing_presets(Rc::new(VecModel::from(names)).into());
    let view = match timing_event(&shell) {
        Some(event) => view_of(&state_of(&shell, event)),
        None => TimingView::default(),
    };
    app.set_shader_timing(view);
}

/// Preview an edit in progress: the card and the preview follow it, the
/// config isn't touched.
fn live(app: &AppWindow, shell: &Rc<RefCell<Shell>>, curve: Curve, duration_ms: Option<u32>) {
    let Some(event) = timing_event(&shell.borrow()) else {
        return;
    };
    let mut state = state_of(&shell.borrow(), event);
    state.curve = curve;
    if let Some(text) = curve.to_config_string() {
        state.text = text;
    }
    if let Some(ms) = duration_ms {
        state.duration_ms = ms;
    }
    let timeline = Timeline {
        curve: state.curve,
        duration_ms: state.duration_ms,
    };
    shell.borrow_mut().shader_preview_timeline = timeline;
    app.set_shader_preview_length_ms(timeline.length_ms() as i32);
    render_preview_at(app, shell, app.get_shader_preview_progress());
    app.set_shader_timing(view_of(&state));
}

/// Write `event`'s `key`s, each through the document that sets it now (if
/// that can't be written, the main config, which wins over includes).
/// One backup run per editor session, not per write.
fn write_event(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    event: &str,
    edits: &[(&str, Value)],
) -> Result<(), String> {
    let mut shell = shell.borrow_mut();
    if !shell.timing_backed_up {
        super::page_backups::snapshot_before_save(
            app,
            &shell,
            &discovery::Env::from_process(),
            "shader",
        );
        shell.timing_backed_up = true;
    }
    let main = shell.includes.docs.len();
    for (key, value) in edits {
        let path = ["animation", event, key];
        if matches!(value, Value::Remove) {
            remove_everywhere(&mut shell, &path)?;
            continue;
        }
        let owner = curves::defining_doc(&chain_docs(&shell), &path);
        let mut failure = String::new();
        let mut written = false;
        for doc in owner.into_iter().chain([main]) {
            let edit = (path.map(String::from).to_vec(), doc, value.clone());
            match write_keys(&mut shell, &[edit]) {
                Ok(()) => {
                    written = true;
                    break;
                }
                Err(err) => failure = err,
            }
        }
        if !written {
            return Err(failure);
        }
    }
    Ok(())
}

/// Remove `path` from every document that sets it (a removal in one
/// would leave another's value in force).
fn remove_everywhere(shell: &mut Shell, path: &[&str]) -> Result<(), String> {
    let owners: Vec<usize> = {
        let docs = chain_docs(shell);
        (0..docs.len())
            .filter(|doc| docs[*doc].get_raw(path).is_some())
            .collect()
    };
    let mut failure = None;
    for doc in owners {
        let edit = (
            path.iter().map(|part| (*part).to_owned()).collect(),
            doc,
            Value::Remove,
        );
        if let Err(err) = write_keys(shell, &[edit]) {
            failure = Some(err);
        }
    }
    failure.map_or(Ok(()), Err)
}

/// The edits that set an event's curve to `text`: a spring sets its own
/// length, so the event's `duration_ms` (which Umbriel warns about next
/// to one) goes.
fn curve_edits(text: &str, tables: &CurveTables) -> Vec<(&'static str, Value)> {
    let mut edits = vec![("curve", Value::Text(text.to_owned()))];
    if matches!(curves::parse(text, tables), Some(Curve::Spring { .. })) {
        edits.push(("duration_ms", Value::Remove));
    }
    edits
}

/// Set the event's curve and reload from the config.
fn commit_curve(app: &AppWindow, shell: &Rc<RefCell<Shell>>, text: &str) {
    let tables = curves::tables(&chain_docs(&shell.borrow()));
    commit(app, shell, &curve_edits(text, &tables));
}

/// Write an edit and reload the preview's timing and the card from the
/// config, which now says it.
fn commit(app: &AppWindow, shell: &Rc<RefCell<Shell>>, edits: &[(&str, Value)]) {
    let Some(event) = timing_event(&shell.borrow()) else {
        return;
    };
    match write_event(app, shell, event, edits) {
        Ok(()) => {
            let index = shell.borrow().shader_preview_event;
            set_preview_event(app, shell, index);
        }
        Err(err) => {
            toast(app, ToastKind::Error, err, "");
            refresh_timing(app, shell);
        }
    }
}

/// An edited curve as the text umbriel will read, and as it will play.
fn rounded(curve: Curve) -> Option<(String, Curve)> {
    let text = curve.to_config_string()?;
    let curve = curves::parse(&text, &CurveTables::default())?;
    Some((text, curve))
}

/// Open the dialog for animation event `index` (into `shaders::EVENTS`),
/// previewing what runs on it: its shader, else a plain fade (Umbriel's
/// built-in animations aren't shaders, so the preview can't run them).
fn open_timing(app: &AppWindow, shell: &Rc<RefCell<Shell>>, index: usize) {
    let Some(event) = shaders::EVENTS.get(index) else {
        return;
    };
    if shaders::slot_kind(event) != shaders::KINDS[0] {
        return;
    }
    let (source, subtitle) = {
        let shell = shell.borrow();
        let assigned = resolved_assignments(&shell)
            .into_iter()
            .nth(index)
            .flatten();
        match assigned
            .as_ref()
            .and_then(|found| Some((found, std::fs::read_to_string(found.path.as_ref()?).ok()?)))
        {
            Some((found, code)) => (code, format!("Runs \u{201c}{}\u{201d}.", found.value)),
            None => (
                shader_preview::FADE.to_owned(),
                "No shader runs this event, so Umbriel plays its built-in animation. The preview uses a plain fade.".to_owned(),
            ),
        }
    };
    {
        // The event's own keys now, for Revert.
        let mut shell = shell.borrow_mut();
        let (curve, duration) = {
            let docs = chain_docs(&shell);
            let own = |key| ["animation", event, key];
            (
                docs.iter()
                    .rev()
                    .find_map(|doc| doc.get_string(&own("curve"))),
                docs.iter()
                    .rev()
                    .find_map(|doc| doc.get_integer(&own("duration_ms"))),
            )
        };
        shell.timing_original = Some((index, curve, duration));
    }
    app.set_shader_timing_title(format!("Timing: {}", slot_label(event)).into());
    app.set_shader_timing_subtitle(subtitle.into());
    start_preview(app, shell, source, index);
    app.set_shader_timing_shown(true);
}

pub(super) fn install(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_timing_open(move |index| {
            if let Some(app) = weak.upgrade() {
                open_timing(&app, &shell, index.max(0) as usize);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_timing_bezier(move |x1, y1, x2, y2, release| {
            let Some(app) = weak.upgrade() else { return };
            let y = |value: f32| f64::from(value.clamp(BEZIER_Y.0, BEZIER_Y.1));
            let x = |value: f32| f64::from(value.clamp(0.0, 1.0));
            let Some((text, curve)) = rounded(Curve::Bezier([x(x1), y(y1), x(x2), y(y2)])) else {
                return;
            };
            if release {
                commit_curve(&app, &shell, &text);
            } else {
                live(&app, &shell, curve, None);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_timing_spring(move |damping, stiffness, release| {
            let Some(app) = weak.upgrade() else { return };
            let curve = Curve::Spring {
                damping: f64::from(damping).clamp(0.01, 5.0),
                stiffness: f64::from(stiffness).clamp(1.0, 10_000.0),
            };
            let Some((text, curve)) = rounded(curve) else {
                return;
            };
            if release {
                commit_curve(&app, &shell, &text);
            } else {
                live(&app, &shell, curve, None);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_timing_duration(move |ms, release| {
            let Some(app) = weak.upgrade() else { return };
            let ms = i64::from(ms).clamp(1, 10_000);
            if release {
                commit(&app, &shell, &[("duration_ms", Value::Int(ms))]);
                return;
            }
            // (Not an `if let` on the borrow: it would live through `live`.)
            let Some(event) = timing_event(&shell.borrow()) else {
                return;
            };
            let curve = state_of(&shell.borrow(), event).curve;
            live(&app, &shell, curve, Some(ms as u32));
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_timing_preset(move |index| {
            let Some(app) = weak.upgrade() else { return };
            let names = preset_names(&curves::tables(&chain_docs(&shell.borrow())));
            if let Some(name) = usize::try_from(index).ok().and_then(|at| names.get(at)) {
                commit_curve(&app, &shell, name);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // A mode starts from something of its kind: the current curve
        // when it already is one, else a sensible default.
        app.on_shader_timing_mode(move |mode| {
            let Some(app) = weak.upgrade() else { return };
            let Some(event) = timing_event(&shell.borrow()) else {
                return;
            };
            let state = state_of(&shell.borrow(), event);
            if mode == state.mode {
                return;
            }
            let text = match mode {
                MODE_BEZIER => rounded(Curve::Bezier(BEZIER_START)),
                MODE_SPRING => rounded(Curve::Spring {
                    damping: SPRING_START.0,
                    stiffness: SPRING_START.1,
                }),
                _ => None,
            }
            .map_or_else(|| "easeout".to_owned(), |(text, _)| text);
            commit_curve(&app, &shell, &text);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_timing_revert(move || {
            let Some(app) = weak.upgrade() else { return };
            let Some((_, curve, duration)) = shell.borrow().timing_original.clone() else {
                return;
            };
            let edits = [
                ("curve", curve.map_or(Value::Remove, Value::Text)),
                ("duration_ms", duration.map_or(Value::Remove, Value::Int)),
            ];
            commit(&app, &shell, &edits);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        // Back to what the config says without this event's own keys:
        // removed wherever they are set.
        app.on_shader_timing_reset(move || {
            let Some(app) = weak.upgrade() else { return };
            let Some(event) = timing_event(&shell.borrow()) else {
                return;
            };
            let mut failure = None;
            for key in ["curve", "duration_ms"] {
                let path = ["animation", event, key];
                if let Err(err) = remove_everywhere(&mut shell.borrow_mut(), &path) {
                    failure = Some(err);
                }
            }
            if let Some(err) = failure {
                toast(&app, ToastKind::Error, err, "");
            }
            let index = shell.borrow().shader_preview_event;
            set_preview_event(&app, &shell, index);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(text: &str, tables: &CurveTables) -> Vec<(&'static str, bool)> {
        curve_edits(text, tables)
            .iter()
            .map(|(key, value)| (*key, matches!(value, Value::Remove)))
            .collect()
    }

    #[test]
    fn a_spring_drops_the_events_duration_and_other_curves_keep_it() {
        let none = CurveTables::default();
        let drops = [("curve", false), ("duration_ms", true)];
        // Inline, and built-in names that are springs.
        assert_eq!(keys("spring:1,900", &none), drops);
        assert_eq!(keys("bouncy", &none), drops);
        assert_eq!(keys("easeout", &none), [("curve", false)]);
        assert_eq!(keys("0.2,0.8,0.2,1", &none), [("curve", false)]);
        // A registered spring counts too.
        let doc: umbriel_config::config::document::ConfigDocument =
            "[animation.springs]\nmyBounce = { damping = 0.5, stiffness = 200 }\n"
                .parse()
                .unwrap();
        let tables = curves::tables(&[&doc]);
        assert_eq!(keys("myBounce", &tables), drops);
    }
}
