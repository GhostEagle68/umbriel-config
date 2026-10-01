//! The Shaders page: the library with thumbnails, community downloads
//! with update checks, the assignment picker, the visual editor with
//! its preview scrubber. Split by concern; each part registers its own callbacks.

use super::*;

mod builder;
mod download;
mod editor;
mod library;
mod preview;
mod timing;

pub(super) use download::maybe_check_shader_updates;
pub(super) use library::{data_roots, include_preset, rebuild_shaders, scan_shaders};
pub(super) use preview::poll_shader_preview;

pub(super) fn install_shaders(app: &AppWindow, shell: &Rc<RefCell<Shell>>) {
    download::install(app, shell);
    library::install(app, shell);
    editor::install(app, shell);
    builder::install(app, shell);
    preview::install(app, shell);
    timing::install(app, shell);
}
