//! The live preview: which event it plays as (stand-in and timing), and
//! passing scrub positions to the render worker and frames back.

use super::super::common::*;
use super::super::*;
use super::library::*;

/// Park the scrubber mid-animation and hand the freshly loaded code to
/// the preview worker. Not the opening frame: at progress 0 a fade-in
/// is fully transparent and the preview looks broken.
pub(super) fn kick_shader_preview(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    const START: f32 = 0.5;
    app.set_shader_preview_progress(START);
    app.set_shader_preview_note(String::new().into());
    let text = app.get_shader_editor_text().to_string();
    shell
        .borrow_mut()
        .preview_command(shader_preview::PreviewCommand::SetSource(text));
    // Preview as the event the shader is assigned to (the first, when
    // several), else Windows in.
    let event = {
        let shell = shell.borrow();
        shell
            .shader_editing
            .as_deref()
            .and_then(|path| {
                assignments_of(&shell, path)
                    .first()
                    .map(|(event, _)| *event)
            })
            .and_then(|event| shaders::EVENTS.iter().position(|e| *e == event))
            .unwrap_or(0)
    };
    set_preview_event(app, shell, event);
}

/// What an event's shader animates, for the stand-in frame.
pub(super) fn preview_target(event: &str) -> shader_preview::Target {
    use shader_preview::Target;
    match event {
        "workspaces" => Target::Workspace,
        "overview" => Target::Overview,
        "scratchpad" => Target::Scratchpad,
        "layers" => Target::Layer,
        "border" => Target::Border,
        _ => Target::Window,
    }
}

/// Switch the preview to play as `index` (into `shaders::EVENTS`): its
/// stand-in, its timing from the config, and its natural direction.
pub(super) fn set_preview_event(app: &AppWindow, shell: &Rc<RefCell<Shell>>, index: usize) {
    let Some(event) = shaders::EVENTS.get(index) else {
        return;
    };
    let timeline =
        umbriel_config::config::curves::event_timeline(&chain_docs(&shell.borrow()), event);
    {
        let mut shell = shell.borrow_mut();
        shell.shader_preview_event = index;
        shell.shader_preview_timeline = timeline;
        shell.preview_command(shader_preview::PreviewCommand::SetTarget(preview_target(
            event,
        )));
    }
    app.set_shader_preview_event(index as i32);
    app.set_shader_preview_length_ms(timeline.length_ms() as i32);
    // Closing is the only event umbriel runs purely outward.
    app.set_shader_preview_direction(if *event == "windows_out" { -1.0 } else { 1.0 });
    render_preview_at(app, shell, app.get_shader_preview_progress());
}

/// Render at timeline position `linear`, eased by the event's curve.
pub(super) fn render_preview_at(app: &AppWindow, shell: &Rc<RefCell<Shell>>, linear: f32) {
    let linear = linear.clamp(0.0, 1.0);
    let mut shell = shell.borrow_mut();
    let eased = umbriel_config::config::curves::ease(
        shell.shader_preview_timeline.curve,
        f64::from(linear),
    ) as f32;
    shell.preview_command(shader_preview::PreviewCommand::Render {
        linear,
        eased,
        direction: app.get_shader_preview_direction(),
    });
}

/// Apply pending worker events to the UI. Runs on the poll timer; the
/// worker never touches Slint directly.
pub(in crate::slint_ui) fn poll_shader_preview(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let events = match &shell.borrow().shader_preview {
        Some(handle) => handle.drain(),
        None => Vec::new(),
    };
    for event in events {
        match event {
            shader_preview::PreviewEvent::Ready => {
                app.set_shader_preview_ready(true);
            }
            shader_preview::PreviewEvent::Unavailable(err) => {
                let mut shell = shell.borrow_mut();
                shell.shader_preview = None;
                shell.shader_preview_failed = true;
                app.set_shader_preview_ready(false);
                app.set_shader_preview_note(format!("Preview unavailable: {err}").into());
            }
            shader_preview::PreviewEvent::Compiled(problems) => {
                let note = problems.map_or_else(String::new, |err| format!("GLSL error: {err}"));
                app.set_shader_preview_note(note.into());
            }
            shader_preview::PreviewEvent::Frame(width, height, pixels) => {
                let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
                let rgba: Vec<slint::Rgba8Pixel> = pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|chunk| slint::Rgba8Pixel::new(chunk[0], chunk[1], chunk[2], chunk[3]))
                    .collect();
                buffer.make_mut_slice().copy_from_slice(&rgba);
                // Premultiplied, as umbriel shaders return it.
                app.set_shader_preview_image(slint::Image::from_rgba8_premultiplied(buffer));
            }
        }
    }
}

pub(super) fn install(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_scrub(move |progress| {
            let Some(app) = weak.upgrade() else { return };
            render_preview_at(&app, &shell, progress);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_event_picked(move |index| {
            let Some(app) = weak.upgrade() else { return };
            set_preview_event(&app, &shell, index.max(0) as usize);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_direction_toggled(move || {
            let Some(app) = weak.upgrade() else { return };
            let flipped = if app.get_shader_preview_direction() > 0.0 {
                -1.0
            } else {
                1.0
            };
            app.set_shader_preview_direction(flipped);
            render_preview_at(&app, &shell, app.get_shader_preview_progress());
        });
    }
}
