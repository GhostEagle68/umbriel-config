//! The `[keybinds]` vocabulary: umbriel's action table for editor dropdowns.
//! Mirrors `actionSpecs()` from umbriel's keybind parser (kept alphabetized
//! like the original); new upstream actions still work via free text — the
//! list is a helper, not a gate.

/// One keybind action: name, parameter hint, and one-line summary.
pub struct Action {
    pub name: &'static str,
    pub param: &'static str,
    pub summary: &'static str,
}

pub const ACTIONS: &[Action] = &[
    Action {
        name: "cheatsheet-close",
        param: "",
        summary: "Hide the keybind cheatsheet",
    },
    Action {
        name: "cheatsheet-open",
        param: "",
        summary: "Show the keybind cheatsheet",
    },
    Action {
        name: "cheatsheet-toggle",
        param: "",
        summary: "Show or hide the keybind cheatsheet",
    },
    Action {
        name: "column-center",
        param: "",
        summary: "Center the focused column in the viewport",
    },
    Action {
        name: "column-focus-first",
        param: "",
        summary: "Focus the first column in the workspace",
    },
    Action {
        name: "column-focus-last",
        param: "",
        summary: "Focus the last column in the workspace",
    },
    Action {
        name: "column-move-left",
        param: "",
        summary: "Move the focused column one position left",
    },
    Action {
        name: "column-move-right",
        param: "",
        summary: "Move the focused column one position right",
    },
    Action {
        name: "column-move-to-first",
        param: "",
        summary: "Move the focused column to the first position",
    },
    Action {
        name: "column-move-to-last",
        param: "",
        summary: "Move the focused column to the last position",
    },
    Action {
        name: "column-move-to-output-down",
        param: "",
        summary: "Move the focused column to the output below",
    },
    Action {
        name: "column-move-to-output-left",
        param: "",
        summary: "Move the focused column to the output left",
    },
    Action {
        name: "column-move-to-output-right",
        param: "",
        summary: "Move the focused column to the output right",
    },
    Action {
        name: "column-move-to-output-up",
        param: "",
        summary: "Move the focused column to the output above",
    },
    Action {
        name: "column-move-to-workspace",
        param: "<workspace>[/<output>]",
        summary: "Move the focused column to the selected workspace",
    },
    Action {
        name: "column-move-to-workspace-next",
        param: "",
        summary: "Move the focused column to the next workspace",
    },
    Action {
        name: "column-move-to-workspace-previous",
        param: "",
        summary: "Move the focused column to the previous workspace",
    },
    Action {
        name: "config-reload",
        param: "",
        summary: "Reload the configuration file",
    },
    Action {
        name: "dpms-off",
        param: "[<output>]",
        summary: "Power off one output, or every output when bare",
    },
    Action {
        name: "dpms-on",
        param: "[<output>]",
        summary: "Power on one output, or every output when bare",
    },
    Action {
        name: "keyboard-layout-next",
        param: "",
        summary: "Switch one keyboard to its next configured layout",
    },
    Action {
        name: "layout-master-count-decrease",
        param: "",
        summary: "Demote the last master window to the stack",
    },
    Action {
        name: "layout-master-count-increase",
        param: "",
        summary: "Promote the first stack window to master",
    },
    Action {
        name: "layout-scroll-down",
        param: "",
        summary: "Scroll the strip toward its end",
    },
    Action {
        name: "layout-scroll-drag",
        param: "",
        summary: "Pan the strip while the bound button is held",
    },
    Action {
        name: "layout-scroll-left",
        param: "",
        summary: "Scroll the strip toward its start",
    },
    Action {
        name: "layout-scroll-right",
        param: "",
        summary: "Scroll the strip toward its end",
    },
    Action {
        name: "layout-scroll-up",
        param: "",
        summary: "Scroll the strip toward its start",
    },
    Action {
        name: "output-focus-down",
        param: "",
        summary: "Focus the output below",
    },
    Action {
        name: "output-focus-left",
        param: "",
        summary: "Focus the output to the left",
    },
    Action {
        name: "output-focus-right",
        param: "",
        summary: "Focus the output to the right",
    },
    Action {
        name: "output-focus-up",
        param: "",
        summary: "Focus the output above",
    },
    Action {
        name: "overview-close",
        param: "",
        summary: "Close the workspace overview",
    },
    Action {
        name: "overview-open",
        param: "",
        summary: "Open the workspace overview",
    },
    Action {
        name: "overview-toggle",
        param: "",
        summary: "Open or close the workspace overview",
    },
    Action {
        name: "scratchpad-focus-next",
        param: "[<output>]",
        summary: "Focus the next visible scratchpad window",
    },
    Action {
        name: "scratchpad-toggle",
        param: "[<output>]",
        summary: "Show or hide the output's scratchpad windows",
    },
    Action {
        name: "session-quit",
        param: "[skip-confirmation]",
        summary: "Quit the session, confirming first unless told to skip",
    },
    Action {
        name: "shortcuts-inhibit-toggle",
        param: "",
        summary: "Toggle shortcuts inhibition for the focused surface",
    },
    Action {
        name: "spawn",
        param: "<cmd>",
        summary: "Run a command with a launch activation token",
    },
    Action {
        name: "submap",
        param: "<name>",
        summary: "Enter a submap layer, or leave one with 'reset'",
    },
    Action {
        name: "window-center",
        param: "",
        summary: "Center the focused floating window on its output",
    },
    Action {
        name: "window-close",
        param: "[<window-id>]",
        summary: "Close the focused window, or the given window",
    },
    Action {
        name: "window-consume-left",
        param: "",
        summary: "Stack the focused window into the column left",
    },
    Action {
        name: "window-consume-or-expel-left",
        param: "",
        summary: "Split the window out, or stack it into the column left",
    },
    Action {
        name: "window-consume-or-expel-right",
        param: "",
        summary: "Split the window out, or stack it into the column right",
    },
    Action {
        name: "window-consume-right",
        param: "",
        summary: "Stack the focused window into the column right",
    },
    Action {
        name: "window-cycle-height",
        param: "",
        summary: "Cycle the focused window through the height presets",
    },
    Action {
        name: "window-cycle-height-back",
        param: "",
        summary: "Cycle the height presets in reverse",
    },
    Action {
        name: "window-cycle-width",
        param: "",
        summary: "Cycle the focused column through the width presets",
    },
    Action {
        name: "window-cycle-width-back",
        param: "",
        summary: "Cycle the width presets in reverse",
    },
    Action {
        name: "window-focus",
        param: "<window-id>",
        summary: "Focus the given window",
    },
    Action {
        name: "window-focus-down",
        param: "",
        summary: "Focus the next window down in the column",
    },
    Action {
        name: "window-focus-last",
        param: "",
        summary: "Focus the previously focused window",
    },
    Action {
        name: "window-focus-left",
        param: "",
        summary: "Focus the window to the left",
    },
    Action {
        name: "window-focus-next",
        param: "",
        summary: "Focus the next window in layout order",
    },
    Action {
        name: "window-focus-or-output-down",
        param: "",
        summary: "Focus down, or the output below at the edge",
    },
    Action {
        name: "window-focus-or-output-left",
        param: "",
        summary: "Focus left, or the output left at the edge",
    },
    Action {
        name: "window-focus-or-output-right",
        param: "",
        summary: "Focus right, or the output right at the edge",
    },
    Action {
        name: "window-focus-or-output-up",
        param: "",
        summary: "Focus up, or the output above at the edge",
    },
    Action {
        name: "window-focus-or-workspace-down",
        param: "",
        summary: "Focus down, or the next workspace at the edge",
    },
    Action {
        name: "window-focus-or-workspace-up",
        param: "",
        summary: "Focus up, or the previous workspace at the edge",
    },
    Action {
        name: "window-focus-previous",
        param: "",
        summary: "Focus the previous window in layout order",
    },
    Action {
        name: "window-focus-right",
        param: "",
        summary: "Focus the window to the right",
    },
    Action {
        name: "window-focus-switch-floating",
        param: "",
        summary: "Focus the last window of the opposite floating state",
    },
    Action {
        name: "window-focus-up",
        param: "",
        summary: "Focus the next window up in the column",
    },
    Action {
        name: "window-focus-warp",
        param: "<window-id>",
        summary: "Focus the given window and warp the cursor to it",
    },
    Action {
        name: "window-modify-height",
        param: "<delta>",
        summary: "Change the focused window's height by a fraction",
    },
    Action {
        name: "window-modify-width",
        param: "<delta>",
        summary: "Change the focused column's width by a fraction",
    },
    Action {
        name: "window-move-down",
        param: "",
        summary: "Move the focused window down in its column",
    },
    Action {
        name: "window-move-or-output-down",
        param: "",
        summary: "Move down, or the column to the output below",
    },
    Action {
        name: "window-move-or-output-left",
        param: "",
        summary: "Move the column left, or to the output left",
    },
    Action {
        name: "window-move-or-output-right",
        param: "",
        summary: "Move the column right, or to the output right",
    },
    Action {
        name: "window-move-or-output-up",
        param: "",
        summary: "Move up, or the column to the output above",
    },
    Action {
        name: "window-move-or-workspace-down",
        param: "",
        summary: "Move down, or to the next workspace at the edge",
    },
    Action {
        name: "window-move-or-workspace-up",
        param: "",
        summary: "Move up, or to the previous workspace at the edge",
    },
    Action {
        name: "window-move-to-output-down",
        param: "",
        summary: "Move the focused window to the output below",
    },
    Action {
        name: "window-move-to-output-left",
        param: "",
        summary: "Move the focused window to the output left",
    },
    Action {
        name: "window-move-to-output-right",
        param: "",
        summary: "Move the focused window to the output right",
    },
    Action {
        name: "window-move-to-output-up",
        param: "",
        summary: "Move the focused window to the output above",
    },
    Action {
        name: "window-move-to-scratchpad",
        param: "[<output>]",
        summary: "Move the focused window into the scratchpad",
    },
    Action {
        name: "window-move-to-workspace",
        param: "<workspace>[/<output>]",
        summary: "Move the focused window to the selected workspace",
    },
    Action {
        name: "window-move-to-workspace-next",
        param: "",
        summary: "Move the focused window to the next workspace",
    },
    Action {
        name: "window-move-to-workspace-previous",
        param: "",
        summary: "Move the focused window to the previous workspace",
    },
    Action {
        name: "window-move-up",
        param: "",
        summary: "Move the focused window up in its column",
    },
    Action {
        name: "window-restore-from-scratchpad",
        param: "[<output>]",
        summary: "Return the scratchpad window to its saved workspace",
    },
    Action {
        name: "window-set-height",
        param: "<fraction>",
        summary: "Set the focused window's height fraction",
    },
    Action {
        name: "window-set-width",
        param: "<fraction>",
        summary: "Set the focused column's width fraction",
    },
    Action {
        name: "window-swap-next",
        param: "",
        summary: "Swap with the next window in layout order",
    },
    Action {
        name: "window-swap-previous",
        param: "",
        summary: "Swap with the previous window in layout order",
    },
    Action {
        name: "window-toggle-floating",
        param: "",
        summary: "Float or tile the focused window",
    },
    Action {
        name: "window-toggle-fullscreen",
        param: "",
        summary: "Toggle fullscreen for the focused window",
    },
    Action {
        name: "window-toggle-maximize",
        param: "",
        summary: "Toggle full width for the focused column",
    },
    Action {
        name: "window-toggle-maximize-to-edges",
        param: "",
        summary: "Toggle maximize without gaps, struts, or borders",
    },
    Action {
        name: "window-toggle-pinned",
        param: "",
        summary: "Pin the focused window above other windows",
    },
    Action {
        name: "window-toggle-scratchpad",
        param: "[<output>]",
        summary: "Move the focused window to or from the scratchpad",
    },
    Action {
        name: "workspace-focus-last",
        param: "",
        summary: "Focus the previously active workspace",
    },
    Action {
        name: "workspace-move-down",
        param: "",
        summary: "Move the focused workspace down the list",
    },
    Action {
        name: "workspace-move-to-output-down",
        param: "",
        summary: "Move every workspace window to the output below",
    },
    Action {
        name: "workspace-move-to-output-left",
        param: "",
        summary: "Move every workspace window to the output left",
    },
    Action {
        name: "workspace-move-to-output-right",
        param: "",
        summary: "Move every workspace window to the output right",
    },
    Action {
        name: "workspace-move-to-output-up",
        param: "",
        summary: "Move every workspace window to the output above",
    },
    Action {
        name: "workspace-move-up",
        param: "",
        summary: "Move the focused workspace up the list",
    },
    Action {
        name: "workspace-next",
        param: "",
        summary: "Switch to the next workspace on this output",
    },
    Action {
        name: "workspace-previous",
        param: "",
        summary: "Switch to the previous workspace on this output",
    },
    Action {
        name: "workspace-set-layout",
        param: "<scrolling|dwindle|master|toggle>",
        summary: "Set the active workspace's layout mode",
    },
    Action {
        name: "workspace-switch",
        param: "<workspace>[/<output>]",
        summary: "Switch to the selected workspace",
    },
];

/// Hover help for the chord field, summarizing umbriel's chord grammar.
pub const CHORD_HINT: &str = "Modifiers: Mod (your mod_key), Ctrl, Alt, Shift, Super/Logo/Win. \
The last token is the key — case-sensitive, e.g. T, Return, XF86AudioRaiseVolume, \
MouseMiddle, WheelUp. Submap-scoped binds: submap[name],chord.";

/// umbriel's built-in default binds, transcribed from the compositor's
/// `defaultKeybinds()`. They stay active underneath the user's config; a
/// user bind with the same chord replaces the default one (umbriel ignores
/// letter case when matching chords). Re-sync this if upstream ever
/// changes its defaults — nothing runtime-readable exposes them.
pub const DEFAULT_BINDS: &[(&str, &str)] = &[
    ("Mod+Escape", "session-quit"),
    ("Mod+Q", "window-close"),
    ("Mod+F1", "window-focus-next"),
    ("Mod+Left", "window-focus-left"),
    ("Mod+H", "window-focus-left"),
    ("Mod+Right", "window-focus-right"),
    ("Mod+L", "window-focus-right"),
    ("Mod+Up", "window-focus-up"),
    ("Mod+K", "window-focus-up"),
    ("Mod+Down", "window-focus-down"),
    ("Mod+J", "window-focus-down"),
    ("Mod+Shift+Left", "column-move-left"),
    ("Mod+Shift+H", "column-move-left"),
    ("Mod+Shift+Right", "column-move-right"),
    ("Mod+Shift+L", "column-move-right"),
    ("Mod+Shift+Up", "window-move-up"),
    ("Mod+Shift+K", "window-move-up"),
    ("Mod+Shift+Down", "window-move-down"),
    ("Mod+Shift+J", "window-move-down"),
    ("Mod+comma", "window-consume-left"),
    ("Mod+period", "window-consume-right"),
    ("Mod+R", "window-cycle-width"),
    ("Mod+Shift+R", "window-cycle-width-back"),
    ("Mod+F", "window-toggle-fullscreen"),
    ("Mod+Ctrl+F", "window-toggle-maximize"),
    ("Mod+M", "window-toggle-maximize-to-edges"),
    ("Mod+T", "window-toggle-floating"),
    ("Mod+P", "window-toggle-pinned"),
    ("Mod+O", "overview-toggle"),
    ("Mod+1", "workspace-switch:1"),
    ("Mod+2", "workspace-switch:2"),
    ("Mod+3", "workspace-switch:3"),
    ("Mod+4", "workspace-switch:4"),
    ("Mod+5", "workspace-switch:5"),
    ("Mod+6", "workspace-switch:6"),
    ("Mod+7", "workspace-switch:7"),
    ("Mod+8", "workspace-switch:8"),
    ("Mod+9", "workspace-switch:9"),
    ("Mod+KP_1", "workspace-switch:1"),
    ("Mod+KP_2", "workspace-switch:2"),
    ("Mod+KP_3", "workspace-switch:3"),
    ("Mod+KP_4", "workspace-switch:4"),
    ("Mod+KP_5", "workspace-switch:5"),
    ("Mod+KP_6", "workspace-switch:6"),
    ("Mod+KP_7", "workspace-switch:7"),
    ("Mod+KP_8", "workspace-switch:8"),
    ("Mod+KP_9", "workspace-switch:9"),
    ("Mod+Shift+1", "window-move-to-workspace:1"),
    ("Mod+Shift+2", "window-move-to-workspace:2"),
    ("Mod+Shift+3", "window-move-to-workspace:3"),
    ("Mod+Shift+4", "window-move-to-workspace:4"),
    ("Mod+Shift+5", "window-move-to-workspace:5"),
    ("Mod+Shift+6", "window-move-to-workspace:6"),
    ("Mod+Shift+7", "window-move-to-workspace:7"),
    ("Mod+Shift+8", "window-move-to-workspace:8"),
    ("Mod+Shift+9", "window-move-to-workspace:9"),
    ("Mod+Shift+KP_1", "window-move-to-workspace:1"),
    ("Mod+Shift+KP_2", "window-move-to-workspace:2"),
    ("Mod+Shift+KP_3", "window-move-to-workspace:3"),
    ("Mod+Shift+KP_4", "window-move-to-workspace:4"),
    ("Mod+Shift+KP_5", "window-move-to-workspace:5"),
    ("Mod+Shift+KP_6", "window-move-to-workspace:6"),
    ("Mod+Shift+KP_7", "window-move-to-workspace:7"),
    ("Mod+Shift+KP_8", "window-move-to-workspace:8"),
    ("Mod+Shift+KP_9", "window-move-to-workspace:9"),
    ("Mod+WheelUp", "window-focus-left"),
    ("Mod+WheelDown", "window-focus-right"),
];

/// Named keys for the chord picker beyond letters and digits:
/// (keysym, label). Any other keysym can be typed by hand.
pub const COMMON_KEYS: &[(&str, &str)] = &[
    ("Return", "Enter"),
    ("Escape", "Esc"),
    ("Tab", "Tab"),
    ("space", "Space"),
    ("BackSpace", "Backspace"),
    ("Delete", "Delete"),
    ("Insert", "Insert"),
    ("Home", "Home"),
    ("End", "End"),
    ("Page_Up", "Page Up"),
    ("Page_Down", "Page Down"),
    ("Left", "Left arrow"),
    ("Right", "Right arrow"),
    ("Up", "Up arrow"),
    ("Down", "Down arrow"),
    ("F1", "F1"),
    ("F2", "F2"),
    ("F3", "F3"),
    ("F4", "F4"),
    ("F5", "F5"),
    ("F6", "F6"),
    ("F7", "F7"),
    ("F8", "F8"),
    ("F9", "F9"),
    ("F10", "F10"),
    ("F11", "F11"),
    ("F12", "F12"),
    // Punctuation, by the keysym names chords use.
    ("minus", "-"),
    ("equal", "="),
    ("bracketleft", "["),
    ("bracketright", "]"),
    ("backslash", "\\"),
    ("semicolon", ";"),
    ("apostrophe", "'"),
    ("comma", ","),
    ("period", "."),
    ("slash", "/"),
    ("grave", "`"),
    ("XF86AudioRaiseVolume", "Volume up"),
    ("XF86AudioLowerVolume", "Volume down"),
    ("XF86AudioMute", "Mute audio"),
    ("XF86AudioMicMute", "Mute microphone"),
    ("XF86AudioPlay", "Play/pause"),
    ("XF86AudioPause", "Pause"),
    ("XF86AudioStop", "Stop"),
    ("XF86AudioNext", "Next track"),
    ("XF86AudioPrev", "Previous track"),
    ("XF86AudioRewind", "Rewind"),
    ("XF86MonBrightnessUp", "Screen brightness up"),
    ("XF86MonBrightnessDown", "Screen brightness down"),
    ("XF86KbdBrightnessUp", "Keyboard brightness up"),
    ("XF86KbdBrightnessDown", "Keyboard brightness down"),
    ("Print", "Print screen"),
    ("Pause", "Pause"),
    ("Scroll_Lock", "Scroll lock"),
    ("Num_Lock", "Num lock"),
    ("Menu", "Menu key"),
    // Mouse buttons and wheel directions (chords like "Mod+MouseMiddle").
    ("MouseLeft", "Left mouse button"),
    ("MouseRight", "Right mouse button"),
    ("MouseMiddle", "Middle mouse button"),
    ("MouseBack", "Back mouse button"),
    ("MouseForward", "Forward mouse button"),
    ("WheelUp", "Wheel up"),
    ("WheelDown", "Wheel down"),
    ("WheelLeft", "Wheel left"),
    ("WheelRight", "Wheel right"),
    // Numpad keys (umbriel distinguishes them from top-row digits).
    ("KP_0", "Numpad 0"),
    ("KP_1", "Numpad 1"),
    ("KP_2", "Numpad 2"),
    ("KP_3", "Numpad 3"),
    ("KP_4", "Numpad 4"),
    ("KP_5", "Numpad 5"),
    ("KP_6", "Numpad 6"),
    ("KP_7", "Numpad 7"),
    ("KP_8", "Numpad 8"),
    ("KP_9", "Numpad 9"),
    ("KP_Enter", "Numpad Enter"),
    ("KP_Add", "Numpad +"),
    ("KP_Subtract", "Numpad -"),
    ("KP_Multiply", "Numpad *"),
    ("KP_Divide", "Numpad /"),
    ("KP_Decimal", "Numpad ."),
];

/// Which page section an action belongs to. Groups follow the action
/// name's family prefix, so new upstream actions usually land in the
/// right section automatically.
pub fn action_group(action: &str) -> &'static str {
    let name = action.split(':').next().unwrap_or(action);
    if name.starts_with("window-focus") || name.starts_with("column-focus") {
        "Focus"
    } else if name.starts_with("window-move")
        || name.starts_with("window-swap")
        || name.starts_with("window-consume")
        || name.starts_with("column-move")
    {
        "Move windows"
    } else if name.starts_with("window-toggle")
        || name.starts_with("window-set")
        || name.starts_with("window-modify")
        || name.starts_with("window-cycle")
        || name == "window-center"
        || name == "window-close"
    {
        "Window state & size"
    } else if name.starts_with("workspace") {
        "Workspaces"
    } else if name.starts_with("scratchpad") {
        "Scratchpad"
    } else if name.starts_with("overview") {
        "Overview"
    } else if name.starts_with("output") {
        "Outputs"
    } else if name.starts_with("layout") {
        "Layout"
    } else if name.starts_with("cheatsheet") {
        "Cheatsheet"
    } else if name == "spawn" {
        "Launch apps"
    } else if matches!(
        name,
        "dpms-off"
            | "dpms-on"
            | "session-quit"
            | "config-reload"
            | "keyboard-layout-next"
            | "shortcuts-inhibit-toggle"
    ) {
        "Session & system"
    } else if name == "submap" {
        "Submaps"
    } else {
        "Other"
    }
}

/// A bind's page section: its action's group, except that commands on
/// media and hardware keys (volume, playback, brightness) get their own
/// section instead of crowding "Launch apps".
pub fn bind_group(chord: &str, action: &str) -> &'static str {
    let group = action_group(action);
    let key = chord.rsplit(['+', ',']).next().unwrap_or(chord);
    if group == "Launch apps" && key.starts_with("XF86") {
        "Media keys"
    } else {
        group
    }
}

/// Every key the chord picker offers, as (keysym, label): letters and
/// digits first, then the named keys.
pub fn key_choices() -> Vec<(String, String)> {
    ('A'..='Z')
        .chain('0'..='9')
        .map(|ch| (ch.to_string(), ch.to_string()))
        .chain(
            COMMON_KEYS
                .iter()
                .map(|(key, label)| ((*key).to_owned(), (*label).to_owned())),
        )
        .collect()
}

/// The chord spelling of a typed character: letters upper-cased, and
/// symbols as their keysym names on the unshifted key (US layout), since
/// umbriel binds keys, not characters — Shift+1 is "Shift+1", not "!".
pub fn key_name(ch: char) -> String {
    const SHIFTED: &str = "!@#$%^&*()_+{}|:\"<>?~";
    const UNSHIFTED: &str = "1234567890-=[]\\;',./`";
    let ch = SHIFTED
        .find(ch)
        .and_then(|i| UNSHIFTED.chars().nth(SHIFTED[..i].chars().count()))
        .unwrap_or(ch);
    let symbol = COMMON_KEYS
        .iter()
        .find(|(key, label)| key.len() > 1 && label.chars().eq([ch]));
    match symbol {
        Some((key, _)) => (*key).to_owned(),
        None if ch == ' ' => "space".to_owned(),
        None => ch.to_uppercase().collect(),
    }
}

/// Usual commands for media and hardware keys: (key prefix, label,
/// command). The editor offers the ones matching the bind's key.
pub const COMMAND_PRESETS: &[(&str, &str, &str)] = &[
    (
        "XF86Audio",
        "Volume up",
        "wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+",
    ),
    (
        "XF86Audio",
        "Volume down",
        "wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-",
    ),
    (
        "XF86Audio",
        "Mute speakers",
        "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle",
    ),
    (
        "XF86Audio",
        "Mute microphone",
        "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle",
    ),
    ("XF86Audio", "Play / pause", "playerctl play-pause"),
    ("XF86Audio", "Next track", "playerctl next"),
    ("XF86Audio", "Previous track", "playerctl previous"),
    ("XF86Audio", "Stop playback", "playerctl stop"),
    (
        "XF86MonBrightness",
        "Brightness up",
        "brightnessctl set 5%+",
    ),
    (
        "XF86MonBrightness",
        "Brightness down",
        "brightnessctl set 5%-",
    ),
    (
        "XF86KbdBrightness",
        "Keyboard backlight up",
        "brightnessctl -d '*::kbd_backlight' set 1+",
    ),
    (
        "XF86KbdBrightness",
        "Keyboard backlight down",
        "brightnessctl -d '*::kbd_backlight' set 1-",
    ),
];

/// The presets for `chord`'s key, as (label, command).
pub fn command_presets(chord: &str) -> Vec<(&'static str, &'static str)> {
    let key = chord.rsplit(['+', ',']).next().unwrap_or(chord);
    COMMAND_PRESETS
        .iter()
        .filter(|(prefix, _, _)| key.starts_with(prefix))
        .map(|(_, label, command)| (*label, *command))
        .collect()
}

/// Display order for the keybind page's groups; unknown groups land at
/// the end via `Other`.
pub const GROUP_ORDER: &[&str] = &[
    "Launch apps",
    "Media keys",
    "Focus",
    "Move windows",
    "Window state & size",
    "Workspaces",
    "Scratchpad",
    "Overview",
    "Outputs",
    "Layout",
    "Cheatsheet",
    "Session & system",
    "Submaps",
    "Other",
];

/// A runtime action parsed from the installed umbriel (owned strings).
#[derive(Debug, Clone, PartialEq)]
pub struct LiveAction {
    pub name: String,
    pub param: String,
    pub summary: String,
}

/// The committed snapshot as owned actions — the fallback whenever the
/// installed umbriel cannot be asked.
pub fn builtin_actions() -> Vec<LiveAction> {
    ACTIONS
        .iter()
        .map(|action| LiveAction {
            name: action.name.to_owned(),
            param: action.param.to_owned(),
            summary: action.summary.to_owned(),
        })
        .collect()
}

/// Parse `umbriel msg --help`: unindented lines are category headers, and
/// action lines read `  name[:<param>]  summary`. Anything unparsable is
/// skipped; an empty result makes callers fall back to the snapshot.
pub fn actions_from_help(text: &str) -> Vec<LiveAction> {
    let mut actions = Vec::new();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        let Some((token, summary)) = rest.split_once("  ") else {
            continue;
        };
        let summary = summary.trim();
        if token.is_empty() || summary.is_empty() {
            continue;
        }
        let (name, param) = match token.split_once(':') {
            Some((name, param)) => (name, param),
            None => (token, ""),
        };
        actions.push(LiveAction {
            name: name.to_owned(),
            param: param.to_owned(),
            summary: summary.to_owned(),
        });
    }
    actions
}

/// Split `submap[name],chord` into (name, chord); ("", chord) otherwise.
pub fn split_scope(chord: &str) -> (&str, &str) {
    match chord
        .strip_prefix("submap[")
        .and_then(|rest| rest.split_once(']'))
    {
        Some((scope, body)) => (scope, body.trim_start_matches(',')),
        None => ("", chord),
    }
}

/// The `[keybinds]` key for `body` scoped to submap `scope` (if any).
pub fn compose_chord(scope: &str, body: &str) -> String {
    match (scope.trim(), body.trim()) {
        ("", body) => body.to_owned(),
        (scope, body) => format!("submap[{scope}],{body}"),
    }
}

/// Whether a chord token is a modifier (Mod, Ctrl, …), in any case.
pub fn is_modifier(token: &str) -> bool {
    ["mod", "ctrl", "alt", "shift", "super", "logo", "win"]
        .iter()
        .any(|name| token.trim().eq_ignore_ascii_case(name))
}

/// Whether `bind` answers to the keys pressed in `pressed`: the same key
/// with at least the pressed modifiers, so pressing P finds Mod+P and
/// Mod+Shift+P, and Mod+P finds only binds holding Mod. A pressed
/// modifier on its own finds every bind that holds it.
pub fn chord_matches(bind: &str, pressed: &str) -> bool {
    let tokens = |chord: &str| -> Vec<String> {
        split_scope(chord)
            .1
            .split('+')
            .map(|token| token.trim().to_lowercase())
            .filter(|token| !token.is_empty())
            .collect()
    };
    let (bind, pressed) = (tokens(bind), tokens(pressed));
    let Some(key) = pressed.last() else {
        return false;
    };
    let wanted = if is_modifier(key) {
        &pressed[..]
    } else {
        if bind.last() != Some(key) {
            return false;
        }
        &pressed[..pressed.len() - 1]
    };
    wanted.iter().all(|modifier| bind.contains(modifier))
}

/// Whether another user bind already owns `chord` (umbriel matches chords
/// ignoring letter case). `skip` is the file-exact chord being edited, if
/// any. Returns the other bind's action, for the warning text.
pub fn find_conflict(
    docs: &[&super::document::ConfigDocument],
    chord: &str,
    skip: Option<&str>,
) -> Option<(usize, String)> {
    let chord = chord.trim().to_ascii_lowercase();
    if chord.is_empty() {
        return None;
    }
    for (file, doc) in docs.iter().enumerate() {
        if let Some(bind) = doc.keybinds().iter().find(|bind| {
            bind.chord.trim().to_ascii_lowercase() == chord
                && skip.is_none_or(|skip| bind.chord.trim() != skip)
        }) {
            return Some((file, bind.action.clone()));
        }
    }
    None
}

/// A user bind and the index of the document that owns it — the winner
/// for its chord under umbriel's precedence: later documents override
/// earlier ones, case-insensitively per chord.
#[derive(Debug, Clone, PartialEq)]
pub struct SourcedBind {
    pub entry: super::document::KeybindEntry,
    pub source_file: usize,
}

impl std::ops::Deref for SourcedBind {
    type Target = super::document::KeybindEntry;

    fn deref(&self) -> &Self::Target {
        &self.entry
    }
}

/// User binds across the include chain and the main file, in umbriel's
/// precedence: per chord, the last document that binds it wins.
pub fn merged_binds(docs: &[&super::document::ConfigDocument]) -> Vec<SourcedBind> {
    let mut merged: Vec<SourcedBind> = Vec::new();
    for (file, doc) in docs.iter().enumerate() {
        for bind in doc.keybinds() {
            let sourced = SourcedBind {
                entry: bind,
                source_file: file,
            };
            match merged
                .iter_mut()
                .find(|existing| existing.chord.eq_ignore_ascii_case(&sourced.chord))
            {
                Some(existing) => *existing = sourced,
                None => merged.push(sourced),
            }
        }
    }
    merged
}

/// Human text for an action string: the live vocabulary's summary with any
/// parameter appended; the raw action when unknown (a spawn command or an
/// action newer than the installed umbriel's list).
pub fn describe(action: &str, actions: &[LiveAction]) -> String {
    let (name, param) = action.split_once(':').unwrap_or((action, ""));
    let summary = actions
        .iter()
        .find(|live| live.name == name)
        .map(|live| live.summary.clone());
    match (summary, param.is_empty()) {
        (Some(summary), true) => summary,
        (Some(summary), false) => format!("{summary} ({param})"),
        (None, _) => action.to_owned(),
    }
}

/// How the editor asks for an action's parameter, read from its spec.
#[derive(Debug, Clone, PartialEq)]
pub enum ParamKind {
    None,
    /// A shell command (`spawn`).
    Command,
    /// A fixed set; an optional parameter includes "" for none.
    Choice(Vec<String>),
    /// Free text with a placeholder hint.
    Text(String),
}

pub fn param_kind(spec: &str) -> ParamKind {
    let spec = spec.trim();
    if spec.is_empty() {
        return ParamKind::None;
    }
    if spec == "<cmd>" {
        return ParamKind::Command;
    }
    let optional = spec.starts_with('[');
    let inner = spec.trim_matches(['[', ']', '<', '>']);
    // `<a|b|c>` lists every value; a bare `[word]` is an optional flag.
    if inner.contains('|') || (optional && !spec.contains('<')) {
        let mut choices: Vec<String> = optional.then(String::new).into_iter().collect();
        choices.extend(inner.split('|').map(str::to_owned));
        return ParamKind::Choice(choices);
    }
    let hint = match inner.split(['>', '[']).next().unwrap_or(inner) {
        "workspace" => "workspace number or name, e.g. 3 or 3/DP-1",
        "output" => "output name, e.g. DP-1",
        "delta" => "signed fraction, e.g. +0.1 or -0.1",
        "fraction" => "fraction from 0.1 to 1.0, e.g. 0.5",
        "window-id" => "window id",
        "name" => "submap name, or reset to leave one",
        _ => spec,
    };
    ParamKind::Text(if optional {
        format!("{hint} (optional)")
    } else {
        hint.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn media_key_commands_get_their_own_group() {
        assert_eq!(
            bind_group("XF86AudioMute", "spawn:wpctl set-mute"),
            "Media keys"
        );
        assert_eq!(
            bind_group("Mod+XF86AudioNext", "spawn:playerctl next"),
            "Media keys"
        );
        assert_eq!(bind_group("Mod+Return", "spawn:kitty"), "Launch apps");
        assert_eq!(bind_group("Mod+Q", "window-close"), "Window state & size");
    }

    #[test]
    fn typed_characters_become_keysyms() {
        assert_eq!(key_name('h'), "H");
        assert_eq!(key_name('!'), "1");
        assert_eq!(key_name(','), "comma");
        assert_eq!(key_name('<'), "comma");
        assert_eq!(key_name('\\'), "backslash");
        assert_eq!(key_name('|'), "backslash");
        assert_eq!(key_name(' '), "space");
    }

    #[test]
    fn presets_follow_the_key() {
        assert_eq!(command_presets("Mod+XF86AudioMute").len(), 8);
        assert_eq!(command_presets("XF86MonBrightnessUp").len(), 2);
        assert!(command_presets("Mod+Return").is_empty());
    }

    #[test]
    fn param_kinds_follow_the_spec() {
        assert_eq!(param_kind(""), ParamKind::None);
        assert_eq!(param_kind("<cmd>"), ParamKind::Command);
        assert_eq!(
            param_kind("<scrolling|dwindle|master|toggle>"),
            ParamKind::Choice(vec![
                "scrolling".into(),
                "dwindle".into(),
                "master".into(),
                "toggle".into()
            ])
        );
        assert_eq!(
            param_kind("[skip-confirmation]"),
            ParamKind::Choice(vec!["".into(), "skip-confirmation".into()])
        );
        assert_eq!(
            param_kind("<workspace>[/<output>]"),
            ParamKind::Text("workspace number or name, e.g. 3 or 3/DP-1".into())
        );
        assert_eq!(
            param_kind("[<output>]"),
            ParamKind::Text("output name, e.g. DP-1 (optional)".into())
        );
        assert_eq!(param_kind("<mystery>"), ParamKind::Text("<mystery>".into()));
    }

    #[test]
    fn parses_msg_help_output() {
        let text = "Usage: umbriel msg <action> [args...]\n\
                    \n\
                    Apps\n  spawn:<cmd>  Run a command\n\
                    \n\
                    Focus\n  window-focus-left        Focus the window to the left\n";
        let actions = actions_from_help(text);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].name, "spawn");
        assert_eq!(actions[0].param, "<cmd>");
        assert_eq!(actions[0].summary, "Run a command");
        assert_eq!(actions[1].name, "window-focus-left");
        assert_eq!(actions[1].param, "");
    }

    #[test]
    fn scopes_split_and_compose() {
        assert_eq!(
            split_scope("submap[resize],Mod+Escape"),
            ("resize", "Mod+Escape")
        );
        assert_eq!(split_scope("Mod+T"), ("", "Mod+T"));
        assert_eq!(compose_chord("resize", "Mod+T"), "submap[resize],Mod+T");
        assert_eq!(compose_chord(" ", "Mod"), "Mod");
    }

    #[test]
    fn pressed_keys_find_binds_holding_them() {
        assert!(chord_matches("Mod+P", "P"));
        assert!(chord_matches("Mod+Shift+P", "Mod+P"));
        assert!(!chord_matches("Mod+P", "Mod+Shift+P"));
        assert!(!chord_matches("Mod+Page_Up", "P"));
        assert!(chord_matches("Mod+Q", "Mod"));
        assert!(chord_matches("submap[resize],Mod+Escape", "escape"));
    }

    #[test]
    fn conflicts_are_case_insensitive_and_skip_the_edited_bind() {
        let doc = crate::config::document::ConfigDocument::from_str(
            "[keybinds]\n\"Mod+T\" = \"window-close\"\n",
        )
        .unwrap();
        assert_eq!(
            find_conflict(&[&doc], "mod+t", None),
            Some((0, "window-close".to_owned()))
        );
        assert_eq!(find_conflict(&[&doc], "Mod+T", Some("Mod+T")), None);
        assert_eq!(find_conflict(&[&doc], "Mod+X", None), None);
    }

    #[test]
    fn merged_binds_last_file_wins_per_chord() {
        let first = crate::config::document::ConfigDocument::from_str(
            "[keybinds]\n\"Mod+T\" = \"window-close\"\n\"Mod+X\" = \"window-close\"\n",
        )
        .unwrap();
        let main = crate::config::document::ConfigDocument::from_str(
            "[keybinds]\n\"mod+t\" = \"config-reload\"\n",
        )
        .unwrap();
        let merged = merged_binds(&[&first, &main]);
        assert_eq!(merged.len(), 2);
        let t = merged.iter().find(|b| b.chord == "mod+t").unwrap();
        assert_eq!((t.source_file, t.action.as_str()), (1, "config-reload"));
        let x = merged.iter().find(|b| b.chord == "Mod+X").unwrap();
        assert_eq!((x.source_file, x.action.as_str()), (0, "window-close"));
    }

    #[test]
    fn describe_uses_live_summaries_and_falls_back() {
        let actions = vec![LiveAction {
            name: "spawn".to_owned(),
            param: "<cmd>".to_owned(),
            summary: "Run a command".to_owned(),
        }];
        assert_eq!(describe("spawn:kitty", &actions), "Run a command (kitty)");
        assert_eq!(describe("spawn", &actions), "Run a command");
        assert_eq!(describe("brand-new:thing", &actions), "brand-new:thing");
    }
}
