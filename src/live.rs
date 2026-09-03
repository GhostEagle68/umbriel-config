//! Live compositor state, read-only. Connected monitors and their modes
//! arrive over `wlr-output-management-unstable-v1` — the same protocol
//! umbriel's own `umbriel outputs` CLI speaks. Each call opens its own
//! short-lived connection, enumerates until the manager's first `done`
//! event, and disconnects. Nothing here applies changes; writes go through
//! the config file only.

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, protocol::wl_registry};
use wayland_protocols_wlr::output_management::v1::client::{
    zwlr_output_head_v1::{self, ZwlrOutputHeadV1},
    zwlr_output_manager_v1::{self, ZwlrOutputManagerV1},
    zwlr_output_mode_v1::{self, ZwlrOutputModeV1},
};

#[derive(Debug, thiserror::Error)]
pub enum LiveError {
    #[error("could not connect to the Wayland display: {0}")]
    Connect(#[from] wayland_client::ConnectError),
    #[error("Wayland protocol error: {0}")]
    Dispatch(#[from] wayland_client::DispatchError),
    #[error("the compositor does not support wlr-output-management")]
    Unsupported,
}

/// One monitor as the compositor sees it right now.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LiveOutput {
    pub name: String,
    pub description: String,
    pub make: String,
    pub model: String,
    pub enabled: bool,
    pub scale: f64,
    pub position: (i32, i32),
    pub modes: Vec<LiveMode>,
    /// Index into `modes` of the active mode, when reported.
    pub current: Option<usize>,
}

/// One mode of one monitor.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LiveMode {
    pub width: i32,
    pub height: i32,
    /// Millihertz, the protocol's unit; 360 000 = 360 Hz.
    pub refresh_mhz: i32,
    pub preferred: bool,
}

impl LiveMode {
    /// `2560x1440@360`, matching the config `mode` syntax (rounded Hz).
    pub fn label(&self) -> String {
        format!(
            "{}x{}@{:.0}",
            self.width,
            self.height,
            self.refresh_mhz as f64 / 1000.0
        )
    }
}

/// Enumerate connected outputs now.
pub fn outputs() -> Result<Vec<LiveOutput>, LiveError> {
    let conn = Connection::connect_to_env()?;
    let mut queue = conn.new_event_queue::<Enumeration>();
    let handle = queue.handle();
    conn.display().get_registry(&handle, ());
    let mut state = Enumeration::default();
    queue.roundtrip(&mut state)?;
    if state.manager.is_none() {
        return Err(LiveError::Unsupported);
    }
    while !state.done {
        queue.blocking_dispatch(&mut state)?;
    }
    Ok(state.finish())
}

/// Per-connection enumeration state.
#[derive(Default)]
struct Enumeration {
    manager: Option<ZwlrOutputManagerV1>,
    done: bool,
    heads: Vec<Head>,
}

#[derive(Default)]
struct Head {
    live: LiveOutput,
    proxy: Option<ZwlrOutputHeadV1>,
    modes: Vec<Mode>,
}

#[derive(Default)]
struct Mode {
    live: LiveMode,
    proxy: Option<ZwlrOutputModeV1>,
}

impl Enumeration {
    fn finish(self) -> Vec<LiveOutput> {
        self.heads
            .into_iter()
            .map(|head| LiveOutput {
                modes: head.modes.into_iter().map(|mode| mode.live).collect(),
                ..head.live
            })
            .collect()
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Enumeration {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
            && interface == ZwlrOutputManagerV1::interface().name
        {
            let version = version.min(ZwlrOutputManagerV1::interface().version);
            state.manager = Some(registry.bind(name, version, qh, ()));
        }
    }
}

impl Dispatch<ZwlrOutputManagerV1, ()> for Enumeration {
    fn event(
        state: &mut Self,
        _manager: &ZwlrOutputManagerV1,
        event: zwlr_output_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_output_manager_v1::Event::Head { head } => {
                state.heads.push(Head {
                    proxy: Some(head.clone()),
                    ..Head::default()
                });
            }
            zwlr_output_manager_v1::Event::Done { .. } => state.done = true,
            zwlr_output_manager_v1::Event::Finished => {}
            _ => {}
        }
    }

    wayland_client::event_created_child!(Enumeration, ZwlrOutputManagerV1, [
        zwlr_output_manager_v1::EVT_HEAD_OPCODE => (ZwlrOutputHeadV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputHeadV1, ()> for Enumeration {
    fn event(
        state: &mut Self,
        head: &ZwlrOutputHeadV1,
        event: zwlr_output_head_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(slot) = state
            .heads
            .iter_mut()
            .find(|slot| slot.proxy.as_ref() == Some(head))
        else {
            return;
        };
        match event {
            zwlr_output_head_v1::Event::Name { name } => slot.live.name = name,
            zwlr_output_head_v1::Event::Description { description } => {
                slot.live.description = description;
            }
            zwlr_output_head_v1::Event::Make { make } => slot.live.make = make,
            zwlr_output_head_v1::Event::Model { model } => slot.live.model = model,
            zwlr_output_head_v1::Event::Enabled { enabled } => {
                slot.live.enabled = enabled != 0;
            }
            zwlr_output_head_v1::Event::Scale { scale } => slot.live.scale = scale,
            zwlr_output_head_v1::Event::Position { x, y } => {
                slot.live.position = (x, y);
            }
            zwlr_output_head_v1::Event::Mode { mode } => {
                slot.modes.push(Mode {
                    proxy: Some(mode.clone()),
                    ..Mode::default()
                });
            }
            zwlr_output_head_v1::Event::CurrentMode { mode } => {
                slot.live.current = slot
                    .modes
                    .iter()
                    .position(|entry| entry.proxy.as_ref() == Some(&mode));
            }
            zwlr_output_head_v1::Event::PhysicalSize { .. }
            | zwlr_output_head_v1::Event::SerialNumber { .. }
            | zwlr_output_head_v1::Event::AdaptiveSync { .. }
            | zwlr_output_head_v1::Event::Transform { .. }
            | zwlr_output_head_v1::Event::Finished => {}
            _ => {}
        }
    }

    wayland_client::event_created_child!(Enumeration, ZwlrOutputHeadV1, [
        zwlr_output_head_v1::EVT_MODE_OPCODE => (ZwlrOutputModeV1, ()),
    ]);
}

impl Dispatch<ZwlrOutputModeV1, ()> for Enumeration {
    fn event(
        state: &mut Self,
        mode: &ZwlrOutputModeV1,
        event: zwlr_output_mode_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(slot) = state.heads.iter_mut().find_map(|head| {
            head.modes
                .iter_mut()
                .find(|entry| entry.proxy.as_ref() == Some(mode))
        }) else {
            return;
        };
        match event {
            zwlr_output_mode_v1::Event::Size { width, height } => {
                slot.live.width = width;
                slot.live.height = height;
            }
            zwlr_output_mode_v1::Event::Refresh { refresh } => {
                slot.live.refresh_mhz = refresh;
            }
            zwlr_output_mode_v1::Event::Preferred => slot.live.preferred = true,
            zwlr_output_mode_v1::Event::Finished => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_label_matches_config_syntax() {
        let mode = LiveMode {
            width: 2560,
            height: 1440,
            refresh_mhz: 360_000,
            preferred: true,
        };
        assert_eq!(mode.label(), "2560x1440@360");
    }
}
