<!-- umbriel-config page: actions.md -->
# Actions

Actions are the compositor's verbs. Bind one under `[keybinds]`, attach it to a
[hot corner](keybinds.md#hot-corners), or run it with `umbriel msg <action>`.
`umbriel msg --help` prints this same list, with the same wording and the same
grouping. See [Keybinds](keybinds.md) for chord syntax and [IPC](ipc.md) for the
socket behind `umbriel msg`, including the event stream that reports what an
action changed.

## Argument forms

An action takes at most one argument, appended after a colon. `<angle>` forms
are required, `[bracket]` forms are optional.

| Form | Meaning |
|------|---------|
| `<cmd>` | Command line, run through the shell: `spawn:kitty` |
| `<name>` | Submap to enter; `submap:reset` leaves one level |
| `<workspace>[/<output>]` | Bare digits select a 1-based position, other text selects a name, and double quotes force a name; append `/output` to scope either form |
| `<window-id>` | Window id from `umbriel windows` |
| `[<window-id>]` | The same id; the bare action targets the focused window |
| `[<output>]` | Connector or monitor name. Bare `dpms-off` and `dpms-on` target every configured output |
| `[<scratchpad>]` | Scratchpad name. The bare form selects the implicit `default` scratchpad, which exists only when no named scratchpads are configured |
| `<fraction>` | `0.1` to `1.0` of the column extent, or of the usable area for a floating window |
| `<delta>` | Signed `-0.9` to `0.9`; the result clamps to `0.1` to `1.0` |
| `<scrolling\|dwindle\|master\|toggle>` | Layout mode for `workspace-set-layout`; `toggle` cycles scrolling, dwindle, master |
| `[skip-confirmation]` | `session-quit` only: quit without the on-screen confirmation |

## Apps

| Action | Effect |
|--------|--------|
| `spawn:<cmd>` | Run a command with a launch activation token |

## Focus

| Action | Effect |
|--------|--------|
| `column-focus-first` | Focus the first column in the workspace |
| `column-focus-last` | Focus the last column in the workspace |
| `output-focus-down` | Focus the output below |
| `output-focus-left` | Focus the output to the left |
| `output-focus-next` | Focus the next output, wrapping around |
| `output-focus-previous` | Focus the previous output, wrapping around |
| `output-focus-right` | Focus the output to the right |
| `output-focus-up` | Focus the output above |
| `window-focus:<window-id>` | Focus the given window |
| `window-focus-down` | Focus the next window down in the column |
| `window-focus-last` | Focus the previously focused window |
| `window-focus-left` | Focus the window to the left |
| `window-focus-next` | Focus the next window in layout order |
| `window-focus-or-output-down` | Focus down, or the output below at the edge |
| `window-focus-or-output-left` | Focus left, or the output left at the edge |
| `window-focus-or-output-right` | Focus right, or the output right at the edge |
| `window-focus-or-output-up` | Focus up, or the output above at the edge |
| `window-focus-or-workspace-down` | Focus down, or the next workspace at the edge |
| `window-focus-or-workspace-up` | Focus up, or the previous workspace at the edge |
| `window-focus-previous` | Focus the previous window in layout order |
| `window-focus-right` | Focus the window to the right |
| `window-focus-switch-floating` | Focus the last window of the opposite floating state |
| `window-focus-up` | Focus the next window up in the column |
| `window-focus-warp:<window-id>` | Focus the given window and warp the cursor to it |
| `workspace-focus-last` | Focus the previously active workspace |

## Move & size

Sizing rules per layout live in [Sizing behavior](layout.md#sizing-behavior).

| Action | Effect |
|--------|--------|
| `column-center` | Center the focused column in the viewport |
| `column-move-left` | Move the focused column one position left |
| `column-move-right` | Move the focused column one position right |
| `column-move-to-first` | Move the focused column to the first position |
| `column-move-to-last` | Move the focused column to the last position |
| `column-move-to-output-down` | Move the focused column to the output below |
| `column-move-to-output-left` | Move the focused column to the output left |
| `column-move-to-output-right` | Move the focused column to the output right |
| `column-move-to-output-up` | Move the focused column to the output above |
| `layout-master-count-decrease` | Demote the last master window to the stack |
| `layout-master-count-increase` | Promote the first stack window to master |
| `layout-scroll-down` | Scroll the strip toward its end |
| `layout-scroll-drag` | Pan the strip while the bound button is held |
| `layout-scroll-left` | Scroll the strip toward its start |
| `layout-scroll-right` | Scroll the strip toward its end |
| `layout-scroll-up` | Scroll the strip toward its start |
| `window-center` | Center the focused floating window on its output |
| `window-consume-left` | Stack the focused window into the column left |
| `window-consume-or-expel-left` | Split the window out, or stack it into the column left |
| `window-consume-or-expel-right` | Split the window out, or stack it into the column right |
| `window-consume-right` | Stack the focused window into the column right |
| `window-cycle-primary-extent` | Cycle the focused area's primary extent through presets |
| `window-cycle-primary-extent-back` | Cycle the primary extent presets in reverse |
| `window-cycle-secondary-extent` | Cycle the focused area's secondary extent through presets |
| `window-cycle-secondary-extent-back` | Cycle the secondary extent presets in reverse |
| `window-modify-height-down:<delta>` | Resize the focused window from its bottom edge |
| `window-modify-height-up:<delta>` | Resize the focused window from its top edge |
| `window-modify-primary-extent:<delta>` | Change the focused area's primary extent by a fraction |
| `window-modify-secondary-extent:<delta>` | Change the focused area's secondary extent by a fraction |
| `window-modify-width-left:<delta>` | Resize the focused column from its left edge |
| `window-modify-width-right:<delta>` | Resize the focused column from its right edge |
| `window-move-down` | Move the focused window down in its column |
| `window-move-or-output-down` | Move down, or the column to the output below |
| `window-move-or-output-left` | Move the column left, or to the output left |
| `window-move-or-output-right` | Move the column right, or to the output right |
| `window-move-or-output-up` | Move up, or the column to the output above |
| `window-move-or-workspace-down` | Move down, or to the next workspace at the edge |
| `window-move-or-workspace-up` | Move up, or to the previous workspace at the edge |
| `window-move-to-output-down` | Move the focused window to the output below |
| `window-move-to-output-left` | Move the focused window to the output left |
| `window-move-to-output-next` | Move the focused window to the next output |
| `window-move-to-output-previous` | Move the focused window to the previous output |
| `window-move-to-output-right` | Move the focused window to the output right |
| `window-move-to-output-up` | Move the focused window to the output above |
| `window-move-up` | Move the focused window up in its column |
| `window-set-primary-extent:<fraction>` | Set the focused area's primary extent fraction |
| `window-set-secondary-extent:<fraction>` | Set the focused area's secondary extent fraction |
| `window-swap-next` | Swap with the next window in layout order |
| `window-swap-previous` | Swap with the previous window in layout order |

## Windows

| Action | Effect |
|--------|--------|
| `window-close:[<window-id>]` | Close the focused window, or the given window |
| `window-toggle-floating:[<window-id>]` | Float or tile the focused window, or the given window |
| `window-toggle-fullscreen` | Toggle fullscreen or exit a window covering the focus |
| `window-toggle-maximize` | Toggle full width for the focused column |
| `window-toggle-maximize-to-edges` | Toggle maximize without gaps, struts, or borders |
| `window-toggle-pinned` | Pin the focused window above other windows |

## Scratchpad

Scratchpads are global named holding areas that roam between outputs.
[Scratchpads](scratchpad.md) covers their configuration, restoration rules, and
multi-output behavior.

| Action | Effect |
|--------|--------|
| `scratchpad-focus-next:[<scratchpad>]` | Focus the next visible scratchpad window |
| `scratchpad-toggle:[<scratchpad>]` | Show or hide the selected scratchpad windows |
| `window-move-to-scratchpad:[<scratchpad>]` | Move the focused window into a scratchpad |
| `window-restore-from-scratchpad:[<scratchpad>]` | Return a scratchpad window to its saved workspace |
| `window-toggle-scratchpad:[<scratchpad>]` | Move the focused window to or from a scratchpad |

## Workspaces

Selector resolution, including forced numeric names and `/output` qualifiers, is
described in [Workspace selectors](workspaces.md#workspace-selectors).

| Action | Effect |
|--------|--------|
| `column-move-to-workspace:<workspace>[/<output>]` | Move the focused column to the selected workspace |
| `column-move-to-workspace-next` | Move the focused column to the next workspace |
| `column-move-to-workspace-previous` | Move the focused column to the previous workspace |
| `window-move-to-workspace:<workspace>[/<output>]` | Move the focused window to the selected workspace |
| `window-move-to-workspace-next` | Move the focused window to the next workspace |
| `window-move-to-workspace-previous` | Move the focused window to the previous workspace |
| `window-move-to-workspace-silent:<workspace>[/<output>]` | Move the focused window to the selected workspace silently |
| `window-move-to-workspace-silent-next` | Move the focused window to the next workspace silently |
| `window-move-to-workspace-silent-previous` | Move the focused window to the previous workspace silently |
| `workspace-move-down` | Move the focused workspace down the list |
| `workspace-move-to-output-down` | Move every workspace window to the output below |
| `workspace-move-to-output-left` | Move every workspace window to the output left |
| `workspace-move-to-output-right` | Move every workspace window to the output right |
| `workspace-move-to-output-up` | Move every workspace window to the output above |
| `workspace-move-up` | Move the focused workspace up the list |
| `workspace-next` | Switch to the next workspace on this output |
| `workspace-previous` | Switch to the previous workspace on this output |
| `workspace-set-layout:<scrolling\|dwindle\|master\|toggle>` | Set the active workspace's layout mode |
| `workspace-swap-active-output-down` | Swap active workspace windows with the output below |
| `workspace-swap-active-output-left` | Swap active workspace windows with the output left |
| `workspace-swap-active-output-next` | Swap active workspace windows with the next output |
| `workspace-swap-active-output-previous` | Swap active workspace windows with the previous output |
| `workspace-swap-active-output-right` | Swap active workspace windows with the output right |
| `workspace-swap-active-output-up` | Swap active workspace windows with the output above |
| `workspace-switch:<workspace>[/<output>]` | Switch to the selected workspace |

## Overview

Dragging windows between previews and creating workspaces by dropping into a gap
are described in [Overview](workspaces-overview.md).

| Action | Effect |
|--------|--------|
| `overview-close` | Close the workspace overview |
| `overview-open` | Open the workspace overview |
| `overview-toggle` | Open or close the workspace overview |

## System

| Action | Effect |
|--------|--------|
| `cheatsheet-close` | Hide the keybind cheatsheet |
| `cheatsheet-open` | Show the keybind cheatsheet |
| `cheatsheet-toggle` | Show or hide the keybind cheatsheet |
| `config-reload` | Reload the configuration file |
| `dpms-off:[<output>]` | Power off one output, or every output when bare |
| `dpms-on:[<output>]` | Power on one output, or every output when bare |
| `keyboard-layout-next` | Switch one keyboard to its next configured layout |
| `session-quit:[skip-confirmation]` | Quit the session, confirming first unless told to skip |
| `shortcuts-inhibit-toggle` | Toggle shortcuts inhibition for the focused surface |
| `submap:<name>` | Enter a submap layer, or leave one with 'reset' |

## Layout differences

Column and extent actions adapt to the active layout:

| Action group | Scrolling | Dwindle | Master |
| --- | --- | --- | --- |
| Column movement | Reorders columns | Swaps neighboring tiles | Exchanges master and stack contents |
| Consume and expel | Joins or splits columns | Swaps directional neighbors | Moves between master and stack |
| Primary extent | Changes column width | Adjusts horizontal splits | Changes master fraction |
| Secondary extent | Changes a row | Adjusts vertical splits | Changes a row |
| Layout scrolling | Pans the strip | No effect | No effect |
| Master count | No effect | No effect | Moves a window between master and stack |

See [Layout](layout.md) for geometry, directions, and resizing behavior.

## Notes

- Output direction actions do not wrap. The `next` and `previous` variants do.
- Workspace `next`, `previous`, `move-up`, and `move-down` do not wrap.
- A whole-column move preserves order, proportions, and column extent.
- Moving a multi-window column into Dwindle creates separate tiles.
- Floating and pinned behavior is described in [Layout](layout.md) and
  [Scratchpads](scratchpad.md).
- An action unavailable in the active layout does nothing from a keybind and
  returns an explanatory error through `umbriel msg`.
- `spawn:` supplies an activation token so the launched application can request
  focus. Autostart commands do not receive one.
- Bare `session-quit` asks for confirmation. Use
  `session-quit:skip-confirmation` only when an immediate exit is intended.

<!-- umbriel-config page: animation.md -->
# Animation

Animation settings live under `[animation]`. The top-level values provide
defaults, and each event can override them.

```toml
[animation]
enabled = true
duration_ms = 250
curve = "easeout"

[animation.windows_in]
enabled = true
curve = "spring:1,900"
style = "popin"
scale = 0.5

[animation.windows_out]
enabled = true
curve = "spring:1,1400"
style = "popin"
scale = 0.8

[animation.windows_move]
enabled = true
curve = "spring:1,900"

[animation.workspaces]
enabled = true
curve = "spring:1,800"

[animation.overview]
enabled = true
curve = "spring:1,800"
workspace_curve = "spring:1,1000"

[animation.scratchpad]
enabled = true
curve = "spring:1,800"
dim = 0.8
blur = false
scale = 0.0
maximize = false
fullscreen = false

[animation.border]
enabled = true
curve = "spring:1,900"

[animation.dim_unfocused]
enabled = false
dim = 0.0

[animation.layers]
enabled = false
```

## Defaults

| Key | Default | Description |
| --- | --- | --- |
| `enabled` | `true` | Master switch for every transition. |
| `duration_ms` | `250` | Default duration for non-spring curves. |
| `curve` | `"easeout"` | Default easing curve. |

Each event also accepts `enabled`, `duration_ms`, and `curve`. A spring curve
chooses its own duration, so `duration_ms` has no effect on that event.

## Event tables

| Table | Additional fields | Transition |
| --- | --- | --- |
| `[animation.windows_in]` | `style`, `scale` | Window opening |
| `[animation.windows_out]` | `style`, `scale` | Window closing |
| `[animation.windows_move]` | none | Move, resize, reflow, maximize, and restore |
| `[animation.workspaces]` | none | Workspace switching |
| `[animation.overview]` | `workspace_curve` | Overview opening, closing, and filmstrip movement |
| `[animation.scratchpad]` | `dim`, `blur`, `scale`, `maximize`, `fullscreen` | Scratchpad windows and backdrop |
| `[animation.border]` | none | Focus-border color |
| `[animation.dim_unfocused]` | `dim` | Unfocused-window opacity |
| `[animation.layers]` | none | Layer-shell map and unmap |

`windows_in` accepts `popin`, `zoom`, `slide`, `fade`, or `none`.
`windows_out` accepts `fade`, `slide`, `popin`, or `zoom`. `scale` applies to
`popin`: an opening window grows from it to full size, and a closing window
shrinks toward it, while both fade.

`animation.overview.workspace_curve` controls filmstrip movement after wheel,
keyboard, and touchpad navigation.

Scratchpad `dim` and `blur` remain active without a fade when animation is
disabled. `scale`, `maximize`, and `fullscreen` set the presentation applied
when a window enters a scratchpad.

## Curves

Use a built-in curve such as `linear`, `ease`, `easeout`, `snappy`, `bounce`, or
`elastic`; a cubic Bézier string; or a spring:

```toml
curve = "0.05,0.9,0.1,1.0"
# Or use a spring:
# curve = "spring:1,1000"
```

For Bézier curves, x coordinates must be between 0 and 1. Spring syntax is
`spring:<damping>,<stiffness>`:

- Damping below 1 overshoots.
- Damping 1 reaches the target without overshoot.
- Damping above 1 approaches more slowly.
- Greater stiffness settles faster.

Register reusable names when several events share a curve:

```toml
[animation.beziers]
myBezier = [0.05, 0.9, 0.1, 1.05]

[animation.springs]
myBounce = { damping = 0.5, stiffness = 200 }
```

Then set `curve = "myBezier"` or `curve = "myBounce"`.

## Custom GLSL shaders

Every animation event can use a custom fragment shader. The event's enabled
state and curve still control its timeline.

Umbriel ships `reveal.glsl` and `squash.glsl`. Reference the installed files
directly:

```toml
[animation.windows_in]
duration_ms = 300
curve = "easeout"
shader = "/usr/share/umbriel/shaders/reveal.glsl"

[animation.windows_out]
duration_ms = 250
curve = "easeout"
shader = "/usr/share/umbriel/shaders/reveal.glsl"

[animation.windows_move]
shader = "/usr/share/umbriel/shaders/squash.glsl"
```

Adjust `/usr/share` for the package prefix. Relative paths resolve from the
configuration file containing the setting. Shader files are watched and reload
with the configuration.

NixOS users can derive the path from the configured package:

```nix
{
  programs.umbriel.settings.animation.windows_in.shader =
    "${config.programs.umbriel.package}/share/umbriel/shaders/reveal.glsl";
}
```

The `shader` value must name a regular GLSL file smaller than 256 KiB. Inline
GLSL and recursive includes are not supported.

### Shader interface

Write GLSL ES 1.00 with this entry point. Do not add a `#version` declaration
or your own `main`:

```glsl
vec4 animation(vec2 uv) {
    return umbriel_sample(uv);
}
```

Umbriel supplies `main`, precision declarations, and these commonly used
values:

| Name | Meaning |
| --- | --- |
| `uv` | Normalized target coordinates |
| `umbriel_sample(vec2 uv)` | Sample the rendered target |
| `umbriel_sample_previous(vec2 uv)` | Sample this target's previous shader result |
| `umbriel_size` | Target width and height in logical units |
| `umbriel_progress` | Eased progress, including overshoot |
| `umbriel_clamped_progress` | Eased progress clamped to 0 through 1 |
| `umbriel_linear_progress` | Progress before easing |
| `umbriel_direction` | `1` for entering and `-1` for leaving |
| `umbriel_random_seed` | Four stable random values for this transition |

Return premultiplied RGBA. Preserve sampled alpha when modifying colors so a
shader does not fill transparent parts of its target.

`umbriel_sample_previous` enables feedback and allocates two additional buffers
for the active target. Avoid it when an effect does not need feedback,
especially for workspace and overview shaders.

### Targets and composition

Window shaders process the window, subsurfaces, and border as one target.
Workspace and overview shaders process their corresponding scene trees.
Shaders change presentation only; they do not affect layout, client sizes,
input coordinates, or focus.

Window shadows follow the alpha shape produced by window and border shaders.
The compositor still applies configured color, softness, and offset.

### Reload and failures

Shaders compile on startup or configuration reload. A missing source or compile
failure produces a diagnostic and falls back to the built-in effect. Compiler
details appear in the Umbriel log.

Custom shaders are trusted local GPU code. Expensive or nonterminating shaders
can stall the driver, and active effects disable direct scanout. Prefer short,
inexpensive effects.

<!-- umbriel-config page: appearance.md -->
# Appearance

Configure Umbriel's colors, window decorations, blur, and shadows.

## Colors

```toml
[colors]
background = "#141419FF"
text_primary = "#E8E8EAFF"
text_muted = "#8A8A92FF"
accent_primary = "#7AA3FFFF"
accent_secondary = "#F5C96BFF"
warning = "#F5C96BFF"
error = "#FF6B6BFF"
insert_hint = "#7FC8FF80"
backdrop = "#000000FF"
shadow = "#0000007F"
```

Colors use `#RRGGBB` or `#RRGGBBAA`.

| Key | Description |
| --- | --- |
| `background` | Background for Umbriel panels and banners. |
| `text_primary` | Primary text. |
| `text_muted` | Secondary help and status text. |
| `accent_primary` | Titles, key chords, and primary emphasis. |
| `accent_secondary` | Secondary emphasis and group headings. |
| `warning` | Warning text and borders. |
| `error` | Error text and confirmation borders. |
| `insert_hint` | Drop-target preview during dragging. |
| `backdrop` | Fullscreen background and RGB color of the opaque emergency lock blank. |
| `shadow` | Window shadow color. |

### Border colors

```toml
[colors.border]
focused = "#7AA3FFFF"
unfocused = "#292933FF"
scratchpad_focused = "#E5C07BFF"
scratchpad_unfocused = "#5C4A2AFF"
outer = "#1A1A1FFF"
```

The first four values select focused and unfocused colors for regular and
scratchpad windows. `outer` colors the optional outer border.

### Overview colors

```toml
[colors.overview]
background_tint = "#10101430"
workspace_background = "#00000044"
badge = "#7AA3FFFF"
```

| Key | Description |
| --- | --- |
| `background_tint` | Tint over the desktop behind the overview. |
| `workspace_background` | Background behind each workspace preview. |
| `badge` | Shortcut badge color. |

See [Workspaces Overview](workspaces-overview.md#settings-and-behavior) for
overview behavior.

## Window appearance

```toml
[appearance]
prefer_no_csd = true
border_width = 2
outer_border_width = 0
corner_radius = 10
drag_opacity = 0.75
opaque_fullscreen = true
```

| Key | Default | Description |
| --- | --- | --- |
| `prefer_no_csd` | `true` | Prefer Umbriel's border-only server decoration. |
| `border_width` | `2` | Inner border width in logical pixels. |
| `outer_border_width` | `0` | Outer ring width in logical pixels. |
| `corner_radius` | `10` | Radius of the complete decorated window. |
| `drag_opacity` | `0.75` | Opacity while dragging a window. |
| `opaque_fullscreen` | `true` | Draw fullscreen windows over the backdrop and ignore window rule `opacity`. |

With `opaque_fullscreen = false`, a fullscreen window that is translucent shows
the desktop behind it instead of the backdrop, and can be blurred. A window is
translucent when its rule opacity is below 1 or the application itself draws
transparent content. Other fullscreen windows stay opaque and skip blur.

Set `prefer_no_csd = false` to let newly connected applications draw their own
decorations. Restart applications after changing it because decoration protocol
availability is fixed when an application connects.

Borders render outside window content and are included in layout spacing.
`corner_radius = 0` keeps every contour square.

### Blur

```toml
[appearance.blur]
enabled = true
optimized = true
passes = 3
radius = 5
noise = 0.02
brightness = 0.9
contrast = 0.9
saturation = 1.1
```

`enabled` is the master switch. Individual surfaces still opt in through
[window rules](window-rules.md) or [layer rules](layer-rules.md). Blur appears
only where a surface is transparent.

| Key | Default | Description |
| --- | --- | --- |
| `enabled` | `true` | Enable blur rendering. |
| `optimized` | `true` | Share one cached background blur across surfaces on an output. |
| `passes` | `3` | Blur passes from 0 to 8. |
| `radius` | `5` | Blur radius from 0 to 100. |
| `noise` | `0.02` | Noise overlay from 0.0 to 1.0. |
| `brightness` | `0.9` | Brightness multiplier from 0.0 to 2.0. |
| `contrast` | `0.9` | Contrast multiplier from 0.0 to 2.0. |
| `saturation` | `1.1` | Saturation multiplier from 0.0 to 2.0. |

Optimized blur samples the background beneath the window stack. Set it to
`false` when translucent surfaces should blur the surfaces directly behind
them, at a higher rendering cost.

### Shadow

```toml
[appearance.shadow]
enabled = true
softness = 10
offset_x = 2
offset_y = 2
```

| Key | Default | Description |
| --- | --- | --- |
| `enabled` | `true` | Draw shadows behind tiled and floating windows. |
| `softness` | `10` | Blur softness from 0 to 200. |
| `offset_x` | `2` | Horizontal offset from -200 to 200. |
| `offset_y` | `2` | Vertical offset from -200 to 200. |

A window's shadow falls on everything below it, including other floating,
pinned, or scratchpad windows it overlaps. Tiled windows never shadow each
other. Shadows are hidden for fullscreen windows. During a
[custom window animation](animation.md#custom-glsl-shaders), the shadow follows
the visible shape produced by the shader.

<!-- umbriel-config page: configuration.md -->
# Configuration

## Starting configuration

The packaged starting configuration is
[`examples/config.toml`](https://github.com/noctalia-dev/umbriel/blob/main/examples/config.toml).
Copy it before making local changes:

```sh
mkdir -p ~/.config/umbriel
cp /usr/share/umbriel/config.toml ~/.config/umbriel/config.toml
```

A manual installation under `/usr/local` places it at
`/usr/local/share/umbriel/config.toml`. Nix users should prefer
`programs.umbriel.settings` in Home Manager or hjem.

Umbriel watches the active configuration and applies valid changes when you
save. Most options reload immediately; reference tables identify options that
require a restart. If a reload fails, Umbriel keeps the last working
configuration and tries again on the next save.

Without `-c`, Umbriel checks these locations in order:

1. `$XDG_CONFIG_HOME/umbriel/config.toml`
2. Each `$XDG_CONFIG_DIRS/umbriel/config.toml`
3. The packaged `share/umbriel/config.toml`

Use `umbriel -c <path>` to select one exact file. Umbriel never creates or
modifies a user configuration automatically.

## Diagnostics

Configuration warnings and errors appear in a panel on the primary output.
Errors keep the previous working configuration active. A warning-only panel
closes after ten seconds; an error remains until the next successful reload.

Check a file without starting the compositor:

```sh
umbriel validate
```

The command prints every diagnostic with its file, line, and column, and exits
nonzero when it finds a problem.

## Include

Split a configuration into smaller files with required or optional includes:

```toml
[include]
files = [
  "appearance.toml",
  "keybinds.toml",
]

[include.optional]
files = [
  "~/.config/umbriel/noctalia.toml",
]
```

Paths are relative to the file containing the include. `~`, `$VAR`, and
`${VAR}` are expanded. Missing optional files are ignored and watched, so
creating one later reloads the configuration.

Included files are applied in list order. The including file is applied last:

- Tables merge by key.
- Rule lists such as `[[window_rule]]` and `[[workspace]]` collect entries.
- Plain arrays and scalar values are replaced by the last file that sets them.
- Setting a rule list to `[]` discards entries collected earlier.

Every file must contain valid TOML. Duplicate device or workspace selectors are
still errors when they come from different files.

If any included file defines `[drm]`, also declare `[drm]` in the main file.
This prevents an incomplete GPU exclusion policy from loading when an include
is unavailable.

## General

```toml
[general]
autostart = ["noctalia", "kitty"]
mod_key = "Super"
xwayland = true
show_cheatsheet = true
focus_on_activate = false
honor_restored_maximize = false
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `autostart` | string array | `[]` | Commands started once with the session. Changes require a restart. |
| `mod_key` | string | Super, or Alt when nested | Modifier represented by `Mod` in keybinds. |
| `xwayland` | bool | `true` | Start xwayland-satellite for X11 applications. Changes require a restart. |
| `show_cheatsheet` | bool | `true` | Show the keybind cheatsheet when Umbriel starts. |
| `focus_on_activate` | bool | `false` | Let application activation requests focus and reveal their target. |
| `honor_restored_maximize` | bool | `false` | Honor maximize requests an application makes while its window opens, until it acknowledges its opening layout. |

`xwayland-satellite` must be installed and available on `PATH` when X11 support
is enabled.

## DRM devices

Use `[drm]` only when Umbriel should leave a GPU unopened in a native session.
Omit the section for automatic GPU discovery. Changes require a restart.

```toml
[drm]
ignored_pci_addresses = ["0000:01:00.0"]
# ignored_devices = ["/dev/dri/by-path/pci-0000:01:00.0-card"]
```

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `ignored_devices` | string array | `[]` | Absolute card or render-node paths. Prefer stable `/dev/dri/by-path` links. |
| `ignored_pci_addresses` | string array | `[]` | PCI addresses in `domain:bus:slot.function` form. |

Umbriel does not bind or unbind PCI drivers. Configure that lifecycle in
libvirt or equivalent host tooling.

### Limits

- GPU exclusions affect native sessions only.
- Startup fails when no allowed GPU can initialize.
- Secondary GPUs must support the primary GPU's DMA-BUF formats and modifiers.
- Software rendering is incompatible with GPU exclusions.
- With exclusions, `WLR_BACKENDS` supports only `drm` and optional `libinput`.

## Environment

```toml
[environment]
ELECTRON_OZONE_PLATFORM_HINT = "auto"
SDL_VIDEODRIVER = "wayland"
```

These variables apply to Umbriel and commands started in its session. In a
managed native session, they are also published to the systemd user manager for
session services such as Noctalia. Nested sessions do not modify the host
session environment.

Names must match `[A-Za-z_][A-Za-z0-9_]*`, and values must be strings. Umbriel
owns its display and session variables, so this section cannot override
`WAYLAND_DISPLAY`, `WAYLAND_SOCKET`, `DISPLAY`, `UMBRIEL_SOCKET`,
`XDG_CURRENT_DESKTOP`, `XDG_SESSION_DESKTOP`, or `XDG_SESSION_TYPE`.

Environment changes require an Umbriel restart. Fully quit and relaunch
long-running applications that survived the restart.

Removing a key does not clear a value already published to the systemd user
manager. Run `systemctl --user unset-environment NAME` to remove it immediately,
or wait for the user manager to exit.

## Events

Run commands when the laptop lid closes or opens:

```toml
[events]
lid_close = "notify-send 'The laptop lid is closed!'"
lid_open = "notify-send 'The laptop lid is open!'"
```

## Scratchpads

With no `[[scratchpad]]` entries, Umbriel provides one implicit scratchpad named
`default`:

```toml
[keybinds]
"Mod+Shift+Space" = "window-move-to-scratchpad"
"Mod+Space" = "scratchpad-toggle"
```

To configure several scratchpads, give each one a unique name and include that
name in its actions:

```toml
[[scratchpad]]
name = "terminal"

[[scratchpad]]
name = "music"

[keybinds]
"Mod+Space" = "scratchpad-toggle:terminal"
"Mod+M" = "scratchpad-toggle:music"
```

Defining a named scratchpad removes the implicit `default`. See
[Scratchpads](scratchpad.md) for window assignment and behavior.

## Idle inhibition

Umbriel honors idle inhibitors only while their application surface is mapped
and visible. Switching away from its workspace, hiding it in a scratchpad,
disabling its output, or locking the session suspends the inhibitor until the
surface becomes visible again.

<!-- umbriel-config page: index.md -->
# Umbriel

Umbriel is a Wayland compositor with scrolling and tiling layouts, independent
workspaces per monitor, and configurable visual effects. It works on its own or
with [Noctalia](https://docs.noctalia.dev/noctalia/), a desktop shell designed
to integrate with Umbriel.

> Umbriel is young and actively evolving. Configuration and behavior may change
> between releases, and you may encounter rough edges.

## Start here

1. [Install Umbriel](installation.md).
2. [Copy and edit the starting configuration](configuration.md#starting-configuration).
3. Configure your [outputs](outputs.md), [input devices](input.md), and
   [keybinds](keybinds.md).
4. Choose a [layout](layout.md) and [workspace model](workspaces.md).
5. Use [window rules](window-rules.md) for application-specific behavior.

Umbriel reloads most configuration changes when you save the file. Errors and
warnings appear on screen, and `umbriel validate` can check a configuration
without a running session.

## Features

- Scrolling, Dwindle, and Master layouts
- Independent workspaces and configuration per output
- Floating, pinned, fullscreen, and [scratchpad](scratchpad.md) windows
- Configurable keybinds, gestures, window rules, blur, shadows, and animations
- X11 application support through xwayland-satellite
- Local [IPC](ipc.md) for scripts, panels, and runtime inspection

## Help and contributing

Bug reports are welcome. Feature requests are considered against the project's
[scope statement](https://github.com/noctalia-dev/umbriel/blob/main/SCOPE.md).
For general help and design discussion, join the community on
[Discord](https://discord.noctalia.dev).

<!-- umbriel-config page: input.md -->
# Input

Configure keyboard, pointer, touchpad, tablet, cursor, and focus behavior under
`[input]`.

## Settings

```toml
[input]
middle_click_paste = false
window_drag_toggle = "none"
```

`middle_click_paste = false` disables primary-selection paste, including
Shift+Insert. The regular Ctrl+C and Ctrl+V clipboard is unaffected.
Applications started while primary selection is disabled must be restarted
after it is re-enabled.

`window_drag_toggle` controls what pressing the other main mouse button does
during a window drag:

| Value | Behavior |
| --- | --- |
| `"none"` | Leave the drag unchanged. |
| `"floating"` | Toggle whether the dropped window is tiled or floating. |
| `"pinned"` | Toggle whether the dropped window is pinned. |

The state changes when the window is dropped. Unsupported transitions, such as
pinning a fullscreen window, leave the window unchanged.

### Keyboard

```toml
[input.keyboard]
layout = ""           # empty uses the system default
variant = ""
options = ""
repeat_rate = 25
repeat_delay = 600
numlock_toggle = true
track_layout = "global"
```

| Key | Range or values | Description |
| --- | --- | --- |
| `repeat_rate` | 0 to 1000 Hz | Key repeats per second; `0` disables repeat. |
| `repeat_delay` | 0 to 10000 ms | Delay before a held key repeats. |
| `numlock_toggle` | bool | Enable NumLock when a keyboard connects. |
| `track_layout` | `"global"` or `"window"` | Choose session-wide or per-window layout tracking. |

Use a comma-separated layout list to configure several layouts:

```toml
[input.keyboard]
layout = "us,de"
options = "grp:alt_shift_toggle"
```

`options` accepts XKB options such as `caps:escape` or `compose:ralt`. Invalid
layouts and variants are reported in the log and fall back to the system
default. Run `xkbcli list` to inspect available values.

Run `umbriel keyboard-layouts` to list the configured layouts. The active one
is prefixed with `*`.

#### Layout switching

Bind `keyboard-layout-next` to cycle configured physical keyboard layouts:

```toml
[keybinds]
"Mod+Shift+K" = "keyboard-layout-next"
```

`umbriel msg keyboard-layout-next` provides the same action for scripts.
Physical keyboards that provide the selected layout stay synchronized. Virtual
keyboards retain their application-provided keymap.

#### Tracking the layout per window

`track_layout` accepts:

| Value | Behavior |
| --- | --- |
| `"global"` | Use one active layout for the session. |
| `"window"` | Remember the active layout for each focused surface. |

In window mode, a surface that has not been focused before starts with the
first configured layout. Reloading keyboard configuration clears remembered
surface layouts.

### Touchpad

```toml
[input.touchpad]
tap = true
natural_scroll = true
# accel_profile = "adaptive"
# sensitivity = 0.5
# scroll_factor = 1.5
# disable_while_typing = true
# disable_on_external_mouse = true
# click_method = "clickfinger"
```

Omitted values preserve the device's libinput defaults. Explicit unsupported
settings are reported in the log.

| Key | Description |
| --- | --- |
| `tap` | Enable tap-to-click. |
| `natural_scroll` | Reverse scrolling and three-finger gesture direction. |
| `accel_profile` | Use `"flat"`, `"adaptive"`, or a custom acceleration curve. |
| `sensitivity` | Pointer speed from -1.0 to 1.0. |
| `scroll_factor` | Application scroll multiplier from 0.1 to 10.0. |
| `disable_while_typing` | Disable the touchpad during keyboard input. |
| `disable_on_external_mouse` | Disable the touchpad while an external mouse is connected. |
| `click_method` | Use `"button_areas"` or `"clickfinger"`. |

`scroll_factor` also accepts per-axis values:

```toml
scroll_factor = { horizontal = 2.0, vertical = 1.5 }
```

This factor changes continuous two-finger application scrolling. Overview
navigation uses the factors documented in
[Workspaces Overview](workspaces-overview.md#settings-and-behavior).

### Mouse

```toml
[input.mouse]
natural_scroll = false
# accel_profile = "flat"
sensitivity = 0.0
scroll_wheel_step = 60
# scroll_button = "MouseBack"
# scroll_button_lock = false
```

`sensitivity` ranges from -1.0 to 1.0. `scroll_wheel_step` accepts 1 to 1000
logical pixels per layout-scroll action.

Omitting `accel_profile` preserves the device default. A custom libinput curve
uses this form:

```toml
accel_profile = "custom 0.2 0.0 0.5 1.0 2.0"
```

`scroll_button` turns pointer motion into scrolling while the named button is
held. Set `scroll_button_lock = true` to toggle that mode with a press instead.
Accepted names are `MouseLeft`, `MouseRight`, `MouseMiddle`, `MouseBack`, and
`MouseForward`.

`scroll_wheel_step` controls the distance used by layout scroll actions, not
the scrolling sent to applications.

### Per-device overrides

Use `[[input.device]]` for a device whose case-sensitive name exactly matches
the `Device` value from `libinput list-devices`:

```toml
[[input.device]]
name = "Acme Split Keyboard"
layout = "us"
variant = "colemak_dh"
repeat_rate = 40
repeat_delay = 250

[[input.device]]
name = "Acme Precision Touchpad"
tap = true
natural_scroll = false
click_method = "clickfinger"

[[input.device]]
name = "Acme Gaming Mouse"
accel_profile = "flat"
sensitivity = 0.0
```

Each rule inherits its keyboard, touchpad, or mouse class settings and replaces
only the keys it contains. Duplicate rules for the same device name are
rejected.

### Tablet

```toml
[input.tablet]
enabled = true
map_to_output = "DP-1"
map_to_focused_output = false
map_to_focused_window = false
left_handed = false
calibration_matrix = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
```

| Key | Description |
| --- | --- |
| `enabled` | Enable tablet and pad input. |
| `map_to_output` | Confine the tablet to a connector or monitor name. |
| `map_to_focused_output` | Follow the output holding keyboard focus. |
| `map_to_focused_window` | Map the tablet area to the focused window. |
| `left_handed` | Flip the tablet orientation. |
| `calibration_matrix` | Pass a six-number calibration matrix to libinput. |

Focused-window mapping takes precedence over focused-output mapping, which
takes precedence over `map_to_output`. When the selected target is unavailable,
the next configured mapping is used.

### Cursor

```toml
[input.cursor]
theme = ""
size = 24
hardware_cursor = true
follows_focus = false
hide_when_typing = false
hide_timeout_ms = 0
```

| Key | Range or values | Description |
| --- | --- | --- |
| `theme` | string | Xcursor theme; empty uses the environment default. |
| `size` | 1 to 512 | Cursor size in logical pixels. |
| `hardware_cursor` | bool | Use a hardware cursor plane when available. |
| `follows_focus` | bool | Follow keyboard-driven window focus. |
| `hide_when_typing` | bool | Hide after a non-modifier key press. |
| `hide_timeout_ms` | 0 to 3600000 | Hide after inactivity; `0` disables the timeout. |

Set `hardware_cursor = false` to work around hardware cursor flicker or
disappearance. Set `hide_timeout_ms` to a value from 1 to 3600000 to hide an
inactive cursor; `0` disables the timeout.

`follows_focus = true` moves the cursor to a newly focused window after
keyboard-driven focus and transfer actions. Pointer-driven focus, gestures, and
automatic replacement focus do not move it. `window-focus-warp:<id>` always
moves the cursor regardless of this setting.

### Focus

```toml
[input.focus]
follows_mouse = false
follows_mouse_max_scroll = 0.5
```

`follows_mouse = true` focuses the window under the pointer when pointer motion
or a layout change places a different window there.

`follows_mouse_max_scroll` limits how far Umbriel may scroll a layout to reveal
that window, measured in viewport widths. `0.0` allows only fully visible
windows. Omit the key for no limit.

<!-- umbriel-config page: installation.md -->
# Installing Umbriel

Umbriel is packaged for several Linux distributions. Prefer a distribution
package when one is available; it provides the simplest installation and
upgrade path.

> Distribution packages and third-party repositories are maintained by their
> respective maintainers. Review a repository before installing from it.

## Arch Linux

[`umbriel-git`](https://aur.archlinux.org/packages/umbriel-git) is available in
the AUR:

```sh
yay -S umbriel-git
```

## Fedora

[Terra](https://wiki.fyralabs.com/Terra) provides nightly builds:

```sh
sudo dnf install umbriel-nightly
```

## openSUSE

The [Noctalia OBS repository](https://build.opensuse.org/project/show/home:neifua:Noctalia)
provides
[`umbriel-git`](https://build.opensuse.org/package/show/home:neifua:Noctalia/umbriel-git).

#### Tumbleweed
```sh
sudo zypper addrepo --refresh --name Noctalia https://download.opensuse.org/repositories/home:neifua:Noctalia/openSUSE_Tumbleweed/home:neifua:Noctalia.repo
sudo zypper refresh && sudo zypper install umbriel-git
```

#### Slowroll
```sh
sudo zypper addrepo --refresh --name Noctalia https://download.opensuse.org/repositories/home:neifua:Noctalia/openSUSE_Slowroll/home:neifua:Noctalia.repo
sudo zypper refresh && sudo zypper install umbriel-git
```

## Debian and Ubuntu

The NickH APT repository provides Umbriel for Debian-based distributions.

### Install the repository signing key

```sh
wget https://pkg.noctalia.dev/deb/nickh-archive-keyring.deb
sudo dpkg -i nickh-archive-keyring.deb
```

### Add the repository

Choose the source matching your distribution:

```sh
# Debian Trixie
sudo wget -O /etc/apt/sources.list.d/noctalia-trixie.sources \
  https://pkg.noctalia.dev/deb/noctalia-trixie.sources

# Debian Sid
sudo wget -O /etc/apt/sources.list.d/noctalia-unstable.sources \
  https://pkg.noctalia.dev/deb/noctalia-unstable.sources

# Ubuntu 26.04
sudo wget -O /etc/apt/sources.list.d/noctalia-resolute.sources \
  https://pkg.noctalia.dev/deb/noctalia-resolute.sources
```

### Install Umbriel

```sh
sudo apt update
sudo apt install umbriel
```

The repository provides `amd64` and `arm64` packages only.

## GNU Guix

Umbriel and its XDG portal are available through the third-party
[`midnight`](https://codeberg.org/stampede/midnight) Guix channel. Add the
channel to `~/.config/guix/channels.scm`:

```scheme
(channel
  (name 'midnight)
  (url "https://codeberg.org/stampede/midnight.git")
  (branch "main")
  (introduction
    (make-channel-introduction
      "d97d1568954cfcbf543c9fcdfd5771e2b730ae19"
      (openpgp-fingerprint
        "640A 2C3C E948 22D3 394B 40C3 CAFA EECA 00FF 9B1E"))))
```

Run `guix pull`, then install `umbriel` and
`xdg-desktop-portal-umbriel`. Adding them to the system configuration makes the
Umbriel session available to display managers.

## Manual build

Manual installations have no automatic upgrade path. Prefer a distribution
package when one is available.

Install a C++23 compiler, Meson, Ninja, `just`, `pkg-config`,
`wayland-scanner`, and the development packages listed in
[`PACKAGING.md`](https://github.com/noctalia-dev/umbriel/blob/main/PACKAGING.md#dependencies).
Then clone, build, and
install Umbriel:

```sh
git clone https://github.com/noctalia-dev/umbriel.git
cd umbriel
just release
sudo just install
```

The default installation prefix is `/usr/local`. Set `prefix` when building to
install elsewhere:

```sh
just prefix="$HOME/.local" release
just install
```

## Starting Umbriel

Installed display-manager sessions start Umbriel through `start-umbriel`.
Select Umbriel from your display manager, or start it from a TTY:

```sh
start-umbriel
```

The launcher loads the login profile for supported shells such as bash, zsh,
and fish. Environment variables from that profile are available to Umbriel and
applications started in the session. Interactive shell files such as
`~/.zshrc` are not loaded.

In a managed native session, Umbriel places startup, autostart, event, and
`spawn:` commands in scopes bound to the compositor service, so they are
cleaned up when the session ends. This requires systemd 254 or newer.

Run `umbriel` directly only for a nested development session or an explicitly
unmanaged launch.

## Logs

Umbriel writes its main log to `$XDG_CACHE_HOME/umbriel/umbriel.log`. If
`XDG_CACHE_HOME` is unset, the fallback path is
`~/.cache/umbriel/umbriel.log`. The previous file is retained as
`umbriel.log.1` when the current log reaches 1 MiB.

When standard output or standard error is connected to a TTY, raw writes from
Umbriel and its child processes are redirected to
`$XDG_CACHE_HOME/umbriel/umbriel-stderr.log`, or
`~/.cache/umbriel/umbriel-stderr.log` when `XDG_CACHE_HOME` is unset.

<!-- umbriel-config page: ipc.md -->
# IPC

Umbriel exposes a local UNIX socket for queries, actions, and event
subscriptions. Most users should use the `umbriel` command rather than connect
to the socket directly.

`UMBRIEL_SOCKET` contains the socket path. Without it, use
`$XDG_RUNTIME_DIR/umbriel-$WAYLAND_DISPLAY.sock`.

Each request and reply is one JSON object per line:

```sh
printf '{"cmd":"workspaces"}\n' | socat -t 5 STDIO "$UMBRIEL_SOCKET"
```

Replies use `{"ok": ...}` or `{"err": "..."}`.

## Queries

| Request | CLI |
| --- | --- |
| `{"cmd":"windows"}` | `umbriel windows --json` |
| `{"cmd":"workspaces"}` | `umbriel workspaces --json` |
| `{"cmd":"submap"}` | `umbriel submap --json` |
| `{"cmd":"layers"}` | `umbriel layers --json` |
| `{"cmd":"msg","arg":"<action>"}` | `umbriel msg <action>` |

Window entries include IDs, application identity, process ID, geometry,
workspace, and scratchpad membership. XWayland windows report an unknown client
PID because they share the xwayland-satellite connection.

Workspace entries include a stable ID, display name, index, output, layout,
occupancy, and active and focused states. Use the `named` boolean instead of
guessing from the display name; an explicitly named workspace may still be
called `"2"`.

## Event stream

Subscribe with:

```json
{"cmd":"subscribe","events":["workspaces","windows"]}
```

The connection first receives the current state of each family, then a new
snapshot whenever that family changes:

```json
{"event":"workspaces","data":[]}
```

| Family | Changes reported |
| --- | --- |
| `theme` | Colors and corner radius |
| `overview` | Overview opened, or started closing |
| `keyboard_layout` | Active keyboard layout |
| `windows` | Window identity, geometry, focus, state, workspace, or scratchpad |
| `workspaces` | Inventory, layout, activity, occupancy, output, or focus |
| `submap` | Active keybind submap |

Payloads are full snapshots rather than deltas. Replace local state with the
newest event instead of trying to merge increments. Identical consecutive
payloads are omitted.

An unknown family returns an error and closes the subscription.

### Theme payload

The `theme` event mirrors `[colors]`, `[colors.border]`,
`[colors.overview]`, and `appearance.corner_radius`:

```json
{"event":"theme","data":{
  "background":"#141419FF",
  "text_primary":"#E8E8EAFF",
  "text_muted":"#8A8A92FF",
  "accent_primary":"#7AA3FFFF",
  "accent_secondary":"#F5C96BFF",
  "warning":"#F5C96BFF",
  "error":"#FF6B6BFF",
  "insert_hint":"#7FC8FF80",
  "backdrop":"#000000FF",
  "shadow":"#0000007F",
  "border":{
    "focused":"#7AA3FFFF",
    "unfocused":"#292933FF",
    "scratchpad_focused":"#E5C07BFF",
    "scratchpad_unfocused":"#5C4A2AFF",
    "outer":"#1A1A1FFF"
  },
  "overview":{
    "background_tint":"#10101430",
    "workspace_background":"#00000044",
    "badge":"#7AA3FFFF"
  },
  "corner_radius":10
}}
```

See [Appearance](appearance.md#colors) for the meaning of each value.

### From the command line

The CLI exposes the same event stream:

```sh
umbriel subscribe workspaces
umbriel subscribe workspaces,windows
umbriel subscribe submap
```

It writes one JSON line per event until Umbriel exits or the reader closes:

```sh
umbriel subscribe workspaces |
  jq -r '.data[] | select(.focused) | "\(.output) \(.name) \(.layout)"'
```

## Inspection commands

`umbriel outputs`, `umbriel color`, `umbriel tearing`, `umbriel layers`, and
`umbriel keyboard-layouts` print human-readable state. Each accepts `--json`.
`umbriel validate` checks a configuration without a running compositor.

<!-- umbriel-config page: keybinds.md -->
# Keybinds

Configure bindings under `[keybinds]`. See [Actions](actions.md) for the
complete action list.

```toml
[keybinds]
"Mod+T" = "spawn:kitty"
"Mod+Q" = "window-close"
"Mod+Left" = "window-focus-left"
"Mod+Right" = "window-focus-right"
"Mod+I" = "overview-toggle"
```

## Modifiers

| Modifier | Notes |
| --- | --- |
| `Mod` | Uses `general.mod_key`; defaults to Super on DRM and Alt when nested. |
| `Shift` | |
| `Ctrl` or `Control` | |
| `Alt` | |
| `Super`, `Logo`, or `Win` | |

Bare keys such as `XF86AudioMute` are also valid.

A modifier can be bound by itself:

```toml
"Mod" = "spawn:noctalia msg panel-toggle launcher"
```

Modifier-only binds run on release when no other key, button, scroll, touch, or
gesture input occurred while the modifier was held. Pointer motion alone does
not cancel them.

## Special keys

- Wheel: `WheelUp`, `WheelDown`, `WheelLeft`, `WheelRight`
- Mouse: `MouseLeft`, `MouseRight`, `MouseMiddle`, `MouseBack`, `MouseForward`

Mouse and wheel binds require at least one modifier:

```toml
"Mod+MouseMiddle" = "layout-scroll-drag"
```

## Consumed input

A matched bind consumes its press and release, so neither reaches the focused
application. Unbound input is delivered normally.

## Repeat

Binds repeat using the configured keyboard rate and delay, including `spawn:`
binds, so held volume and brightness keys keep stepping. Disable repeat for one
bind, such as a launcher, with the table form:

```toml
"Mod+Return" = { action = "spawn:kitty", repeat = false }
```

Scratchpad visibility and cycling actions never repeat. The built-in `Mod+Q`
and `Mod+O` binds do not repeat either.

## Allow when locked

Binds are blocked while the session is locked unless explicitly allowed:

```toml
"XF86MonBrightnessDown" = { action = "spawn:noctalia msg brightness-down 10", allow_when_locked = true }
```

Use this only for actions that are safe without an unlocked session.

## Keyboard shortcuts inhibition

Games and remote desktop clients can ask Umbriel to pass keyboard shortcuts
through. Pointer and wheel binds are unaffected.

Keep one escape binding available:

```toml
"Mod+Shift+Escape" = { action = "shortcuts-inhibit-toggle", allow_when_inhibited = true, repeat = false }
```

## Cooldown

`cooldown_ms` suppresses repeated actions for a period while continuing to
consume matching input:

```toml
"Mod+WheelUp" = { action = "workspace-previous", cooldown_ms = 150 }
"Mod+WheelDown" = { action = "workspace-next", cooldown_ms = 150 }
```

## Submaps

Submaps are temporary keybind layers. Enter one with `submap:<name>` and leave
one level with `submap:reset`.

Prefix bindings inside a submap with `submap[name],`:

```toml
"Mod+S" = { action = "submap:screencapture", repeat = false }
"submap[screencapture],1" = { action = "spawn:grim screenshot.png", submap = "reset" }
"submap[screencapture],2" = { action = "submap:region", repeat = false }
"submap[screencapture],Escape" = "submap:reset"
"submap[region],R" = { action = "spawn:grim -g \"$(slurp)\" screenshot.png", submap = "reset" }
"submap[region],Escape" = "submap:reset"
```

The optional `submap` field applies a transition after the action. Use
`submap = "reset"` for one-shot commands. A default-context
`"Escape" = "submap:reset"` always matches, even outside a submap, so use it
only when bare Escape should be consumed globally.

Run `umbriel submap` to print the active layer, or subscribe to `submap` events
for a panel or script.

## Keyboard layouts

Bindings normally match the symbol produced by the active layout. When several
layouts are configured, Umbriel also checks the same physical key for a
printable ASCII symbol in the other layouts. This keeps binds such as `Mod+T`
working after switching to a non-Latin layout.

A keyboard configured with only a non-Latin layout has no ASCII fallback. Add
an alternate layout or bind the active XKB keysym name.

## Hot corners

Hot corners run an action after the pointer rests in an output corner:

```toml
[hot_corners.top_left]
enabled = true
delay_ms = 500
action = "overview-open"
```

Available sections are `top_left`, `top_right`, `bottom_left`, and
`bottom_right`.

| Key | Default | Description |
| --- | --- | --- |
| `enabled` | `false` | Enable this corner. |
| `delay_ms` | `500` | Delay from 0 to 10000 milliseconds. |
| `action` | unset | Any action accepted by a keybind. |

Hot corners are inactive while the output's focused window is fullscreen.

## Cheatsheet

The cheatsheet lists active keybinds. It opens at startup when
`general.show_cheatsheet` is enabled. Use `cheatsheet-toggle`,
`cheatsheet-open`, or `cheatsheet-close` from a bind or `umbriel msg`.

Any non-modifier key or mouse button closes it. Normal bound actions still run.

## Example: Noctalia shell integration

```toml
"Mod" = "spawn:noctalia msg panel-toggle launcher"
"Mod+V" = "spawn:noctalia msg panel-toggle clipboard"
"Mod+W" = "spawn:noctalia msg panel-toggle wallpaper"
"Mod+P" = "spawn:noctalia msg screenshot-region"
"Mod+Escape" = "spawn:noctalia msg panel-toggle session"
```

## Example: direct primary extents

```toml
"Mod+A" = "window-set-primary-extent:0.333"
"Mod+S" = "window-set-primary-extent:0.5"
"Mod+D" = "window-set-primary-extent:0.667"
"Mod+F" = "window-set-primary-extent:1.0"
"Mod+R" = "window-cycle-primary-extent"
"Mod+Shift+R" = "window-cycle-primary-extent-back"
```

## Example: scroll-wheel navigation

```toml
"Mod+WheelUp" = "window-focus-left"
"Mod+WheelDown" = "window-focus-right"
"Mod+Shift+WheelUp" = "column-move-left"
"Mod+Shift+WheelDown" = "column-move-right"
```

## Example: media and brightness keys

```toml
"XF86AudioRaiseVolume" = "spawn:wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%+"
"XF86AudioLowerVolume" = "spawn:wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"
"XF86AudioPlay" = "spawn:playerctl play-pause"
"XF86AudioNext" = "spawn:playerctl next"
"XF86AudioPrev" = "spawn:playerctl previous"
"XF86MonBrightnessUp" = "spawn:brightnessctl set +5%"
"XF86MonBrightnessDown" = "spawn:brightnessctl set 5%-"
```

Volume commands require `wpctl`, media commands require `playerctl`, and
brightness commands require `brightnessctl`.

<!-- umbriel-config page: layer-rules.md -->
# Layer Rules

Layer rules apply visual effects to layer-shell surfaces such as bars,
launchers, and notifications. Run `umbriel layers` to list active namespaces.

```toml
[[layer_rule]]
match.namespace = "^noctalia-bar-"
blur = true
blur_ignore_alpha = 0.5
blur_popups = true
```

## Matching

| Selector | Type | Description |
|----------|------|-------------|
| `match.namespace` | regex | Match the layer surface namespace. |

Regular expressions match any part of a namespace. Use `^` and `$` for an
exact match.

## Effects

| Key | Type | Description |
|-----|------|-------------|
| `blur` | bool | Enable/disable blur for the layer surface. |
| `blur_popups` | bool | Enable/disable blur for descendant XDG popups. |
| `blur_ignore_alpha` | float | Skip blur below an alpha threshold. |
| `blur_optimized` | bool | Override the global optimized-blur choice. |

Layer-shell blur is off by default. Every matching rule contributes its
settings, and later values take precedence.

## Keyboard focus

A layer surface declares its own keyboard interactivity through the layer-shell
protocol; no rule overrides it.

| Interactivity | Behavior |
|---------------|----------|
| `none` | Never receives keyboard focus. Clicking the surface leaves the focused window alone. |
| `on_demand` | Takes focus when mapped; clicking a window or using a focus action moves focus away. |
| `exclusive` | Keeps keyboard focus; windows receive no keys and focus actions cannot leave it. |

Launchers and panels with search fields commonly use `on_demand`.

<!-- umbriel-config page: layout.md -->
# Layout

Choose one layout mode for each workspace. Configure the default globally, then
use workspace rules for exceptions.

## Choose a layout

```toml
[layout]
mode = "scrolling"
```

| Mode | Arrangement | Best suited to |
| --- | --- | --- |
| `scrolling` | Fixed-size columns in a scrollable strip | Keeping many windows readable without shrinking all of them |
| `dwindle` | Each new window splits an existing tile | Flexible recursive tiling |
| `master` | Primary windows beside a stack | Keeping one or more main windows prominent |

Change the current workspace at runtime with
`workspace-set-layout:<mode>`. See [Actions](actions.md#argument-forms) and
[Workspace Rules](workspaces.md#workspace-rules).

## Shared settings

```toml
[layout]
gap = 8
extent_presets = [0.333, 0.5, 0.667]
```

| Key | Default | Description |
| --- | --- | --- |
| `gap` | `8` | Gap between windows in logical pixels. |
| `extent_presets` | `[0.333, 0.5, 0.667]` | Fractions used by primary and secondary extent cycle actions. |

### Struts

```toml
[layout.struts]
left = 0
right = 0
top = 0
bottom = 0
```

Positive struts reserve extra space inside layer-shell exclusive zones.
Negative values let tiled windows extend beneath panels or beyond an output
edge. Floating windows and popups ignore struts.

All layouts support `Mod+Right-drag` resizing. Drag from an edge to resize one
axis or from a corner to resize both.

## Scrolling layout

Scrolling keeps columns at their configured extents and moves the strip through
the output. A column can contain several stacked windows.

The strip is perpendicular to the output's
[workspace axis](workspaces.md#workspace-axis). Vertical workspaces use a
horizontal strip; horizontal workspaces use a vertical strip.

### Settings

```toml
[layout.scrolling]
default_extent_fraction = 0.5
center_underfull_strip = true
center_focused = "never"
```

| Key | Default | Description |
| --- | --- | --- |
| `default_extent_fraction` | unset | Initial column extent from 0.1 to 1.0. The packaged config uses `0.5`. |
| `center_underfull_strip` | `true` | Center a strip narrower than the viewport. |
| `center_focused` | `"never"` | Use `"never"`, `"always"`, or `"on_overflow"` to control focus centering. |

### Horizontal and vertical scrolling

| Workspace axis | Scrolling arrangement |
| --- | --- |
| `vertical` | Columns run left to right; windows stack top to bottom. |
| `horizontal` | Lanes run top to bottom; windows sit left to right. |

Primary extent actions resize a column along the strip. Secondary extent
actions resize a window within its column.

A three-finger swipe along the workspace axis switches workspaces. A swipe
across it scrolls the strip. Touchpad direction follows
`input.touchpad.natural_scroll`.

### Scrolling behavior

When `default_extent_fraction` is unset, applications choose their initial
extent. A `default_scrolling_extent` window rule overrides the fraction, and
`default_scrolling_extent_px` takes highest precedence.

Set an output-specific default with:

```toml
[output.DP-1.layout.scrolling]
default_extent_fraction = 0.4
```

A workspace rule can override both global and output defaults. Reloading these
settings affects new columns only.

When focus moves to a hidden column, Umbriel scrolls just far enough to reveal
it. Dragged windows show an insertion preview and can be dropped into a new or
existing column.

## Vertical strips

With horizontal workspaces, screen directions remain literal:

- Left and right actions move within a lane.
- Up and down actions move between lanes.
- Strip scroll actions toward up or left move toward strip start.

Configurations using vertical strips usually bind wheel navigation to
`window-focus-up` and `window-focus-down` instead of the default left and right
actions.

## Dwindle layout

Dwindle recursively splits tiles into independently sized regions.

### Settings

```toml
[layout.dwindle]
preserve_split = false
new_exits_fullscreen = false
```

| Key | Default | Description |
| --- | --- | --- |
| `preserve_split` | `false` | Keep each split direction fixed after creation. |
| `new_exits_fullscreen` | `false` | Exit fullscreen when a new window opens. |

### Behavior

A new window splits the focused tile along its longer edge. Without
`preserve_split`, split directions may adapt as geometry changes. Enable it for
stable, manually shaped regions.

Dwindle has no multi-window columns. Moving one into Dwindle places its windows
as separate tiles.

## Master layout

Master places one or more primary windows in a master area and the remaining
windows in a stack. The master area may sit left, right, or between two stacks.

### Settings

```toml
[layout.master]
position = "left"
default_width_fraction = 0.55
new_on_top = true
new_becomes_master = false
new_exits_fullscreen = false
```

| Key | Default | Description |
| --- | --- | --- |
| `position` | `"left"` | Use `"left"`, `"right"`, or `"center"`. |
| `default_width_fraction` | `0.55` | Initial master-area fraction. |
| `new_on_top` | `true` | Put new stack windows at the top. |
| `new_becomes_master` | `false` | Give the master slot to each new window. |
| `new_exits_fullscreen` | `false` | Exit fullscreen when a new window opens. |

### Behavior

The first window becomes master. Later windows join the stack unless
`new_becomes_master` is enabled. Use
`layout-master-count-increase` and `layout-master-count-decrease` to move
windows between the two areas.

In center mode, stack windows are balanced between the left and right sides.
Primary extent actions resize the master area. Secondary extent actions resize
rows within an area.

## Sizing behavior

Primary and secondary extent actions use `layout.extent_presets`:

- Scrolling: primary changes column extent; secondary changes a row.
- Dwindle: primary adjusts horizontal splits; secondary adjusts vertical splits.
- Master: primary changes the master fraction; secondary changes a row.
- Floating: primary changes width; secondary changes height.

The `window-set-*` actions assign an exact fraction. `window-modify-*` changes
it by a signed amount, and `window-cycle-*` walks the configured presets.

### Client minimum sizes

Applications may enforce a minimum size larger than their assigned tile.
Umbriel clips oversized content rather than allowing it to cover neighboring
tiles. Application-specific minimums must be disabled in that application's
settings when smaller tiles are required.


### Floating windows

For floating windows, extent fractions use the output's usable area and respect
the application's minimum and maximum size hints. Resizing a maximized floating
window leaves maximization and keeps the new size. Extent actions do nothing
while the window is fullscreen.

Parented dialogs stay above their parent and normally open centered over its
visible area.

### Maximize and fullscreen

`window-toggle-fullscreen` fills the complete output, ignoring struts and panel
exclusive zones.

`window-toggle-maximize` fills the layout area. Tiled columns keep struts and
gaps; floating windows fill the output's usable area.

`window-toggle-maximize-to-edges` removes layout struts, gaps, and borders while
leaving panel exclusive zones visible.

<!-- umbriel-config page: outputs.md -->
# Outputs

Output sections configure monitors by connector name, such as `DP-1`, or by
monitor identity:

```toml
[output.DP-1]
mode = "3840x2160@165"
position = [0, 0]
scale = 1.25
```

Run `umbriel outputs` inside a session to list names and available modes. The
`Config name` value is a copyable monitor identity in
`"<make> <model> <serial>"` form:

```toml
[output."Microstep MSI G2712F CD6T084401192"]
mode = "1920x1080@180"
```

Use a monitor identity when settings should follow one display between ports.
Use a connector when settings belong to a physical port. If both match, the
monitor section wins. Matching is case-insensitive.

When an output disconnects or is disabled, Umbriel temporarily moves its
workspaces and windows to another enabled output. They return with their layout
and positions when the output becomes available again.

## Settings

| Key | Type | Default | Description |
| --- | --- | --- | --- |
| `enabled` | bool | `true` | Turn the monitor on or off. |
| `mode` | string | preferred | Resolution and optional refresh rate, such as `"2560x1440@165"`. |
| `position` | `[x, y]` | automatic | Top-left position in logical coordinates. |
| `scale` | float | `1.0` | Output scale from 0.25 to 4.0. |
| `transform` | string | `"normal"` | Rotation or reflection. |
| `vrr` | string | `"disabled"` | Variable refresh rate policy. |
| `tearing` | bool | `false` | Allow eligible fullscreen windows to use asynchronous page flips. |
| `direct_scanout` | bool | `true` | Allow eligible fullscreen buffers to bypass composition. |
| `hdr` | string | `"off"` | HDR activation policy. |
| `sdr_white` | float | `203` | SDR reference white in cd/m² while HDR is active. |
| `workspaces` | int, string array, or `"dynamic"` | `"dynamic"` | Workspace inventory for this output. |
| `min_workspaces` | int | `1` | Minimum count for a dynamic output. |
| `workspace_axis` | string | `"vertical"` | Workspace arrangement axis. |
| `layout.scrolling.default_extent_fraction` | float | inherited | Initial scrolling-column extent on this output. |

Umbriel tries an unadvertised resolution as a custom mode. If it cannot apply
the configured mode, it uses the preferred advertised mode and logs a warning.

### Workspace count

`workspaces` accepts:

- `"dynamic"` or an omitted value for workspaces that grow and shrink
- An integer for a fixed number of anonymous workspaces
- A string array for a fixed ordered list of names

`min_workspaces` sets a floor for a dynamic output:

```toml
[output.DP-1]
min_workspaces = 3
```

Do not combine `min_workspaces` with a fixed workspace inventory. See
[Workspaces](workspaces.md#choose-a-workspace-model) for naming, lifecycle, and
workspace rules.

### Initial scrolling width

Override the global starting width for new scrolling columns on one output:

```toml
[output.DP-1.layout.scrolling]
default_extent_fraction = 0.4
```

A matching workspace rule can override this value. Reloading affects new
columns only; existing columns keep their current width. See
[Scrolling behavior](layout.md#scrolling-behavior).

### Position and scale

Positions use logical coordinates after scale and transform. A `3840x2160`
output at scale `1.25` occupies `3072x1728` logical units. An output immediately
to its right therefore starts at x = 3072.

Omit `position` to place outputs automatically from left to right. Explicitly
positioned outputs must touch or overlap for the pointer to move between them.

### Transform values

Accepted values are `normal`, `90`, `180`, `270`, `flipped`, `flipped-90`,
`flipped-180`, and `flipped-270`.

### Direct scanout

Direct scanout can reduce composition work for eligible fullscreen
applications. Disable it if fullscreen content causes corruption, black frames,
or flicker:

```toml
[output.DP-1]
direct_scanout = false
```

The change applies on reload. Disabling direct scanout can increase GPU use and
power consumption.

Set `WLR_SCENE_DISABLE_DIRECT_SCANOUT=1` before starting Umbriel to disable
direct scanout on every output.

### Variable refresh rate

`vrr` accepts:

| Value | Behavior |
| --- | --- |
| `"disabled"` | Never enable adaptive sync. |
| `"always"` | Keep adaptive sync enabled when supported. |
| `"fullscreen"` | Enable it while the active workspace has a fullscreen window. |

```toml
[output.DP-1]
vrr = "fullscreen"
```

A focused window can override this policy through a
[window rule](window-rules.md#settings-updated-while-a-window-is-open).
Unsupported outputs remain at fixed refresh and produce a warning.

### Tearing

Tearing requires an output-level opt-in:

```toml
[output.DP-1]
tearing = true
```

Umbriel uses asynchronous presentation only for an eligible fullscreen window
that requests it or matches a `tearing = true` window rule. A window rule can
also veto a client request. Run `umbriel tearing` to inspect eligibility and
fallback reasons.

### HDR

`hdr` accepts:

| Value | Behavior |
| --- | --- |
| `"off"` | Keep the output in SDR. |
| `"on"` | Keep the output in HDR. |
| `"auto"` | Enable HDR for fullscreen content with supported HDR metadata. |
| `"fullscreen"` | Enable HDR for any fullscreen content. |

```toml
[output.DP-1]
hdr = "auto"
sdr_white = 203
```

Automatic HDR depends on metadata supplied by the application. Untagged
XWayland content cannot be detected; use a native Wayland HDR path or
`hdr = "on"` when necessary. Many monitors briefly go black while switching
between SDR and HDR.

Some native Wayland Proton builds require `PROTON_ENABLE_WAYLAND=1` and
`DXVK_HDR=1` before they publish HDR metadata. Proton variants differ, so follow
the selected runtime's documentation and fully restart Steam after changing
session environment values.

Screenshots from normal screencopy clients receive an SDR view while HDR is
active.

## Disabling an output

Set `enabled = false` for a persistent disabled state:

```toml
[output.HDMI-A-1]
enabled = false
```

The output leaves the desktop, but its workspaces and windows are retained and
return when it is enabled again. Output-management tools can temporarily
override this state until a later configuration reload reapplies the file.

## Display power management

Use DPMS actions to power monitors off without removing their workspaces:

```sh
umbriel msg dpms-off
umbriel msg dpms-off:DP-1
umbriel msg dpms-on:DP-1
```

The bare actions target every configured output. Input wakes all monitors when
every output is powered off. Outputs disabled with `enabled = false` are not
affected.

## Live reconfiguration

Tools such as `wlr-randr`, `kanshi`, and `wdisplays` can change enabled state,
mode, position, scale, transform, and adaptive sync while Umbriel is running.
Those changes last until another tool request or an output-related config
reload replaces them.

## Multi-monitor example

```toml
[output.DP-1]
mode = "3840x2160@165"
position = [0, 0]
scale = 1.25
workspaces = 5

[output.DP-2]
mode = "2560x1440@144"
position = [1300, -1440]
scale = 1.0
workspaces = ["VIDEO"]

[output.HDMI-A-1]
mode = "1920x1080@60"
position = [3072, 0]
scale = 1.0
workspaces = ["CHAT", "STATS"]
```

The primary output is 3072 logical units wide, so the HDMI output begins at
x = 3072.

## Machine-specific overrides

Keep output configuration in a machine-specific include when sharing one base
configuration between systems:

```toml
[include]
files = [
  "src/general.toml",
  "src/keybinds.toml",
  "machines/monolith.toml",
]
```

<!-- umbriel-config page: scratchpad.md -->
# Scratchpads

A scratchpad stores windows outside normal workspaces so they can be shown or
hidden quickly. Its windows float and appear together.

- Moving a window to a scratchpad stores it.
- Toggling a scratchpad shows or hides its stored windows.
- Restoring a window returns it to its saved output and workspace.

## Default scratchpad

Without `[[scratchpad]]` entries, Umbriel creates one implicit scratchpad named
`default`. The packaged config binds it without a name suffix:

```toml
[keybinds]
"Mod+Shift+Space" = "window-move-to-scratchpad"
"Mod+Space" = "scratchpad-toggle"
"Mod+Ctrl+Space" = "window-restore-from-scratchpad"
"Mod+Tab" = "scratchpad-focus-next"
```

A typical workflow:

1. Focus a window and press `Mod+Shift+Space` to store it.
2. Press `Mod+Space` to show or hide the scratchpad.
3. Press `Mod+Tab` to cycle through visible members.
4. Press `Mod+Ctrl+Space` to return the focused member to its workspace.

## Named scratchpads

Define names when you want several independent scratchpads:

```toml
[[scratchpad]]
name = "terminal"

[[scratchpad]]
name = "music"

[keybinds]
"Mod+Shift+Space" = "window-toggle-scratchpad:terminal"
"Mod+Space" = "scratchpad-toggle:terminal"
"Mod+Shift+M" = "window-toggle-scratchpad:music"
"Mod+M" = "scratchpad-toggle:music"
```

Defining any named scratchpad removes the implicit `default`. Every scratchpad
action must then include a configured name. Names must be unique and `default`
is reserved.

Removing a scratchpad on reload restores its windows to their saved
destinations.

## Assigning new windows automatically

Use a `default_scratchpad` window rule:

```toml
[[scratchpad]]
name = "terminal"

[[window_rule]]
match.app_id = "^scratchpad-terminal$"
default_scratchpad = "terminal"
```

For example, `foot --app-id scratchpad-terminal` opens hidden in `terminal`.
With no named definitions, use `default_scratchpad = "default"`.

The window remembers where and how it would otherwise have opened.
`default_output`, `default_workspace`, and `default_floating` control that
restore destination.

## Actions

| Action | What it does |
| --- | --- |
| `window-move-to-scratchpad:[<scratchpad>]` | Store the focused workspace window. |
| `scratchpad-toggle:[<scratchpad>]` | Show or hide the selected scratchpad. |
| `window-restore-from-scratchpad:[<scratchpad>]` | Restore one remembered window. |
| `window-toggle-scratchpad:[<scratchpad>]` | Store the focused window or restore it when already selected. |
| `scratchpad-focus-next:[<scratchpad>]` | Focus the next visible member. |

The argument is optional only for the implicit `default` scratchpad. Restore
and focus-next require the scratchpad to be visible.

`window-focus:<window-id>` summons a matching hidden scratchpad window before
focusing it. `window-focus-warp:<window-id>` also moves the cursor to it.

## Choosing an output

Scratchpads are global rather than owned by one output. The pointer selects the
output where a hidden scratchpad appears.

Toggling a scratchpad already visible on another output moves it to the invoking
output. Toggling it again on the same output hides it. Only one scratchpad can
be visible on an output, but different outputs can show different scratchpads.

## Visibility and focus

Showing a scratchpad focuses its most recently focused member. Hiding it returns
focus to a workspace window. Opening the workspace overview hides every visible
scratchpad without removing its stored windows.

A dialog parented to a visible scratchpad window joins that scratchpad unless a
window rule explicitly chooses another destination.

Backdrop dim and blur affect only the output showing the scratchpad.

## Restoring windows

Each member remembers its source output, workspace, and tiled or floating
state. If that output or workspace no longer exists, Umbriel restores the
window to the scratchpad's current output and active workspace.

Fullscreen, pinned, and maximize-to-edges state are cleared when a window enters
a scratchpad. Scratchpad animation settings may apply a new fullscreen,
maximized, or scaled presentation while it is stored.

## Moving scratchpad windows

Dragging a scratchpad window does not restore it. Dragging one member to another
output moves the whole scratchpad there while preserving the other members'
relative positions.

If its output disconnects, the scratchpad temporarily moves to another enabled
output and returns when the original output becomes available, unless the user
has deliberately moved it elsewhere.

## Appearance and window actions

Scratchpad windows use dedicated border colors:

```toml
[colors.border]
scratchpad_focused = "#E5C07BFF"
scratchpad_unfocused = "#5C4A2AFF"
```

Show and hide transitions, backdrop dimming and blur, and optional entry sizing
are configured under
[`animation.scratchpad`](animation.md#animation).

Normal window size, fullscreen, maximize, and close actions work on a focused
scratchpad window. Layout-relative actions require restoring it to a workspace
first.

## Troubleshooting

- If toggle does nothing, the selected scratchpad has no stored windows.
- If restore or focus-next does nothing, show the scratchpad first.
- If a named action does nothing, check its suffix against configured names.
- If it appears on the wrong monitor, move the pointer before showing it.
- Restore a window before using tiling, pinning, or layout-relative actions.

<!-- umbriel-config page: security.md -->
# Security

## Session locking

Umbriel keeps the current desktop visible while an `ext-session-lock-v1` client
prepares a mapped lock surface for every active output. The client has up to
three seconds to provide those surfaces. Umbriel then switches every output to
the lock scene and reports the session as locked only after each active output
has presented a secure frame.

If a client stalls or omits an output, the handoff continues after the deadline
with an opaque compositor-owned blank. Its RGB color comes from
`[colors].backdrop`; its alpha is always fully opaque. A client that exits after
the secure handoff leaves this blank in place, so the desktop cannot be exposed
by a crashed locker.

## Sandboxed Wayland clients

Umbriel supports version 1 of the
[Wayland security-context protocol](https://wayland.app/protocols/security-context-v1).
It is always enabled.

A sandbox engine can create a restricted Wayland connection and label it with
the sandbox engine, application, and instance. Umbriel then limits the
protocols available through that connection. This does not create the sandbox
or display permission prompts.

### Restricted capabilities

Restricted clients retain the protocols needed for ordinary windows, rendering,
focused input, clipboard use, output discovery, idle inhibition, and activation.
Umbriel withholds compositor-wide authority, including:

- Screen and window capture
- Virtual input and input-method ownership
- Clipboard-manager access
- Layer shell and session locking
- Gamma and output configuration
- Global workspace and window control
- Creation of nested security contexts

Trusted host services such as xdg-desktop-portal can mediate privileged
operations for a sandboxed application.

New protocols remain hidden from restricted clients until they receive a
security review.

### Per-application grants

Use `[[security_context_rule]]` when a sandboxed application genuinely needs a
protocol without a portal equivalent:

```toml
[[security_context_rule]]
match.sandbox_engine = 'org\.flatpak'
match.app_id = 'org\.example\.ClipboardManager'
allow_globals = [
  "ext_data_control_manager_v1",
  "zwlr_data_control_manager_v1",
]
```

Umbriel supports both data-control variants. Existing clipboard managers often
use the `zwlr_` variant, so grant both unless the application is known to use
only `ext_`.

| Selector | Description |
| --- | --- |
| `match.sandbox_engine` | Exact regular-expression match against the sandbox engine. |
| `match.app_id` | Exact regular-expression match against the application ID. |

`allow_globals` lists additional Wayland globals to expose. Every matching rule
contributes its values.

Selectors are optional. A rule without selectors applies to every restricted
client, so avoid broad grants. Patterns match the complete value rather than a
substring.

Rules are additive and cannot remove the base protocol set.
`wp_security_context_manager_v1` remains blocked even if listed. Changes apply
only to new connections, so restart an application after changing its grants.

Invalid entries are ignored with a configuration warning.

### Security boundary

The protocol restricts one Wayland connection. It does not restrict files,
processes, devices, networking, D-Bus, X11, or other host interfaces.

For the restriction to matter, the sandbox must expose only its restricted
Wayland socket. It must also control access to:

- Umbriel IPC through `$UMBRIEL_SOCKET`
- Host D-Bus services
- X11 through `$DISPLAY`
- Devices and other host resources

The sandbox engine supplies the application metadata, so a grant is only as
trustworthy as that engine. Clients connected through the ordinary Wayland
socket remain unrestricted.

Treat security contexts as one part of a sandbox boundary, not as complete
application isolation.

<!-- umbriel-config page: window-rules.md -->
# Window Rules

Window rules apply settings to matching applications. Every matching rule
contributes its values; when two set the same field, the later rule wins.

```toml
[[window_rule]]
match.app_id = "^firefox$"
default_workspace = 2
```

## Matching

| Selector | Description |
| --- | --- |
| `match.app_id` | Match the application ID with a regular expression. |
| `match.title` | Match the window title. |
| `match.xdg_tag` | Match a client-defined XDG toplevel tag. |
| `match.content_type` | Match `"none"`, `"photo"`, `"video"`, or `"game"`. |
| `match.is_focused` | Match current focus state. |
| `match.is_floating` | Match current floating state. |
| `match.is_pinned` | Match current pinned state. |
| `match.is_scratchpad` | Match current scratchpad state. |
| `match.is_alone` | Match whether this is the only tiled window. |
| `match.at_startup` | Match during or after the first 60 seconds. |

Selectors are optional. A rule with none matches every window. Regular
expressions match any part of a value, so use `^` and `$` for an exact match.

Run `umbriel windows` to inspect current application IDs, titles, tags, and
content types. Prefer `app_id` for placement because titles often change.

State selectors update while the window is open. Opening settings are resolved
before a rule's own state changes take effect, so a rule cannot select on the
floating or pinned state that it creates.

## Settings applied when a window opens

| Key | Description |
| --- | --- |
| `default_output` | Open on a connector or monitor name. |
| `default_workspace` | Open on a one-based position or exact workspace name. |
| `default_scratchpad` | Store in the implicit or a configured scratchpad. |
| `default_fullscreen` | Open fullscreen. |
| `default_floating` | Force floating or tiling. |
| `default_maximize` | Open maximized within normal layout bounds. |
| `default_maximize_to_edges` | Open maximized without gaps, struts, or borders. |
| `default_focused` | Choose whether the new window takes focus. |
| `default_pinned` | Open pinned above normal windows. |
| `default_scrolling_column` | Join matching windows into one named scrolling column. |
| `default_scrolling_column_order` | Set order inside a named scrolling column. |

These values apply once when a window opens. Umbriel checks once more when the
first title arrives because some applications set it after mapping.

Dialogs float by default. Use `default_floating = false` in a matching rule to
force one into the layout.

## Size and Position Rules

| Key | Applies to | Description |
| --- | --- | --- |
| `default_floating_size_px` | Floating | Logical-pixel size, such as `{ width = 800, height = 600 }`. |
| `default_floating_size` | Floating | Fraction of the output usable area. |
| `default_scrolling_extent_px` | Scrolling | Initial logical-pixel extent along the strip. |
| `default_scrolling_extent` | Scrolling | Initial fractional extent along the strip. |
| `default_position` | Floating | Position from a named anchor. |

Pixel values take precedence over fractions. Each axis is optional. A tiled
window remembers floating size and position rules until it first floats, and a
floating window remembers its scrolling extent until it first tiles.

```toml
[[window_rule]]
match.app_id = "^org[.]example[.]Utility$"
default_floating = true
default_floating_size_px = { width = 800, height = 600 }
```

### Floating position

Coordinates are logical pixels within the output's usable area:

```toml
default_position = { x = 32, y = 24, anchor = "bottom_left" }
```

Accepted anchors are `center`, `top_left`, `top_right`, `bottom_left`,
`bottom_right`, `top`, `bottom`, `left`, and `right`. Right and bottom anchors
measure inward from those edges.

## Scratchpad placement

Store matching windows directly in a scratchpad:

```toml
[[scratchpad]]
name = "terminal"

[[window_rule]]
match.app_id = "^scratchpad-terminal$"
default_scratchpad = "terminal"
```

With no named definitions, use `"default"`. A hidden scratchpad keeps the new
window hidden. Output, workspace, and floating rules determine where and how it
returns when restored.

## Workspace placement

`default_workspace` uses TOML type to distinguish positions and names:

```toml
default_workspace = 2
# default_workspace = "2"
# default_workspace = "CHAT"
```

An integer selects a one-based position. A string selects an exact,
case-sensitive name. `default_output` restricts either form to one output.
Names must already exist through a static inventory or a named workspace rule.

## Named scrolling columns

Give related applications the same column name:

```toml
[[window_rule]]
match.app_id = "^firefox$"
default_scrolling_column = "browsers"
default_scrolling_column_order = 10

[[window_rule]]
match.app_id = "^chromium$"
default_scrolling_column = "browsers"
default_scrolling_column_order = 20
```

The name is local to a workspace. The first matching window creates the column
and sets its extent.

## Settings updated while a window is open

| Key | Description |
| --- | --- |
| `opacity` | Surface opacity from 0.0 to 1.0. |
| `blur` | Enable or disable window blur. |
| `blur_popups` | Apply blur to descendant XDG popups. |
| `blur_ignore_alpha` | Skip blur below an alpha threshold. |
| `blur_optimized` | Override the global optimized-blur choice. |
| `focus_on_activate` | Override activation focus for this window. |
| `vrr` | Override the focused output's VRR policy. |
| `tearing` | Request or veto asynchronous presentation. |
| `hdr` | Override the focused output's HDR policy. |

These values refresh when matching identity or state changes. Fullscreen
bypasses rule opacity unless
[`appearance.opaque_fullscreen`](appearance.md#window-appearance) is `false`.

## The only window in the workspace

`match.is_alone` reacts when a window becomes the only tiled window on its
workspace:

```toml
[[window_rule]]
match.is_alone = true
default_maximize = true
```

It can apply fullscreen, maximize-to-edges, maximize, or a scrolling extent.
The effect is removed when another tiled window appears and restored when the
window becomes alone again.

Combine it with other selectors when only one application should receive the
behavior:

```toml
[[window_rule]]
match.is_alone = true
match.app_id = "^firefox$"
default_scrolling_extent = 0.8
```

## Examples

```toml
# Blur every window
[[window_rule]]
blur = true

# Float a utility
[[window_rule]]
match.app_id = "^org[.]pulseaudio[.]pavucontrol$"
default_floating = true
default_floating_size = { width = 0.5, height = 0.6 }

# Place a game on workspace 4
[[window_rule]]
match.app_id = "^steam_app_[0-9]+$"
default_workspace = 4
default_fullscreen = true

# Enable VRR for game content
[[window_rule]]
match.content_type = "game"
vrr = "always"

# Dim unfocused windows
[[window_rule]]
match.is_focused = false
opacity = 0.85

[[window_rule]]
match.is_focused = true
opacity = 1.0
```

<!-- umbriel-config page: workspaces-overview.md -->
# Workspaces Overview

The overview displays every workspace and lets you navigate, focus, close, and
move windows.

## Settings and behavior

```toml
[overview]
zoom = 0.5
scroll_factor_horizontal = 1.0
scroll_factor_vertical = 1.0
background_blur = true
workspace_wallpaper = true
shortcuts = true
shortcut_keys = "1234567890"
```

| Key | Default | Description |
| --- | --- | --- |
| `zoom` | `0.5` | Workspace preview scale from 0.1 to 0.75. |
| `scroll_factor_horizontal` | `1.0` | Horizontal gesture and wheel sensitivity. |
| `scroll_factor_vertical` | `1.0` | Vertical gesture and wheel sensitivity. |
| `background_blur` | `true` | Blur the desktop behind the overview. |
| `workspace_wallpaper` | `true` | Show each output's background inside its workspace previews. |
| `shortcuts` | `true` | Show and accept keyboard shortcut badges. |
| `shortcut_keys` | `"1234567890"` | Preferred keys for shortcut badges. |

Background blur uses `[appearance.blur]`. Preview backgrounds use
`colors.overview.workspace_background` when wallpaper mirroring is disabled or
no background surface is available.

### Open and navigate

Press `Mod+O` by default, or use an
[overview action](actions.md#overview). Opening the overview temporarily hides
scratchpads and pinned windows.

- Click a window to focus it and close the overview.
- Middle-click a window to close it.
- Drag a window to move it to another workspace.
- Use the wheel to navigate vertically and Shift+wheel horizontally.
- Use a four-finger swipe to open or close the overview.

Two-finger scrolling and three-finger swipes navigate continuously. Movement
along the output's [workspace axis](workspaces.md#workspace-axis) moves between
workspaces. Movement across it pans a scrolling workspace preview. Dwindle and
Master layouts have no strip to pan.

Touchpad `natural_scroll` controls gesture direction. The horizontal and
vertical overview factors adjust gesture distance and wheel sensitivity without
changing application scrolling.

#### Which window actions act on

One card carries the full focused-border color while the overview is open. It
is the target for focus, close, and other window actions. Each other workspace
keeps a fainter marker showing which card will receive focus when selected.

The current output follows the cursor. Output-changing actions move the cursor
to their destination as usual.

#### Keyboard shortcuts

Normal `[keybinds]` remain active in the overview. Directional focus actions
select neighboring cards, and workspace or output actions keep their normal
fallback behavior.

Shortcut badges provide direct, unmodified key sequences for visible window
cards. When there are more cards than keys, Umbriel creates multi-key labels.
Backspace removes the last key from a pending sequence. Escape clears the
sequence first and closes the overview when no sequence is pending.

Set `shortcuts = false` to disable badges. A normal keybind takes precedence
over a badge. `shortcut_keys` must contain at least two unique printable ASCII
characters.

### Move windows

Drag a window onto another workspace preview to move it. In a dynamic workspace
list, dropping into a gap creates a workspace at that position. Static
workspace inventories accept drops only onto existing previews.

The destination layout shows an insertion preview before the drop. Empty
dynamic workspaces may disappear immediately after their last window is moved
or closed.

### Appearance

Overview cards reuse each window's borders, corner radius, opacity, blur, and
color presentation. Shortcut badges use `colors.overview.badge`.

Configure overview colors under
[`[colors.overview]`](appearance.md#overview-colors).

<!-- umbriel-config page: workspaces.md -->
# Workspaces

Each output has its own workspaces. Choose a dynamic or fixed model, then use
workspace rules to customize individual entries.

## Choose a workspace model

### Dynamic workspaces

Omit `workspaces` or set it to `"dynamic"`:

```toml
[output.DP-1]
workspaces = "dynamic"
min_workspaces = 3
```

A dynamic output keeps an empty workspace at the end and creates another when
that workspace gains a window. Empty inactive anonymous workspaces are removed
again. `min_workspaces` sets a floor, not a maximum.

Set `empty_above = true` under the global
[`[workspaces]`](#global-workspace-settings) section to keep an additional empty
workspace at the beginning.

#### Persistent names in a dynamic inventory

A name-based `[[workspace]]` entry creates a persistent named workspace:

```toml
[[workspace]]
name = "CHAT"
layout.mode = "master"

[[workspace]]
name = "STATS"
output = "DP-1"
layout.mode = "dwindle"
```

Names are case-sensitive and local to an output. An entry without `output`
applies independently to every dynamic output. Named workspaces remain when
empty while anonymous neighbors continue their normal dynamic lifecycle.

Each output supports up to 64 workspaces.

### Static workspaces

Use an integer for a fixed count or a string array for an ordered list of names:

```toml
[output.DP-1]
workspaces = 5

[output.DP-2]
workspaces = ["WEB", "CHAT", "VIDEO"]
```

A count creates anonymous positions. A string list creates exact names, so
position 3 and the name `"3"` remain different selectors. Static workspaces are
never pruned.

### Change workspaces on reload

Workspace changes apply on a valid configuration reload. Umbriel preserves
matching names and positions where possible. Windows from a removed workspace
move to the nearest remaining workspace.

Changing to a dynamic model keeps active and populated workspaces, then restores
the required empty workspace.

## Workspace axis

Set the workspace arrangement per output:

```toml
[output.DP-1]
workspace_axis = "horizontal"
```

| Value | Behavior |
| --- | --- |
| `"vertical"` | Workspaces stack top to bottom; scrolling layouts run left to right. |
| `"horizontal"` | Workspaces sit side by side; scrolling layouts run top to bottom. |

A three-finger swipe along the axis changes workspaces. A swipe across it moves
the scrolling strip. The overview follows the same arrangement.

## Workspace selectors

Actions such as `workspace-switch` and `window-move-to-workspace` use these
forms:

- Bare digits from `1` to `64` select a one-based position.
- Other text selects an exact, case-sensitive name.
- Double quotes force name lookup for an all-digit name.
- `/output` restricts either form to one output.

```text
workspace-switch:3
workspace-switch:3/DP-2
workspace-switch:CHAT/DP-2
workspace-switch:"3"/DP-2
```

Duplicate names resolve on the pointer-preferred output when possible.
Otherwise the selector is ambiguous. On a dynamic output, a numeric position
beyond the current count selects the last workspace.

## Inspect workspace state

Run `umbriel workspaces` to list workspaces and their effective layout:

```text
* DP-1: 1 [scrolling] (focused)
  DP-1: 2 [dwindle]
* DP-2: WEB [master]
```

Use `umbriel workspaces --json` for scripts. Each entry includes its stable
`id`, display `name`, one-based `index`, `output`, active and focused states,
and effective `layout`.

## Global workspace settings

```toml
[workspaces]
back_and_forth = true
empty_above = false
```

| Key | Default | Description |
| --- | --- | --- |
| `back_and_forth` | `false` | Selecting the active workspace returns to the previous one on that output. |
| `empty_above` | `false` | Keep a leading empty workspace on dynamic outputs. |

## Workspace rules

`[[workspace]]` entries customize one workspace. Select it with exactly one of
`name` or `index`, and optionally restrict it to an output:

```toml
[[workspace]]
name = "VIDEO"
output = "DP-2"
layout.mode = "dwindle"
layout.gap = 4
```

On dynamic outputs, `name` also creates a persistent named workspace. `index`
follows whichever workspace currently occupies that position. On static
outputs, rules only customize members already present in the configured
inventory.

### How settings are combined

Settings apply from least to most specific:

1. Global `[layout]`
2. Matching output scrolling extent
3. Matching workspace rule without `output`
4. Matching workspace rule with `output`

Later values replace earlier ones. Strut edges are resolved independently, so a
rule can override only `layout.struts.top`.

### Available fields

| Key | Description |
| --- | --- |
| `name` | Select an exact name and create it on matching dynamic outputs. |
| `index` | Select a current one-based position from 1 to 64. |
| `output` | Restrict the rule to a connector or monitor name. |
| `layout.mode` | Use `"scrolling"`, `"dwindle"`, or `"master"`. |
| `layout.gap` | Set the window gap. |
| `layout.struts.{left,right,top,bottom}` | Reserve signed logical pixels at each edge. |
| `layout.extent_presets` | Set extent-cycle fractions. |
| `layout.scrolling.default_extent_fraction` | Set the initial scrolling-column extent. |
| `layout.scrolling.center_underfull_strip` | Center or start-align an underfull strip. |
| `layout.scrolling.center_focused` | Control when focus changes center a column. |
| `layout.master.position` | Place the master area left, right, or center. |
| `layout.master.default_width_fraction` | Set the master-area fraction. |
| `layout.master.new_on_top` | Place new stack windows at the top or bottom. |
| `layout.master.new_becomes_master` | Give the master slot to new windows. |
| `layout.dwindle.preserve_split` | Keep split directions fixed. |

### Examples

```toml
[[workspace]]
output = "HDMI-A-1"
name = "CHAT"
layout.mode = "scrolling"
layout.scrolling.center_focused = "always"

[[workspace]]
output = "HDMI-A-1"
name = "STATS"
layout.mode = "dwindle"

[[workspace]]
index = 4
output = "DP-1"
layout.gap = 0
layout.struts.top = 24
layout.scrolling.default_extent_fraction = 0.667
```

