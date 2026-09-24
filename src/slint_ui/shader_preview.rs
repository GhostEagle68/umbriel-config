//! Offscreen preview of user GLSL against umbriel's animation-shader
//! contract. A dedicated worker thread owns a pbuffer GLES2 context —
//! Slint's own context on the UI thread is never touched — renders the
//! user code over a synthetic test frame, and posts RGBA pixels back.
//! Every failure degrades to a note: the preview must never take the
//! editor (or the machine) down with it.

use std::sync::mpsc;

use glow::HasContext as _;

/// Preview surface size. 16:9 so the panel keeps a window-like shape.
pub const PREVIEW_WIDTH: u32 = 384;
pub const PREVIEW_HEIGHT: u32 = 216;

/// The fragment prefix umbriel prepends to user code (umbrielfx
/// `fx_animation_shader_create`), verbatim. The `#line 1` directives make
/// driver errors carry user-relative line numbers; keep the placement
/// identical or errors point at the wrong lines.
const PREAMBLE: &str = "\
precision highp float;
varying vec2 v_texcoord;
uniform sampler2D umbriel_texture;
uniform mat3 umbriel_sample_matrix;
uniform sampler2D umbriel_previous_texture;
uniform mat3 umbriel_previous_sample_matrix;
uniform float umbriel_progress;
uniform float umbriel_linear_progress;
uniform float umbriel_direction;
uniform vec2 umbriel_size;
uniform vec4 umbriel_random_seed;
#define umbriel_clamped_progress clamp(umbriel_progress, 0.0, 1.0)
vec4 umbriel_sample(vec2 uv) {
  if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) return vec4(0.0);
  vec2 p = (vec3(uv, 1.0) * umbriel_sample_matrix).xy;
  if (any(lessThan(p, vec2(0.0))) || any(greaterThan(p, vec2(1.0)))) return vec4(0.0);
  return texture2D(umbriel_texture, p);
}
#line 1
";
const PREVIOUS_SAMPLE: &str = "\
vec4 umbriel_sample_previous(vec2 uv) {
  if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) return vec4(0.0);
  vec2 p = (vec3(uv, 1.0) * umbriel_previous_sample_matrix).xy;
  if (any(lessThan(p, vec2(0.0))) || any(greaterThan(p, vec2(1.0)))) return vec4(0.0);
  return texture2D(umbriel_previous_texture, p);
}
#line 1
";
/// The suffix umbriel appends: main() runs the user function per pixel,
/// with `v_texcoord` in [0,1]², (0,0) at the window's top-left.
const SUFFIX: &str = "\nvoid main() { gl_FragColor = animation(v_texcoord); }\n";

/// Full fragment source as umbriel would compile it.
pub fn full_source(user_code: &str) -> String {
    format!("{PREAMBLE}{PREVIOUS_SAMPLE}{user_code}{SUFFIX}")
}

/// The preview's own vertex stage: a clip-space quad whose `v_texcoord`
/// matches umbriel's top-left origin. The y flip (0.5 - pos.y * 0.5)
/// compensates glReadPixels returning rows bottom-up.
const VERTEX_SOURCE: &str = "\
attribute vec2 pos;
varying vec2 v_texcoord;
void main() {
    gl_Position = vec4(pos, 0.0, 1.0);
    v_texcoord = vec2(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);
}
";

/// Fixed seed so scrubbing is reproducible (umbriel randomizes per
/// transition; a constant keeps the preview stable while dragging).
const RANDOM_SEED: [f32; 4] = [0.734, 0.151, 0.892, 0.417];

/// The stand-in for what an event's shader actually draws, per
/// umbriel's scene graph (`view.cpp`, `workspace.cpp`, `overview.cpp`,
/// `scratchpad.cpp`, `layer_surface.cpp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A window's content (windows in/out/move, dim unfocused).
    Window,
    /// The output's window tree during a workspace switch: tiled
    /// windows, no wallpaper (that's a separate layer).
    Workspace,
    /// The overview: the zoomed-out filmstrip of workspace cards.
    Overview,
    /// A scratchpad window over its dim backdrop (both shaded).
    Scratchpad,
    /// A layer surface, bar-shaped.
    Layer,
    /// A window's border ring alone; the content is its own target.
    Border,
}

impl Target {
    /// The target's size within the preview surface: bar-shaped for a
    /// layer, the full frame otherwise, so `uv` and `umbriel_size` match
    /// what the shader really gets.
    pub fn size(self, width: u32, height: u32) -> (u32, u32) {
        match self {
            Target::Layer => (width, (width / 12).max(8)),
            _ => (width, height),
        }
    }
}

/// What the UI asks the worker to do.
pub enum PreviewCommand {
    /// Compile this source (umbriel's assembly of prefix + user code).
    SetSource(String),
    /// Swap the stand-in frames.
    SetTarget(Target),
    /// Render at a scrub position: `linear` is the timeline position,
    /// `eased` the curve's value there (overshoot kept), direction +1
    /// (in) or -1 (out) — exactly umbriel's uniforms.
    Render {
        linear: f32,
        eased: f32,
        direction: f32,
    },
}

/// What the worker tells the UI.
pub enum PreviewEvent {
    /// The EGL/GLES context is live; panels can show a frame.
    Ready,
    /// Context creation failed — the preview is off until app restart.
    Unavailable(String),
    /// Compile result: None is clean, Some is a summarized GLSL error.
    Compiled(Option<String>),
    /// A rendered frame: RGBA8, rows top-down.
    Frame(u32, u32, Vec<u8>),
}

/// UI-side handle to the worker thread. Cheap to keep around; sending on
/// a dead worker is a no-op.
pub struct PreviewHandle {
    cmd_tx: mpsc::Sender<PreviewCommand>,
    evt_rx: mpsc::Receiver<PreviewEvent>,
}

impl PreviewHandle {
    pub fn spawn() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (evt_tx, evt_rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("shader-preview".into())
            .spawn(move || run_worker(cmd_rx, evt_tx))
            .expect("spawn shader preview thread");
        Self { cmd_tx, evt_rx }
    }

    pub fn send(&self, cmd: PreviewCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    /// Non-blocking event pump, drained by the UI timer.
    pub fn drain(&self) -> Vec<PreviewEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.evt_rx.try_recv() {
            events.push(event);
        }
        events
    }
}

fn run_worker(cmd_rx: mpsc::Receiver<PreviewCommand>, evt_tx: mpsc::Sender<PreviewEvent>) {
    // glow panics when it cannot query the context version, so wrap init.
    let state = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        PreviewState::new(PREVIEW_WIDTH, PREVIEW_HEIGHT)
    })) {
        Ok(Ok(state)) => Some(state),
        Ok(Err(err)) => {
            let _ = evt_tx.send(PreviewEvent::Unavailable(err));
            None
        }
        Err(panic) => {
            let _ = evt_tx.send(PreviewEvent::Unavailable(panic_reason(&panic)));
            None
        }
    };
    let Some(mut state) = state else {
        // Park: discard commands so senders never see a closed channel.
        while cmd_rx.recv().is_ok() {}
        return;
    };
    let _ = evt_tx.send(PreviewEvent::Ready);
    while let Ok(first) = cmd_rx.recv() {
        // A burst only needs its newest source and newest frame: older
        // sources are already superseded, and their errors would only
        // flash past. The source goes first so the frame uses it.
        let (mut target, mut source, mut render) = (None, None, None);
        for cmd in std::iter::once(first).chain(std::iter::from_fn(|| cmd_rx.try_recv().ok())) {
            match cmd {
                cmd @ PreviewCommand::SetTarget(_) => target = Some(cmd),
                cmd @ PreviewCommand::SetSource(_) => source = Some(cmd),
                cmd @ PreviewCommand::Render { .. } => render = Some(cmd),
            }
        }
        for cmd in [target, source, render].into_iter().flatten() {
            apply(&mut state, &evt_tx, cmd);
        }
    }
}

fn apply(state: &mut PreviewState, evt_tx: &mpsc::Sender<PreviewEvent>, cmd: PreviewCommand) {
    match cmd {
        PreviewCommand::SetSource(source) => match state.compile(&source) {
            Ok(()) => {
                let _ = evt_tx.send(PreviewEvent::Compiled(None));
                // Re-render at the last scrub position so typing and
                // builder changes update the image without a scrub.
                let (linear, eased, direction) = state.last_position();
                match state.render(linear, eased, direction) {
                    Ok((w, h, pixels)) => {
                        let _ = evt_tx.send(PreviewEvent::Frame(w, h, pixels));
                    }
                    Err(err) => {
                        let _ = evt_tx.send(PreviewEvent::Compiled(Some(err)));
                    }
                }
            }
            Err(err) => {
                let _ = evt_tx.send(PreviewEvent::Compiled(Some(err)));
            }
        },
        PreviewCommand::SetTarget(target) => {
            state.set_target(target);
            let (linear, eased, direction) = state.last_position();
            if let Ok((w, h, pixels)) = state.render(linear, eased, direction) {
                let _ = evt_tx.send(PreviewEvent::Frame(w, h, pixels));
            }
        }
        PreviewCommand::Render {
            linear,
            eased,
            direction,
        } => {
            if let Ok((w, h, pixels)) = state.render(linear, eased, direction) {
                let _ = evt_tx.send(PreviewEvent::Frame(w, h, pixels));
            }
        }
    }
}

fn panic_reason(panic: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(msg) = panic.downcast_ref::<&str>() {
        (*msg).to_owned()
    } else if let Some(msg) = panic.downcast_ref::<String>() {
        msg.clone()
    } else {
        "renderer panicked".to_owned()
    }
}

/// Live GL state on the worker thread.
struct PreviewState {
    // Keep the EGL objects alive for the context's lifetime.
    _egl: khronos_egl::DynamicInstance<khronos_egl::EGL1_4>,
    _display: khronos_egl::Display,
    _surface: khronos_egl::Surface,
    _context: khronos_egl::Context,
    gl: glow::Context,
    vertex_shader: glow::Shader,
    quad: glow::Buffer,
    tex_current: glow::Texture,
    tex_previous: glow::Texture,
    program: Option<LinkedProgram>,
    /// Last render: (linear, eased, direction).
    last: (f32, f32, f32),
    /// The current target's size (at most the surface's).
    target_size: (u32, u32),
    width: u32,
    height: u32,
}

struct LinkedProgram {
    program: glow::Program,
    texture: Option<glow::UniformLocation>,
    previous_texture: Option<glow::UniformLocation>,
    progress: Option<glow::UniformLocation>,
    linear_progress: Option<glow::UniformLocation>,
    direction: Option<glow::UniformLocation>,
    size: Option<glow::UniformLocation>,
    random_seed: Option<glow::UniformLocation>,
    sample_matrix: Option<glow::UniformLocation>,
    previous_sample_matrix: Option<glow::UniformLocation>,
}

impl PreviewState {
    fn new(width: u32, height: u32) -> Result<Self, String> {
        // DynamicInstance *is* the API instance: it dlopens libEGL and
        // implements the EGL traits directly.
        let egl = unsafe {
            khronos_egl::DynamicInstance::<khronos_egl::EGL1_4>::load_required()
                .map_err(|err| format!("loading libEGL failed: {err}"))?
        };
        let display = unsafe { egl.get_display(khronos_egl::DEFAULT_DISPLAY) }
            .ok_or_else(|| "no EGL display".to_owned())?;
        egl.initialize(display)
            .map_err(|err| format!("eglInitialize failed: {err}"))?;
        egl.bind_api(khronos_egl::OPENGL_ES_API)
            .map_err(|err| format!("eglBindApi failed: {err}"))?;

        let attribs = [
            khronos_egl::SURFACE_TYPE,
            khronos_egl::PBUFFER_BIT,
            khronos_egl::RENDERABLE_TYPE,
            khronos_egl::OPENGL_ES2_BIT,
            khronos_egl::RED_SIZE,
            8,
            khronos_egl::GREEN_SIZE,
            8,
            khronos_egl::BLUE_SIZE,
            8,
            khronos_egl::ALPHA_SIZE,
            8,
            khronos_egl::NONE,
        ];
        let count = egl
            .matching_config_count(display, &attribs)
            .map_err(|err| format!("eglChooseConfig failed: {err}"))?;
        let mut configs = Vec::with_capacity(count);
        egl.choose_config(display, &attribs, &mut configs)
            .map_err(|err| format!("eglChooseConfig failed: {err}"))?;
        let config = configs
            .into_iter()
            .next()
            .ok_or_else(|| "no pbuffer EGL config".to_owned())?;

        let surface = egl
            .create_pbuffer_surface(
                display,
                config,
                &[
                    khronos_egl::WIDTH,
                    width as khronos_egl::Int,
                    khronos_egl::HEIGHT,
                    height as khronos_egl::Int,
                    khronos_egl::NONE,
                ],
            )
            .map_err(|err| format!("creating the pbuffer failed: {err}"))?;
        let context = egl
            .create_context(
                display,
                config,
                None,
                &[khronos_egl::CONTEXT_CLIENT_VERSION, 2, khronos_egl::NONE],
            )
            .map_err(|err| format!("creating the GLES context failed: {err}"))?;
        egl.make_current(display, Some(surface), Some(surface), Some(context))
            .map_err(|err| format!("binding the GLES context failed: {err}"))?;

        let gl = unsafe {
            glow::Context::from_loader_function(|name| {
                egl.get_proc_address(name)
                    .map_or(std::ptr::null(), |proc| proc as *const core::ffi::c_void)
            })
        };

        let vertex_shader = compile_shader(&gl, glow::VERTEX_SHADER, VERTEX_SOURCE)?;
        let quad = unsafe { gl.create_buffer() }.map_err(|err| err.to_string())?;
        unsafe {
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                &[-1.0f32, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0]
                    .map(f32::to_ne_bytes)
                    .concat(),
                glow::STATIC_DRAW,
            );
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
            gl.disable(glow::BLEND);
            gl.disable(glow::DEPTH_TEST);
        }

        let (current, previous) = test_frames(width, height, Target::Window);
        let tex_current = upload_texture(&gl, width, height, &current)?;
        let tex_previous = upload_texture(&gl, width, height, &previous)?;

        Ok(Self {
            _egl: egl,
            _display: display,
            _surface: surface,
            _context: context,
            gl,
            vertex_shader,
            quad,
            tex_current,
            tex_previous,
            program: None,
            last: (0.0, 0.0, 1.0),
            target_size: (width, height),
            width,
            height,
        })
    }

    fn last_position(&self) -> (f32, f32, f32) {
        self.last
    }

    /// Re-upload both textures with `target`'s stand-in frames.
    fn set_target(&mut self, target: Target) {
        let (tw, th) = target.size(self.width, self.height);
        self.target_size = (tw, th);
        let (current, previous) = test_frames(tw, th, target);
        for (texture, pixels) in [(self.tex_current, &current), (self.tex_previous, &previous)] {
            unsafe {
                self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    tw as i32,
                    th as i32,
                    0,
                    glow::RGBA,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(Some(pixels)),
                );
            }
        }
    }

    /// Compile user code exactly as umbriel assembles it. On success the
    /// new program replaces the old one; on failure the old program
    /// stays live so the preview keeps showing the last good frame.
    fn compile(&mut self, source: &str) -> Result<(), String> {
        let fragment = compile_shader(&self.gl, glow::FRAGMENT_SHADER, &full_source(source))?;
        let program = unsafe { self.gl.create_program() }.map_err(|err| err.to_string())?;
        unsafe {
            self.gl.attach_shader(program, self.vertex_shader);
            self.gl.attach_shader(program, fragment);
            // Fixed attribute slot: no location queries needed per draw.
            self.gl.bind_attrib_location(program, 0, "pos");
            self.gl.link_program(program);
            self.gl.detach_shader(program, self.vertex_shader);
            self.gl.detach_shader(program, fragment);
            self.gl.delete_shader(fragment);
            if !self.gl.get_program_link_status(program) {
                let log = self.gl.get_program_info_log(program);
                self.gl.delete_program(program);
                return Err(summarize_log(&log));
            }
        }
        let linked = unsafe {
            let loc = |name: &str| self.gl.get_uniform_location(program, name);
            LinkedProgram {
                program,
                texture: loc("umbriel_texture"),
                previous_texture: loc("umbriel_previous_texture"),
                progress: loc("umbriel_progress"),
                linear_progress: loc("umbriel_linear_progress"),
                direction: loc("umbriel_direction"),
                size: loc("umbriel_size"),
                random_seed: loc("umbriel_random_seed"),
                sample_matrix: loc("umbriel_sample_matrix"),
                previous_sample_matrix: loc("umbriel_previous_sample_matrix"),
            }
        };
        if let Some(old) = self.program.replace(linked) {
            unsafe { self.gl.delete_program(old.program) };
        }
        Ok(())
    }

    /// Render one frame and read it back (flipped, premultiplied).
    fn render(
        &mut self,
        linear: f32,
        eased: f32,
        direction: f32,
    ) -> Result<(u32, u32, Vec<u8>), String> {
        let linked = self
            .program
            .as_ref()
            .ok_or_else(|| "nothing compiled yet".to_owned())?;
        self.last = (linear, eased, direction);
        let (tw, th) = self.target_size;
        let (w, h) = (tw as i32, th as i32);
        unsafe {
            let gl = &self.gl;
            gl.viewport(0, 0, w, h);
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.use_program(Some(linked.program));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.tex_current));
            gl.uniform_1_i32(linked.texture.as_ref(), 0);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.tex_previous));
            gl.uniform_1_i32(linked.previous_texture.as_ref(), 1);
            gl.uniform_1_f32(linked.progress.as_ref(), eased);
            gl.uniform_1_f32(linked.linear_progress.as_ref(), linear);
            gl.uniform_1_f32(linked.direction.as_ref(), direction);
            gl.uniform_2_f32(linked.size.as_ref(), tw as f32, th as f32);
            gl.uniform_4_f32(
                linked.random_seed.as_ref(),
                RANDOM_SEED[0],
                RANDOM_SEED[1],
                RANDOM_SEED[2],
                RANDOM_SEED[3],
            );
            gl.uniform_matrix_3_f32_slice(
                linked.sample_matrix.as_ref(),
                false,
                &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            );
            gl.uniform_matrix_3_f32_slice(
                linked.previous_sample_matrix.as_ref(),
                false,
                &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            );
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.quad));
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
        let mut pixels = vec![0u8; (tw * th * 4) as usize];
        unsafe {
            self.gl.read_pixels(
                0,
                0,
                w,
                h,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(&mut pixels)),
            );
        }
        Ok((tw, th, finalize_readback(pixels, tw as usize, th as usize)))
    }
}

fn compile_shader(gl: &glow::Context, kind: u32, source: &str) -> Result<glow::Shader, String> {
    let shader = unsafe { gl.create_shader(kind) }.map_err(|err| err.to_string())?;
    unsafe {
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            let log = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            return Err(summarize_log(&log));
        }
    }
    Ok(shader)
}

fn upload_texture(
    gl: &glow::Context,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<glow::Texture, String> {
    let texture = unsafe { gl.create_texture() }.map_err(|err| err.to_string())?;
    unsafe {
        gl.bind_texture(glow::TEXTURE_2D, Some(texture));
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::LINEAR as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
        );
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(pixels)),
        );
    }
    Ok(texture)
}

/// glReadPixels returns bottom-up rows; flip them top-down. The pixels
/// are already premultiplied (umbriel's contract: `animation()` returns
/// premultiplied RGBA, and blending is off), so colour is only clamped
/// to alpha, which keeps a shader that breaks the contract from handing
/// Slint an invalid premultiplied pixel.
pub fn finalize_readback(pixels: Vec<u8>, width: usize, height: usize) -> Vec<u8> {
    let stride = width * 4;
    let mut out = vec![0u8; pixels.len()];
    for y in 0..height {
        let src = &pixels[y * stride..(y + 1) * stride];
        let flipped = height - 1 - y;
        let dst = &mut out[flipped * stride..(flipped + 1) * stride];
        for (dst, src) in dst
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(src.as_chunks::<4>().0.iter())
        {
            let alpha = src[3];
            dst[0] = src[0].min(alpha);
            dst[1] = src[1].min(alpha);
            dst[2] = src[2].min(alpha);
            dst[3] = alpha;
        }
    }
    out
}

/// Driver logs are verbose; keep the first real lines so the note stays
/// one glance long.
fn summarize_log(log: &str) -> String {
    let mut summary = String::new();
    for line in log
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("NOTE:"))
        .take(2)
    {
        if !summary.is_empty() {
            summary.push_str(" · ");
        }
        summary.push_str(line);
    }
    if summary.len() > 200 {
        summary.truncate(200);
        summary.push('…');
    }
    if summary.is_empty() {
        summary.push_str("shader rejected by the driver");
    }
    summary
}

/// The stand-in "window" the shader animates: a titlebar with dots,
/// text lines, and an accent block, so motion (slide, scale, shatter)
/// and fades are all legible. The previous frame is a visibly different
/// variant, making crossfades and `umbriel_sample_previous` readable.
pub fn test_frames(width: u32, height: u32, target: Target) -> (Vec<u8>, Vec<u8>) {
    match target {
        Target::Window => window_frames(width, height),
        Target::Workspace => workspace_frames(width, height),
        Target::Overview => overview_frames(width, height),
        Target::Scratchpad => scratchpad_frames(width, height),
        Target::Layer => layer_frames(width, height),
        Target::Border => border_frames(width, height),
    }
}

fn window_frames(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let mut current = vec![0u8; (width * height * 4) as usize];
    let mut previous = vec![0u8; (width * height * 4) as usize];
    let w = width as i32;
    let h = height as i32;

    // Flat backgrounds; the structure on top is what makes motion legible.
    fill_rect(&mut current, w, h, r(0, 0, w, h), [36, 39, 48, 255]);
    fill_rect(&mut previous, w, h, r(0, 0, w, h), [30, 33, 41, 255]);

    // Titlebar with traffic-light dots.
    fill_rect(&mut current, w, h, r(10, 10, w - 10, 34), [44, 47, 58, 255]);
    fill_rect(
        &mut previous,
        w,
        h,
        r(10, 10, w - 10, 34),
        [50, 54, 66, 255],
    );
    let dots = [[224, 108, 117], [229, 181, 103], [152, 195, 121]];
    for (i, color) in dots.iter().enumerate() {
        let x = 18 + i as i32 * 16;
        fill_rect(
            &mut current,
            w,
            h,
            r(x, 18, x + 8, 26),
            [color[0], color[1], color[2], 255],
        );
        let dimmed = [color[0] / 2, color[1] / 2, color[2] / 2, 255];
        fill_rect(&mut previous, w, h, r(x, 18, x + 8, 26), dimmed);
    }

    // Text lines: staggered widths so offsets and crops stand out.
    let widths = [0.55, 0.42, 0.63, 0.38, 0.57, 0.47];
    for (i, fraction) in widths.iter().enumerate() {
        let y = 50 + i as i32 * 18;
        let text_w = (*fraction * (w - 48) as f32) as i32;
        fill_rect(
            &mut current,
            w,
            h,
            r(24, y, 24 + text_w, y + 8),
            [154, 160, 174, 255],
        );
        // The previous frame's lines sit lower and dimmer.
        fill_rect(
            &mut previous,
            w,
            h,
            r(24, y + 6, 24 + text_w, y + 14),
            [122, 128, 144, 255],
        );
    }

    // An accent block, right-aligned in the current frame and left in
    // the previous one: sliding effects show it swap sides.
    fill_rect(
        &mut current,
        w,
        h,
        r(
            (w as f32 * 0.62) as i32,
            (h as f32 * 0.55) as i32,
            (w as f32 * 0.90) as i32,
            (h as f32 * 0.78) as i32,
        ),
        [124, 154, 255, 255],
    );
    fill_rect(
        &mut previous,
        w,
        h,
        r(
            (w as f32 * 0.10) as i32,
            (h as f32 * 0.55) as i32,
            (w as f32 * 0.38) as i32,
            (h as f32 * 0.78) as i32,
        ),
        [152, 195, 121, 255],
    );

    (current, previous)
}

/// A small window drawn into `rect`: body, titlebar with dots, text
/// lines and an accent block, `tint` scaling its brightness.
fn draw_window(buf: &mut [u8], w: i32, h: i32, rect: Rect, accent: [u8; 3], tint: f32) {
    let shade = |rgb: [u8; 3]| {
        [
            (rgb[0] as f32 * tint) as u8,
            (rgb[1] as f32 * tint) as u8,
            (rgb[2] as f32 * tint) as u8,
            255,
        ]
    };
    let Rect { x0, y0, x1, y1 } = rect;
    fill_rect(buf, w, h, r(x0, y0, x1, y1), shade([36, 39, 48]));
    fill_rect(buf, w, h, r(x0, y0, x1, y0 + 14), shade([48, 52, 64]));
    for (i, dot) in [[224, 108, 117], [229, 181, 103], [152, 195, 121]]
        .iter()
        .enumerate()
    {
        let x = x0 + 6 + i as i32 * 9;
        fill_rect(buf, w, h, r(x, y0 + 5, x + 5, y0 + 10), shade(*dot));
    }
    let inner = (x1 - x0 - 16).max(4);
    for (i, fraction) in [0.7, 0.5, 0.8, 0.45].iter().enumerate() {
        let y = y0 + 22 + i as i32 * 10;
        if y + 5 < y1 - 4 {
            fill_rect(
                buf,
                w,
                h,
                r(x0 + 8, y, x0 + 8 + (inner as f32 * fraction) as i32, y + 5),
                shade([154, 160, 174]),
            );
        }
    }
    let (ax, ay) = (x0 + (x1 - x0) * 3 / 5, y0 + (y1 - y0) * 3 / 5);
    fill_rect(buf, w, h, r(ax, ay, x1 - 8, y1 - 8), shade(accent));
}

/// Three tiled windows with gaps on a transparent background: the
/// window tree a workspace switch shades (the wallpaper is a layer of
/// its own). The previous frame is the other workspace's layout.
fn workspace_frames(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (width as i32, height as i32);
    let gap = 8;
    let mut current = vec![0u8; (width * height * 4) as usize];
    let split = w * 11 / 20;
    draw_window(
        &mut current,
        w,
        h,
        r(gap, gap, split - gap / 2, h - gap),
        [124, 154, 255],
        1.0,
    );
    draw_window(
        &mut current,
        w,
        h,
        r(split + gap / 2, gap, w - gap, h / 2 - gap / 2),
        [229, 181, 103],
        1.0,
    );
    draw_window(
        &mut current,
        w,
        h,
        r(split + gap / 2, h / 2 + gap / 2, w - gap, h - gap),
        [152, 195, 121],
        1.0,
    );
    let mut previous = vec![0u8; (width * height * 4) as usize];
    draw_window(
        &mut previous,
        w,
        h,
        r(gap, gap, w - gap, h - gap),
        [224, 108, 117],
        0.9,
    );
    (current, previous)
}

/// A mini workspace for an overview card: its wallpaper (umbriel shows
/// it per card by default) under its windows.
fn draw_card(
    buf: &mut [u8],
    w: i32,
    h: i32,
    rect: Rect,
    top: [u8; 3],
    bottom: [u8; 3],
    accent: [u8; 3],
) {
    let Rect { x0, y0, x1, y1 } = rect;
    for y in y0..y1 {
        let t = (y - y0) as f32 / (y1 - y0).max(1) as f32;
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
        fill_rect(
            buf,
            w,
            h,
            r(x0, y, x1, y + 1),
            [
                mix(top[0], bottom[0]),
                mix(top[1], bottom[1]),
                mix(top[2], bottom[2]),
                255,
            ],
        );
    }
    let gap = ((x1 - x0) / 24).max(2);
    let split = x0 + (x1 - x0) * 11 / 20;
    draw_window(
        buf,
        w,
        h,
        r(x0 + gap, y0 + gap, split - gap / 2, y1 - gap),
        accent,
        1.0,
    );
    draw_window(
        buf,
        w,
        h,
        r(split + gap / 2, y0 + gap, x1 - gap, y1 - gap),
        [229, 181, 103],
        1.0,
    );
}

/// The overview: a dark backdrop with the filmstrip of workspace cards
/// zoomed out (0.5 by default), the current one centred and outlined,
/// its neighbours cut off at the edges.
fn overview_frames(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (width as i32, height as i32);
    let (cw, ch) = (w / 2, h / 2);
    let step = cw + w / 16;
    let filmstrip = |current_index: i32| {
        let mut buf = vec![0u8; (width * height * 4) as usize];
        fill_rect(&mut buf, w, h, r(0, 0, w, h), [18, 19, 24, 255]);
        let wallpapers = [
            ([58, 44, 92], [22, 60, 88]),
            ([30, 70, 60], [70, 40, 50]),
            ([80, 60, 30], [30, 40, 80]),
        ];
        for (i, (top, bottom)) in wallpapers.iter().enumerate() {
            let x = (w - cw) / 2 + (i as i32 - current_index) * step;
            let y = (h - ch) / 2;
            if i as i32 == current_index {
                fill_rect(
                    &mut buf,
                    w,
                    h,
                    r(x - 3, y - 3, x + cw + 3, y + ch + 3),
                    [124, 154, 255, 255],
                );
            }
            draw_card(
                &mut buf,
                w,
                h,
                r(x, y, x + cw, y + ch),
                *top,
                *bottom,
                [124, 154, 255],
            );
        }
        buf
    };
    (filmstrip(1), filmstrip(0))
}

/// A scratchpad window centred over the dim backdrop umbriel draws
/// behind it (`dim = 0.8` by default); both take the shader.
fn scratchpad_frames(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (width as i32, height as i32);
    let mut current = vec![0u8; (width * height * 4) as usize];
    // Premultiplied black at 80%: the backdrop darkens what's beneath.
    fill_rect(&mut current, w, h, r(0, 0, w, h), [0, 0, 0, 204]);
    let (mw, mh) = (w / 5, h / 6);
    draw_window(
        &mut current,
        w,
        h,
        r(mw, mh, w - mw, h - mh),
        [124, 154, 255],
        1.0,
    );
    let mut previous = vec![0u8; (width * height * 4) as usize];
    fill_rect(&mut previous, w, h, r(0, 0, w, h), [0, 0, 0, 204]);
    (current, previous)
}

/// A layer surface as a bar (workspace dots, a clock, tray icons); the
/// target is the bar itself, so it fills the whole (bar-shaped) frame.
fn layer_frames(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (width as i32, height as i32);
    let unit = (h / 3).max(2);
    let bar = |active: usize, clock: [u8; 4]| {
        let mut buf = vec![0u8; (width * height * 4) as usize];
        fill_rect(&mut buf, w, h, r(0, 0, w, h), [30, 32, 40, 255]);
        for i in 0..5 {
            let x = unit + i as i32 * unit * 2;
            let rgba = if i == active {
                [124, 154, 255, 255]
            } else {
                [90, 96, 112, 255]
            };
            fill_rect(&mut buf, w, h, r(x, unit, x + unit, h - unit), rgba);
        }
        fill_rect(
            &mut buf,
            w,
            h,
            r(w / 2 - unit * 3, unit, w / 2 + unit * 3, h - unit),
            clock,
        );
        for i in 0..3 {
            let x = w - unit * 2 - i * unit * 2;
            fill_rect(
                &mut buf,
                w,
                h,
                r(x, unit, x + unit, h - unit),
                [154, 160, 174, 255],
            );
        }
        buf
    };
    (bar(1, [232, 232, 234, 255]), bar(3, [150, 150, 156, 255]))
}

/// A window's border ring on its own, transparent inside: the border
/// is its own shader target. Accent while focused (current frame),
/// grey when not (previous), so colour fades show.
fn border_frames(width: u32, height: u32) -> (Vec<u8>, Vec<u8>) {
    let (w, h) = (width as i32, height as i32);
    let ring = |rgba: [u8; 4]| {
        let mut buf = vec![0u8; (width * height * 4) as usize];
        let t = 6;
        fill_rect(&mut buf, w, h, r(0, 0, w, t), rgba);
        fill_rect(&mut buf, w, h, r(0, h - t, w, h), rgba);
        fill_rect(&mut buf, w, h, r(0, 0, t, h), rgba);
        fill_rect(&mut buf, w, h, r(w - t, 0, w, h), rgba);
        buf
    };
    (ring([124, 154, 255, 255]), ring([80, 84, 96, 255]))
}

/// Pixel rectangle, exclusive at x1/y1.
struct Rect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

fn r(x0: i32, y0: i32, x1: i32, y1: i32) -> Rect {
    Rect { x0, y0, x1, y1 }
}

fn fill_rect(buf: &mut [u8], w: i32, h: i32, rect: Rect, rgba: [u8; 4]) {
    let Rect { x0, y0, x1, y1 } = rect;
    let x0 = x0.clamp(0, w);
    let x1 = x1.clamp(0, w);
    let y0 = y0.clamp(0, h);
    let y1 = y1.clamp(0, h);
    for y in y0..y1 {
        let base = (y * w * 4) as usize;
        for x in x0..x1 {
            let pixel = &mut buf[base + (x * 4) as usize..base + (x * 4) as usize + 4];
            pixel.copy_from_slice(&rgba);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_source_layers_the_umbriel_contract() {
        let source = full_source("vec4 animation(vec2 uv) { MARKER }");
        let preamble_end = source.find(PREAMBLE.trim_end()).unwrap() + PREAMBLE.trim_end().len();
        let previous_end = source.find(PREVIOUS_SAMPLE.trim_end()).unwrap();
        let marker = source.find("MARKER").unwrap();
        let suffix = source
            .find("void main() { gl_FragColor = animation(v_texcoord); }")
            .unwrap();
        assert!(preamble_end <= previous_end);
        assert!(previous_end < marker);
        assert!(marker < suffix);
        // The contract the docs promise is all present.
        for name in [
            "umbriel_clamped_progress",
            "umbriel_direction",
            "umbriel_size",
            "umbriel_random_seed",
            "umbriel_sample(",
            "umbriel_sample_previous(",
            "umbriel_linear_progress",
        ] {
            assert!(source.contains(name), "missing {name}");
        }
    }

    #[test]
    fn test_frames_are_window_like_and_distinct() {
        let (current, previous) = test_frames(PREVIEW_WIDTH, PREVIEW_HEIGHT, Target::Window);
        assert_eq!(current.len(), (PREVIEW_WIDTH * PREVIEW_HEIGHT * 4) as usize);
        // Deterministic: the same call draws the same pixels.
        let (current2, _) = test_frames(PREVIEW_WIDTH, PREVIEW_HEIGHT, Target::Window);
        assert_eq!(current, current2);
        // Titlebar dot region is red-ish in the current frame.
        let dot = pixel(&current, PREVIEW_WIDTH, 22, 22);
        assert!(dot[0] > 150 && dot[1] < 150);
        // The two frames differ (previous is dimmer there).
        let prev_dot = pixel(&previous, PREVIEW_WIDTH, 22, 22);
        assert!(dot[0] > prev_dot[0]);
        // Accent blocks sit on opposite sides.
        let right = pixel(
            &current,
            PREVIEW_WIDTH,
            PREVIEW_WIDTH * 3 / 4,
            PREVIEW_HEIGHT * 2 / 3,
        );
        assert!(right[2] > 180 && right[2] > right[0]);
        let prev_right = pixel(
            &previous,
            PREVIEW_WIDTH,
            PREVIEW_WIDTH * 3 / 4,
            PREVIEW_HEIGHT * 2 / 3,
        );
        assert_ne!(right, prev_right);
    }

    fn pixel(buf: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let base = ((y * width + x) * 4) as usize;
        [buf[base], buf[base + 1], buf[base + 2], buf[base + 3]]
    }

    #[test]
    fn finalize_readback_flips_and_keeps_premultiplied_colour() {
        // 1×2 image: readback bottom row first (half-alpha), top row
        // second (opaque) — output starts with the flipped opaque row.
        let pixels = vec![20, 10, 30, 50, 10, 20, 30, 255];
        let out = finalize_readback(pixels, 1, 2);
        assert_eq!(out[0..4], [10, 20, 30, 255]);
        // Already premultiplied: not scaled by alpha a second time.
        assert_eq!(out[4..8], [20, 10, 30, 50]);
        // Colour above alpha (not premultiplied) is clamped to it.
        let bad = finalize_readback(vec![200, 40, 90, 60], 1, 1);
        assert_eq!(bad, [60, 40, 60, 60]);
    }

    #[test]
    fn summarize_log_keeps_first_lines() {
        let log = "NOTE: debug\nERROR: 0:4: 'foo' : undeclared identifier\nERROR: 1:9: '}' : syntax error\nmore\n";
        let summary = summarize_log(log);
        assert!(summary.starts_with("ERROR: 0:4"));
        assert!(summary.contains(" · "));
        assert!(!summary.contains("more"));
        let long = "ERROR: ".to_string() + &"x".repeat(400);
        assert!(summarize_log(&long).ends_with('…'));
        assert_eq!(summarize_log("  \n"), "shader rejected by the driver");
    }

    /// Every builder effect compiles, alone and stacked twice (repeats
    /// must not redeclare locals). Skips where no EGL is available.
    #[test]
    fn every_builder_effect_compiles_alone_and_doubled() {
        use umbriel_config::config::shaders::builder;
        let Ok(mut state) = PreviewState::new(32, 24) else {
            return;
        };
        for def in builder::STEP_DEFS {
            let step = def.default_step();
            for stack in [vec![step], vec![step, step]] {
                let code = builder::generate_stack(&stack);
                if let Err(err) = state.compile(&code) {
                    panic!("{} x{} failed: {err}\n{code}", def.label, stack.len());
                }
            }
        }
    }

    /// Every builder effect finishes on the untouched window: at full
    /// progress of an opening (vis = 1) nothing may still be faded,
    /// moved or scattered. Skips where no EGL is available.
    #[test]
    fn every_builder_effect_ends_on_the_plain_window() {
        use umbriel_config::config::shaders::builder;
        let Ok(mut state) = PreviewState::new(64, 36) else {
            return;
        };
        state
            .compile("vec4 animation(vec2 uv) { return umbriel_sample(uv); }")
            .unwrap();
        let (_, _, plain) = state.render(1.0, 1.0, 1.0).unwrap();
        for def in builder::STEP_DEFS {
            state
                .compile(&builder::generate_stack(&[def.default_step()]))
                .unwrap();
            let (_, _, end) = state.render(1.0, 1.0, 1.0).unwrap();
            let worst = end.iter().zip(&plain).map(|(a, b)| a.abs_diff(*b)).max();
            assert!(
                worst <= Some(2),
                "{} ends {worst:?} off the plain window",
                def.label
            );
        }
    }

    /// Full GL round-trip; skips silently where no EGL is available (CI,
    /// headless environments) so the suite stays green everywhere.
    #[test]
    fn render_produces_pixels_and_compile_errors_are_reported() {
        let Ok(mut state) = PreviewState::new(64, 48) else {
            return;
        };
        state
            .compile("vec4 animation(vec2 uv) { return umbriel_sample(uv); }")
            .expect("valid shader compiles");
        let (w, h, pixels) = state.render(0.5, 0.5, 1.0).expect("render");
        assert_eq!((w, h), (64, 48));
        assert_eq!(pixels.len(), 64 * 48 * 4);
        assert!(pixels.iter().any(|&byte| byte != 0));
        let err = state
            .compile("vec4 animation(vec2 uv) { return totally_undeclared; }")
            .expect_err("broken shader must fail");
        assert!(!err.is_empty());
        // The last good program survives a failed compile.
        assert!(state.render(0.0, 0.0, -1.0).is_ok());
        // Switching the stand-in re-uploads the frames at the target's
        // own size: a layer renders bar-shaped, a border ring is
        // transparent inside.
        state.set_target(Target::Layer);
        let (w, h, _) = state.render(1.0, 1.0, 1.0).expect("render");
        assert_eq!((w, h), Target::Layer.size(64, 48));
        state.set_target(Target::Border);
        let (w, h, pixels) = state.render(1.0, 1.0, 1.0).expect("render");
        let alpha = |x: u32, y: u32| pixels[((y * w + x) * 4 + 3) as usize];
        assert_eq!(alpha(1, h / 2), 255);
        assert_eq!(alpha(w / 2, h / 2), 0);
    }
}
