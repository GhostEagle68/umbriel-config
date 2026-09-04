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

impl Action {
    /// Dropdown label, e.g. `spawn <cmd>`.
    pub fn label(&self) -> String {
        if self.param.is_empty() {
            self.name.to_owned()
        } else {
            format!("{} {}", self.name, self.param)
        }
    }
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

/// Keysyms for special/media keys, for the chord picker: (keysym, label).
/// Any other keysym can be typed by hand — this list is the common ones.
pub const COMMON_KEYS: &[(&str, &str)] = &[
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
    ("XF86Eject", "Eject"),
    ("XF86Calculator", "Calculator"),
    ("XF86Mail", "Mail"),
    ("XF86Search", "Search"),
    ("XF86HomePage", "Home page"),
    ("XF86Favorites", "Favorites"),
    ("XF86Refresh", "Refresh"),
    ("XF86Tools", "Tools"),
    ("XF86Launch1", "Launch 1"),
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
        "dpms-off" | "dpms-on" | "session-quit" | "config-reload" | "keyboard-layout-next"
    ) {
        "Session & system"
    } else if name == "submap" {
        "Submaps"
    } else {
        "Other"
    }
}

/// Display order for the keybind page's groups; unknown groups land at
/// the end via `Other`.
pub const GROUP_ORDER: &[&str] = &[
    "Launch apps",
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

/// Draft state for the single keybind editor. Nothing here reaches the
/// document until the UI applies it, so intermediate typing is never
/// written.
#[derive(Debug, Clone, Default)]
pub struct BindDraft {
    /// Chord body without the submap scope; may already include "Mod+".
    pub chord: String,
    /// Prepend "Mod+" when composing. The compositor keeps the mod key
    /// for itself while a GUI is focused, so capture adds it here.
    pub use_mod: bool,
    /// Optional `submap[name]` scope; scoped chords only fire in that layer.
    pub scope: String,
    pub action: String,
    pub repeat: Option<bool>,
    pub allow_when_locked: Option<bool>,
    /// Post-action submap transition, or "reset" to exit the layer.
    pub submap: Option<String>,
}

impl BindDraft {
    /// The final `[keybinds]` key: `submap[scope],Mod+chord`. Never
    /// double-prefixes Mod.
    pub fn composed_chord(&self) -> String {
        let body = self.chord.trim();
        let has_mod =
            body.eq_ignore_ascii_case("mod") || body.to_ascii_lowercase().starts_with("mod+");
        let mut chord = if self.use_mod && !has_mod {
            format!("Mod+{body}")
        } else {
            body.to_owned()
        };
        let scope = self.scope.trim();
        if !scope.is_empty() {
            chord = format!("submap[{scope}],{chord}");
        }
        chord
    }

    /// Rebuild draft parts from an existing `[keybinds]` entry.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        chord: &str,
        action: &str,
        repeat: Option<bool>,
        allow_when_locked: Option<bool>,
        submap: Option<String>,
    ) -> Self {
        let (scope, body) = split_scope(chord);
        let use_mod =
            body.eq_ignore_ascii_case("mod") || body.to_ascii_lowercase().starts_with("mod+");
        Self {
            chord: body,
            use_mod,
            scope,
            action: action.to_owned(),
            repeat,
            allow_when_locked,
            submap,
        }
    }
}

/// Split `submap[name],chord` into (name, chord); (empty, chord) otherwise.
fn split_scope(chord: &str) -> (String, String) {
    let Some(rest) = chord.strip_prefix("submap[") else {
        return (String::new(), chord.to_owned());
    };
    let Some((scope, tail)) = rest.split_once(']') else {
        return (String::new(), chord.to_owned());
    };
    (scope.to_owned(), tail.trim_start_matches(',').to_owned())
}

/// Whether another user bind already owns `chord` (umbriel matches chords
/// ignoring letter case). `skip` is the file-exact chord being edited, if
/// any. Returns the other bind's action, for the warning text.
pub fn find_conflict(
    doc: &super::document::ConfigDocument,
    chord: &str,
    skip: Option<&str>,
) -> Option<String> {
    let chord = chord.trim().to_ascii_lowercase();
    if chord.is_empty() {
        return None;
    }
    doc.keybinds()
        .iter()
        .find(|bind| {
            bind.chord.trim().to_ascii_lowercase() == chord
                && skip.is_none_or(|skip| bind.chord.trim() != skip)
        })
        .map(|bind| bind.action.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
    fn composed_chord_folds_scope_and_mod() {
        let mut draft = BindDraft {
            chord: "T".to_owned(),
            use_mod: true,
            ..Default::default()
        };
        assert_eq!(draft.composed_chord(), "Mod+T");
        draft.scope = "resize".to_owned();
        assert_eq!(draft.composed_chord(), "submap[resize],Mod+T");
        // Mod is never double-prefixed, and a bare modifier chord survives.
        draft.chord = "Mod+Shift+T".to_owned();
        draft.use_mod = false;
        assert_eq!(draft.composed_chord(), "submap[resize],Mod+Shift+T");
        draft.chord = "Mod".to_owned();
        draft.use_mod = true;
        draft.scope.clear();
        assert_eq!(draft.composed_chord(), "Mod");
    }

    #[test]
    fn conflicts_are_case_insensitive_and_skip_the_edited_bind() {
        let doc = crate::config::document::ConfigDocument::from_str(
            "[keybinds]\n\"Mod+T\" = \"window-close\"\n",
        )
        .unwrap();
        assert_eq!(
            find_conflict(&doc, "mod+t", None).as_deref(),
            Some("window-close")
        );
        assert_eq!(find_conflict(&doc, "Mod+T", Some("Mod+T")), None);
        assert_eq!(find_conflict(&doc, "Mod+X", None), None);
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

    #[test]
    fn draft_round_trips_scoped_chords() {
        let draft = BindDraft::from_parts(
            "submap[resize],Mod+Escape",
            "submap:reset",
            None,
            None,
            None,
        );
        assert_eq!(draft.scope, "resize");
        assert_eq!(draft.chord, "Mod+Escape");
        assert!(draft.use_mod);
        assert_eq!(draft.composed_chord(), "submap[resize],Mod+Escape");
    }
}
