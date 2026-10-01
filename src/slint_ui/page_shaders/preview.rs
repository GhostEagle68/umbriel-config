//! The live preview: which event it plays as (stand-in and timing), and
//! passing scrub positions to the render worker and frames back.

use super::super::common::*;
use super::super::*;
use super::library::*;

/// Hand the editor's freshly loaded code to the preview, playing as the
/// event the shader is assigned to (the first, when several), else the
/// first event its kind runs as (Windows in, for an animation).
pub(super) fn kick_shader_preview(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let text = app.get_shader_editor_text().to_string();
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
            .or_else(|| {
                let kind = shaders::kind_of(&text);
                shaders::EVENTS
                    .iter()
                    .position(|event| shaders::slot_kind(event) == kind)
            })
            .unwrap_or(0)
    };
    start_preview(app, shell, text, event);
}

/// Park the scrubber mid-animation and play `text` as event `event`
/// (into `shaders::EVENTS`). Not the opening frame: at progress 0 a
/// fade-in is fully transparent and the preview looks broken.
pub(super) fn start_preview(
    app: &AppWindow,
    shell: &Rc<RefCell<Shell>>,
    text: String,
    event: usize,
) {
    const START: f32 = 0.5;
    app.set_shader_preview_progress(START);
    app.set_shader_preview_note(String::new().into());
    {
        let mut shell = shell.borrow_mut();
        shell.preview_command(shader_preview::PreviewCommand::SetSource(text));
        super::strip::send_palette(app, &mut shell);
    }
    set_preview_event(app, shell, event);
}

/// Play library shader `index` (into `shell.shaders`) as event `event`:
/// for the picker's preview and hovering a tile.
fn preview_library_shader(app: &AppWindow, shell: &Rc<RefCell<Shell>>, index: usize, event: usize) {
    let Some(path) = shell
        .borrow()
        .shaders
        .get(index)
        .map(|entry| entry.path.clone())
    else {
        return;
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => start_preview(app, shell, text, event),
        Err(err) => {
            app.set_shader_preview_note(format!("Couldn't read {}: {err}", path.display()).into())
        }
    }
}

/// Thumbnails for every library shader that has none yet: its own
/// `preview.png` when it ships one (community shaders do, for every kind),
/// else a render by the worker, which lands in `poll_shader_preview`.
pub(super) fn request_thumbnails(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    let wanted: Vec<PathBuf> = {
        let mut shell = shell.borrow_mut();
        let wanted: Vec<PathBuf> = shell
            .shaders
            .iter()
            .map(|entry| entry.path.clone())
            .filter(|path| !shell.shader_thumbs_asked.contains(path))
            .collect();
        shell.shader_thumbs_asked.extend(wanted.iter().cloned());
        wanted
    };
    for path in wanted {
        if let Ok(image) = slint::Image::load_from_path(&path.with_file_name("preview.png")) {
            set_thumbnail(app, shell, &path, image);
        } else if let Ok(source) = std::fs::read_to_string(&path) {
            shell
                .borrow_mut()
                .preview_command(shader_preview::PreviewCommand::Thumbnail {
                    path: path.display().to_string(),
                    source,
                });
        }
    }
}

/// Remember a shader's thumbnail and patch its library entry.
fn set_thumbnail(app: &AppWindow, shell: &Rc<RefCell<Shell>>, path: &Path, image: slint::Image) {
    shell
        .borrow_mut()
        .shader_thumbs
        .insert(path.to_path_buf(), image.clone());
    let infos = app.get_shaders();
    let path = path.display().to_string();
    if let Some(list) = infos.as_any().downcast_ref::<VecModel<ShaderInfo>>()
        && let Some(row) = list.iter().position(|info| info.path == path)
        && let Some(mut info) = list.row_data(row)
    {
        info.thumb = image;
        info.has_thumb = true;
        list.set_row_data(row, info);
    }
}

/// Drop a shader's thumbnail after its file changed, so it's drawn again.
pub(super) fn forget_thumbnail(shell: &mut Shell, path: &Path) {
    shell.shader_thumbs.remove(path);
    shell.shader_thumbs_asked.remove(path);
}

/// Worker pixels as a Slint image; premultiplied, as umbriel shaders
/// return them.
fn frame_image(width: u32, height: u32, pixels: &[u8]) -> slint::Image {
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(width, height);
    for (pixel, chunk) in buffer
        .make_mut_slice()
        .iter_mut()
        .zip(pixels.as_chunks::<4>().0)
    {
        *pixel = slint::Rgba8Pixel::new(chunk[0], chunk[1], chunk[2], chunk[3]);
    }
    slint::Image::from_rgba8_premultiplied(buffer)
}

/// What an event's shader animates, for the stand-in frame.
pub(super) fn preview_target(event: &str) -> shader_preview::Target {
    use shader_preview::Target;
    match event {
        "workspaces" => Target::Workspace,
        "overview" => Target::Overview,
        "scratchpad" => Target::Scratchpad,
        "layers" => Target::Layer,
        "border" | "effects.border" => Target::Border,
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
    super::timing::refresh_timing(app, shell);
}

/// Whether the editor's shader is a still picture: not an animation, and
/// reading neither the clock nor the pointer sweep, so playing it can't
/// change a frame and the player's ticks needn't redraw it.
fn is_still(app: &AppWindow) -> bool {
    let text = app.get_shader_editor_text();
    app.get_shader_editor_open()
        && app.get_shader_editor_kind() != 0
        && !text.contains("umbriel_time")
        && !text.contains("umbriel_pointer")
}

/// Render at timeline position `linear`, eased by the event's curve.
pub(super) fn render_preview_at(app: &AppWindow, shell: &Rc<RefCell<Shell>>, linear: f32) {
    let linear = linear.clamp(0.0, 1.0);
    let mut shell = shell.borrow_mut();
    let eased = umbriel_config::config::curves::ease(
        shell.shader_preview_timeline.curve,
        f64::from(linear),
    ) as f32;
    let time = super::strip::preview_clock(app, &shell, app.global::<PreviewClock>().get_seconds());
    shell.preview_command(shader_preview::PreviewCommand::Render {
        linear,
        eased,
        direction: app.get_shader_preview_direction(),
        time,
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
            shader_preview::PreviewEvent::Compiled(log) => {
                // The editor marks problems in its code; elsewhere
                // (the picker) one summary line is all there is room for.
                if app.get_shader_editor_open() {
                    let lines = app.get_shader_editor_text().split('\n').count();
                    let problems: Vec<Diagnostic> = log
                        .as_deref()
                        .map(shaders::diagnostics::parse)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|problem| Diagnostic {
                            line: problem.line.min(lines) as i32,
                            message: problem.message.into(),
                        })
                        .collect();
                    app.set_shader_diagnostics(Rc::new(VecModel::from(problems)).into());
                } else {
                    let note = log.map_or_else(String::new, |log| {
                        format!("GLSL error: {}", shader_preview::summarize_log(&log))
                    });
                    app.set_shader_preview_note(note.into());
                }
            }
            shader_preview::PreviewEvent::Frame(width, height, pixels) => {
                app.set_shader_preview_image(frame_image(width, height, &pixels));
            }
            // A shader that won't compile keeps its placeholder.
            shader_preview::PreviewEvent::Thumbnail(path, Some((width, height, pixels))) => {
                let image = frame_image(width, height, &pixels);
                set_thumbnail(app, shell, Path::new(&path), image);
            }
            shader_preview::PreviewEvent::Thumbnail(_, None) => {}
        }
    }
}

pub(super) fn install(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_thumbs_wanted(move || {
            if let Some(app) = weak.upgrade() {
                request_thumbnails(&app, &shell);
            }
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_file(move |index, event| {
            let Some(app) = weak.upgrade() else { return };
            preview_library_shader(&app, &shell, index.max(0) as usize, event.max(0) as usize);
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_view_changed(move || {
            let Some(app) = weak.upgrade() else { return };
            let env = discovery::Env::from_process();
            let mut settings = app_settings::load(&env);
            settings.shader_grid = app.get_shader_grid();
            settings.shader_hover_preview = app.get_shader_hover_preview();
            settings.shader_sort = app.get_shader_sort();
            store_settings(&app, &env, &settings);
            if shell.borrow().shader_sort == settings.shader_sort {
                return;
            }
            // The order is the model's, so rescan (back to the order
            // found) and sort again. Indices move: forget the picker's.
            // Slint callbacks fire while the model is rebuilt, so the
            // shell is not held across it.
            {
                let mut shell = shell.borrow_mut();
                shell.shader_sort = settings.shader_sort;
                scan_shaders(&mut shell);
            }
            app.set_shader_picker_selected(-2);
            rebuild_shaders(&app, &shell.borrow());
        });
    }
    {
        let weak = app.as_weak();
        let shell = Rc::clone(shell);
        app.on_shader_preview_scrub(move |progress| {
            let Some(app) = weak.upgrade() else { return };
            if is_still(&app) {
                return;
            }
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
