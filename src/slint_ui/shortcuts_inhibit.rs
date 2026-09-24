//! Keyboard shortcuts inhibition while the Keybinds page records keys:
//! Umbriel passes its own shortcuts through to the focused window
//! (zwp_keyboard_shortcuts_inhibit_v1, as games use), so pressing Mod+H
//! records Mod+H instead of moving focus. Binds marked
//! `allow_when_inhibited` still fire.
//!
//! Rides the window's own Wayland connection, which winit owns; outside
//! Wayland, or when the compositor lacks the protocol, recording simply
//! works as before.

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use slint::ComponentHandle;
use wayland_client::backend::{Backend, ObjectId};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat::WlSeat, wl_surface::WlSurface};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1,
    zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1,
};

use super::AppWindow;

/// An active inhibition; dropping it hands shortcuts back to Umbriel.
pub(super) struct Inhibitor {
    conn: Connection,
    inhibitor: ZwpKeyboardShortcutsInhibitorV1,
    // Kept alive with the inhibitor.
    _queue: EventQueue<State>,
}

impl Drop for Inhibitor {
    fn drop(&mut self) {
        self.inhibitor.destroy();
        let _ = self.conn.flush();
    }
}

/// Ask the compositor to pass shortcuts through to `app`'s window. None
/// when that isn't possible here.
pub(super) fn inhibit(app: &AppWindow) -> Option<Inhibitor> {
    let handle = app.window().window_handle();
    let RawDisplayHandle::Wayland(display) = handle.display_handle().ok()?.as_raw() else {
        return None;
    };
    let RawWindowHandle::Wayland(window) = handle.window_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: both pointers come from the live window and stay valid while
    // it exists; the foreign backend and proxy only borrow them.
    let conn = Connection::from_backend(unsafe {
        Backend::from_foreign_display(display.display.as_ptr().cast())
    });
    let surface_id =
        unsafe { ObjectId::from_ptr(WlSurface::interface(), window.surface.as_ptr().cast()) }
            .ok()?;
    let surface = WlSurface::from_id(&conn, surface_id).ok()?;

    let (globals, mut queue) = registry_queue_init::<State>(&conn).ok()?;
    let qh = queue.handle();
    let seat: WlSeat = globals.bind(&qh, 1..=1, ()).ok()?;
    let manager: ZwpKeyboardShortcutsInhibitManagerV1 = globals.bind(&qh, 1..=1, ()).ok()?;
    let inhibitor = manager.inhibit_shortcuts(&surface, &seat, &qh, ());
    manager.destroy();
    queue.roundtrip(&mut State).ok()?;
    Some(Inhibitor {
        conn,
        inhibitor,
        _queue: queue,
    })
}

struct State;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &WlSeat,
        _: <WlSeat as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitManagerV1,
        _: <ZwpKeyboardShortcutsInhibitManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpKeyboardShortcutsInhibitorV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitorV1,
        _: <ZwpKeyboardShortcutsInhibitorV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
