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

#[cfg(test)]
mod tests {
    use super::*;

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
}
