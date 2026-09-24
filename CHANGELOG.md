# Changelog


## [0.3.0-beta.4] — 2026-09-24

### Highlights

**Keybinds settings are redesigned.** The Keybinds page is rebuilt as a modern shortcut editor.
- **Edit in place.** Click any bind to edit it right there in the list; click one of Umbriel's defaults to override it.
- **Record keys.** Press the combination instead of typing it; Umbriel passes your existing shortcuts through while recording, so they're captured instead of triggered. In the search box, pressing a key finds every bind that uses it.
- **Pick an app.** "Launch an app" binds pick from your installed apps (AppImages included) instead of a typed command. Shell-command binds offer the usual commands for media and brightness keys, and warn when Umbriel can't find the command.
- **Easier to scan.** Keys show as keycaps, launch binds show their command, and groups collapse. Filter by All, Yours or Defaults.
- **All bind options.** Allow when shortcuts are inhibited and cooldown can now be set, and the shortcuts inhibition toggle is in the action list.
- **Guided parameters.** The parameter field adapts to the action: a dropdown for fixed choices, hints for workspaces and sizes, hidden when unused.

### 🚀 Features

#### General

- Feat(keybinds): redesign the keybind editor


## [0.3.0-beta.3] — 2026-09-24

### 🐛 Fixed

#### Guide

- Clean up guided setup and add Cancel
  - Guided setup: drop the empty heading and lone card toggle, hide the
    NEW and owning-file badges on its rows, and add Cancel when re-run
    from Settings (discards the filled-in suggestions; first run has none).
  - Setting rows: number, color and open-choice editors now line up on
    the right edge like the other editors.

#### Packaging

- Launch from the app menu in sessions with a minimal PATH
  The desktop entry ran umbriel-config by bare name. Umbriel starts apps
  with PATH=/usr/local/bin:/usr/bin, so a ~/.local install never opened
  from the launcher (it did on Plasma, which reads the login profile).
  install.sh now writes the absolute binary path into the installed entry.

#### Settings

- Restart after an in-app update
  "Restart now" failed with "No such file or directory" once an update
  had installed. The installer replaces the running binary, after which
  /proc/self/exe points at the deleted old file. The executable path is
  now read at startup, before any update can replace it.

#### General

- Fix(settings): close the save dialog after saving; guard double install

### 🛠 Build

#### Changelog

- List build and packaging changes in release notes

#### General

- Cut CI and release build times
  - CI: install slint-lsp 1.17.1 from Slint's prebuilt release instead
    of compiling it (was ~8 of the 10 minutes).
  - Release: publish with --no-verify; the build job has already built
    the tagged commit, so the verify rebuild (~11 min) was a duplicate.
  - Dev profile: line-tables-only debug info; incremental rebuilds drop
    from ~7.7 s to ~3.3 s.


## [0.3.0-beta.2] — 2026-09-23

### 🐛 Fixed

#### Settings

- Build setting pages from umbriel's docs
  umbriel now installs a short starter config instead of its full
  reference, so most setting pages showed their options as read-only
  "other" rows. The pages are now built from umbriel's user docs, with
  their ranges and value lists, plus anything only the installed config
  has. A copy of the docs ships with the app; newer docs download at most
  once a day when update checks are on, or right away with Sync schema.



## [0.3.0-beta.1] — 2026-09-23

### Highlights

**Shader editor upgrades.**

- **Builder when editing.** Editing or forking a shader opens the same
  effect builder as a new one, with its steps read back from the code,
  and hand-edited code is never overwritten.
- **Thirteen new effects.** The builder adds Rotate, Swirl, Ripple,
  Pixelate, Stretch, Flip, Drop, Iris, Wipe, Dissolve, Desaturate, Tint
  and Ghost trail, and any effect can be stacked more than once.
- **Live sliders.** Dragging a builder slider updates the code and the
  preview as you move it, and each slider shows its current value.
- **Play.** The preview can play the animation on a loop at 0.3, 0.6
  or 1.2 seconds, instead of only being scrubbed by hand.
- **Rename.** Your own shaders can be renamed from the editor, and every
  event using the shader follows it to the new name.
- **Preview as any event.** Pick which event the preview plays as: it
  shows a fitting stand-in (a window, a workspace, a bar, a focus border)
  and plays with that event's own curve and timing from your config.
- **Choose where it is used.** Saving a shader asks which events should
  use it, showing what each uses now, so a new effect is assigned in the
  same step.

### 🚀 Features

#### Shaders

- When editing a shader
  or forking a shader it now opens the same effect builder as New
  shader, with its steps and slider values read back from the code. The
  builder also stays after the first Save of a new shader, where it used
  to disappear. If the code was changed by hand, the builder locks rather
  than overwriting your edits. "Start over with builder" replaces the
  code with a fresh stack after you confirm. Typing code that matches
  the builder's output unlocks it again.
- The effect builder gains Rotate, Swirl, Ripple, Pixelate, Stretch,
  Flip, Drop, Iris, Wipe, Dissolve, Desaturate, Tint and Ghost trail,
  each with its own sliders. Like the existing effects, every one plays
  from hidden to the plain window and can be stacked more than once.
  Ghost trail reuses the previous frame, which umbriel notes costs extra
  GPU memory, so its name notes that.
- Builder sliders only took effect when you let go. Now the code and the
  preview follow the slider while you drag it, and each slider shows its
  current value next to it, rounded to what the code will use.

- The shader preview could only be scrubbed by hand. A Play button now
  runs the animation on a loop, with a short hold on the last frame, at
  0.3, 0.6 or 1.2 seconds. Grabbing the scrubber pauses it, and direction
  and builder changes keep applying while it plays.
- The Name field in the shader editor is now editable when editing one
  of your own shaders. Saving under a new name renames the file, and
  every event using the shader switches to the new name straight away,
  written to your config at once so a later Discard can't leave it
  pointing at the old name. Taken names are refused, and changing only
  the name counts as an unsaved change. Forking a shader whose "-fork"
  name is already taken now suggests "-fork-2", "-fork-3" and so on.
- The shader preview always showed one window opening or closing. A
  "Preview as" picker now plays it as any of the nine events, each on a
  stand-in for what umbriel really shades: a window, a workspace's
  windows, the overview filmstrip, a scratchpad window over its backdrop,
  a bar-shaped layer, or a border ring. Play and the scrubber follow that
  event's curve and length from your config (springs included), and the
  shader gets eased and linear progress just as umbriel passes them. The
  editor opens on the event the shader is assigned to.

- Saving a shader now asks which events should use it: a checklist of
  all nine, each showing what it uses now, pre-ticked with the event
  being previewed and any already using the shader. Ticked events switch
  to it and unticked ones stop using it, as changes saved with the main
  Save button, so a new effect is assigned in the same step it's made.
- Save a shader without changing assignments. The "Use for" step on Save gains a Save only button: it keeps the
  shader file and leaves every event's assignment exactly as it was.

### 🐛 Fixed

#### Backups

- Keep files that share a name apart
  Backups stored each file by name only, so two config files with the
  same name in different folders overwrote each other, and a restore
  could write one over the other. Each backup now records every file's
  full path and restores it there. Backups made before this still
  restore by file name.

#### Config

- Edit a setting in the file whose value umbriel uses
  When two included files set the same key, umbriel uses the one listed
  last, but editing, resetting or moving that setting acted on the first.
  The edit could land in a file umbriel ignores for that key, and the
  save popup could move the wrong value. Edits now go to the same file
  whose value umbriel applies: the main config, then later includes over
  earlier ones.
- Open your own config on a fresh install
  With umbriel installed but no ~/.config/umbriel/config.toml yet, the
  app opened umbriel's packaged default in the system folder instead.
  First-run setup never appeared and every save failed with "Permission
  denied". The app now always edits your own config file, so a machine
  without one gets the first-run setup and saving creates the file.
- Load optional includes
  Files listed under [include.optional] were ignored, so settings and
  keybinds from them (like a Noctalia theme file) were missing and the
  app showed the wrong current values. They now load after the regular
  includes, the same order umbriel uses, and a missing optional file is
  skipped without a warning.
- Saving keeps a symlinked config a symlink
  Saving replaced a symlinked config.toml (a dotfiles setup) with a
  plain file, so the dotfiles repo stopped receiving changes. Saves now
  write to the file the link points at, and the one-time .bak stays next
  to the link, out of the dotfiles repo.

#### Shaders

- Save shader assignments for every animation event
  Picking a shader in an Assignments dropdown looked like it worked, but
  the change was thrown away: the file path was written without quotes,
  the edit was rejected, and nothing reached Save. Picks are now written
  as proper strings into the file that already holds that event's shader
  (new ones start in your main config, and the save popup can move them),
  and the dropdowns always redraw from what is really in your config.
- Let a missing shader assignment be cleared
  When an event pointed at a shader file that no longer exists, its
  dropdown showed "(no shader)", so picking "(no shader)" did nothing and
  the broken assignment couldn't be removed. The dropdown now shows the
  missing file as its own entry, and choosing "(no shader)" clears it.
- Delete the shader you clicked, after confirming
  The Delete button on a shader card ignored which card it was on. It
  deleted whichever shader the editor had last opened (so after editing
  one shader and cancelling, deleting another removed the first), or did
  nothing at all. It now deletes that card's own file. Both Delete
  buttons, on the card and in the editor, ask for a second click first,
  because a deleted file can't be recovered.
- Refuse to overwrite an existing shader when saving a new one
  Saving a new or forked shader under a name you already used silently
  replaced the old file. The editor now stops and asks for another name,
  or points you to Edit on the existing shader.
- Indent every line of builder-generated code
  The Glow pulse and Shatter effects wrote their first line of code
  flush against the left margin, so the generated shader looked
  misaligned in the code view. Every line is now indented to match.
- Only flag community updates when the installed version is known
  A community shader collection downloaded before the app started
  recording which version it fetched was always reported as "update
  available", even when it was current. With no recorded version the
  page now leaves the indicator alone. Updating once records the version
  and turns the check back on.
- Stay on the current page when a download finishes
  If you left the Shaders page while the community shaders were
  downloading, the app jumped back to Shaders when the download
  finished. It now refreshes the Shaders page only while you are on it.
- Open the preview mid-animation instead of on a blank frame
  The shader editor's preview started at the first frame. For effects
  that fade in, the window is fully transparent there, so the preview
  looked empty or broken. It now opens halfway through, so the effect is
  visible right away; scrub to see the rest.
- New shader assignments go in the file that holds the others
  Assigning a shader to an event that had none always put it in the main
  config.toml, even when your other assignments live in an included
  shaders.toml. New assignments now start in the file that already holds
  the most of them. With none anywhere, an included shaders.toml is used,
  otherwise the main config. The save popup can still move any of them.
- Ask before closing the editor with unsaved code
  Cancel closed the shader editor straight away, throwing out any code
  you had typed or built since the last save. With unsaved changes it
  now asks "Discard unsaved changes?" with Keep editing and Discard.
  Nothing changed since opening or saving still closes immediately.
- Tab indents in the shader code editor
  Tab in the code pane jumped focus to the next control, like in a web
  form. Tab now indents to the next 4-space stop, Shift+Tab outdents,
  both work on every selected line, and Enter keeps the current line's
  indentation. A line above the code lists these keys.
- Keep typing smooth in the shader code editor
  Every keystroke re-checked the code, re-read it into the builder,
  rebuilt every builder card and slider, and recompiled the preview.
  That work now runs once you pause typing, and the builder cards are
  rebuilt only when the steps actually change. Using the builder right
  after typing still sees your latest code.
- Recognise shader paths the way umbriel resolves them
  Assignments were matched as plain text, so a shader written as
  ./shaders/x.glsl or as a full path showed as missing even though
  umbriel loads it fine. Paths are now resolved the way umbriel does it,
  relative to the file that sets them, so every spelling of the same file
  is recognised. A value starting with ~ gets a clear warning, because
  umbriel doesn't expand it. A real file outside the shader library is
  shown as such instead of as missing. New assignments, and keys moved to
  another file in the save popup, get a path that resolves from the file
  they end up in.
- Deleting a shader also clears its assignments
  Deleting a shader that was still assigned left those events pointing
  at a file that no longer exists. The delete confirmation now names the
  events using it ("Delete? Used by Windows out, Overview"), and deleting
  clears those assignments, ready to save.
- Close the editor after saving a new shader
  Saving a new or forked shader flipped the editor into Edit mode for
  the file it had just written, which looked like a second screen
  popping up. It now closes and says the shader was created, ready to
  assign. Saving an existing shader still keeps the editor open for more
  tweaks.
- Find shaders in symlinked folders
  Shader folders linked into shaders/ (as dotfile managers like stow set
  up) were skipped, so their shaders never appeared. Linked folders are
  now scanned too, each real folder once, so a link that loops back can't
  hang the page.
- Lift the community download size cap
  Downloading the community shaders read at most 10 MB, so once the
  collection grew past that the download would fail with an unclear
  error. The limit is now 64 MB.
- Accept any spacing in the animation() signature
  The editor warned that "vec4 animation(vec2 uv)" was missing whenever
  the signature was spaced differently, such as "vec4  animation (" or
  with a line break, even though the shader compiles fine. Any valid
  spacing is now accepted; a misspelled name still warns.
- The same builder effect can be added twice
  Adding Glow pulse or Shatter a second time made the shader fail to
  compile, because both copies declared the same variables. Each such
  effect now sits in its own block, so any effect can be stacked more
  than once. Shaders saved by earlier versions still open in the builder.
- Fade and Shatter builder effects ran backwards
  The builder's Fade made an opening window fade out and end invisible
  (and the reverse for closing windows), and Shatter left an opening
  window still in pieces at the end. Both now finish on the whole,
  visible window. Shaders saved with the old versions still open in the
  builder, and the next builder change updates them.
- Close the editor after every save
  Saving an existing shader from Edit left the editor open, unlike saving
  a new one. Every successful save now closes the editor and returns you
  to the Shaders page.
- Deleting a shader saves its cleared assignments at once
  Deleting a shader cleared its assignments only as unsaved changes, so
  "Discard all" brought them back pointing at a file that no longer
  exists. The cleared assignments are now written to your config
  together with the delete; other unsaved changes stay unsaved.
- Renaming or deleting a shader keeps other settings' change marks
  After the guided setup, renaming or deleting an assigned shader made
  the setup's suggested values show as changed, because the whole file's
  "last saved" state was reset to what was on disk. Only the assignment
  that was written is updated now.
- Show translucent effects in the preview as umbriel draws them
  The preview multiplied colour by transparency a second time, so fades,
  dissolve edges and the scratchpad backdrop looked darker and murkier
  than they really are. Shader output is now shown as umbriel returns it.
- Editing a shader doesn't pre-tick events it isn't used for
  Saving an edited shader pre-ticked the event being previewed in the
  "Use for" checklist, so a quick edit-and-save could switch that event
  to the shader. Editing now only pre-ticks events already using it; new
  and forked shaders still start with the previewed event ticked.
- Arrow keys keep moving a builder slider
  Pressing an arrow key on a builder slider moved it once, then the
  slider lost focus because every card was rebuilt. Only that step's
  card is refreshed now, so the slider keeps focus.
- Download community shaders without a shared temp file
  The community archive was saved to a fixed name in /tmp, which another
  user on the machine could create first. It is now written to a new
  hidden file inside your shaders folder and removed afterwards.
- Back up before renaming or deleting an assigned shader
  Renaming or deleting a shader that events use writes the updated
  assignments straight to your config. That write now takes a backup
  first, like every other save, labelled "shader" on the Backups page.
- Undo in the code editor no longer crashes
  Undo after Tab, Shift+Tab, Enter or a builder change could crash the
  app, because the text box's own undo history pointed at positions in
  text that had since been replaced. The code editor now keeps its own
  history: Ctrl+Z undoes, Ctrl+Shift+Z or Ctrl+Y redoes, quick typing
  and a whole slider drag each undo as one step, and every opened shader
  starts with a fresh history.

#### UI

- Discarding changes fully reverts them and clears Save
  "Discard all" and the save popup's per-key Reset left the Save button
  highlighted, and pages other than settings cards (Shaders, Outputs,
  rules) kept showing the discarded values. Reverting a newly added
  setting also left an empty section header in the file, so the app
  still treated it as changed. Discarding now restores the file exactly,
  clears Save, and redraws whatever page you are on.
- Moving a setting to another file leaves no empty section
  Choosing a different file for a setting in the save popup moved the
  value but left its now-empty section header, such as
  [animation.windows_move], behind in the original file. That header is
  now removed along with the value.
- Accept text values for settings outside the schema
  Editing a setting the packaged config doesn't document wrote text such
  as a path or a name without quotes, so the edit was rejected with
  "umbriel would reject …" and lost. Text is now quoted, while numbers,
  booleans, arrays and already-quoted strings go in as typed.
- Discarding a new setting keeps sections already in the file
  Discarding or resetting a newly added setting also removed a parent
  section that was already in the file but empty, such as a bare
  [animation] header. The file then still differed from what's on disk,
  so Save stayed lit with nothing to save. Only sections the edit itself
  created are removed now.
- Keybind and rule edits can be saved and discarded
  Changing only a keybind, or a field of a window or layer rule, left
  Save saying "Nothing to save", and Discard all didn't undo it. These
  edits now appear in the list of changes like any other setting, can be
  reset one by one, and Discard all restores every file exactly.

### ⚡ Performance

#### Shaders

- Preview compiles only the newest code
- Resolve assignments once per Shaders page redraw
- Draw the preview once per change
  Switching the preview event or opening the editor queued a new target,
  new code and a render together, and each was drawn separately. The
  preview now applies them all and draws a single frame.

### 📦 Other

#### General

- Update README with new version for umbriel-config for cargo install

## [0.2.6-beta.1] — 2026-09-16

### 🚀 Features

- (ui) Color picker popup for color swatches

### 🐛 Fixed

- (ui) Repair invalid-looking setting inputs and add open-choice fields
  Several setting rows across the app looked broken or gave no feedback
  on bad input:

  - Invalid edits (bad color, out-of-range number) silently reverted
    with no explanation; rows now show a red border and inline error.
  - Ranged numeric rows showed a slider even when the value was unset,
    and hid it in other cases due to a dead NaN check.
  - A `mine_comments` section-tracking bug let commented example keys
    inside `[[window_rule]]`/`[[layer_rule]]` blocks leak into the prior
    `[layout.master]` section, producing duplicate phantom rows; the
    duplicate-guard also failed to update as entries were mined, letting
    repeats through.
  - `mine_range` took the first word before a dash as the min value, so
    any prose-prefixed range ("Inner border width, 0-100 logical
    pixels") silently dropped its slider across Appearance/Blur/Shadows.
  - Hot-corner actions and window/layer/security-context rule outputs
    were plain text boxes with no hint of valid values; both now get a
    new `Kind::OpenChoice` widget — a dropdown of known values (actions
    mirrored from Umbriel's docs, live output names for rules) paired
    with a free-text box that stays in sync, so open-ended values
    (`spawn:...`, unlisted outputs) remain fully editable.
  - The dropdown's popup width was tied to the closed box's width and
    truncated long action names; widened to fit the longest entries.
  - A real empty string (e.g. `input.keyboard.layout` meaning "system
    default") looked identical to a broken blank box; text fields now
    show a light "empty" placeholder to distinguish it from actual
    content, without affecting the literal unset `"—"` placeholder.
  - The Reset button only existed on ranged-number rows even though the
    changed-indicator dot lights up for every kind; Reset now works
    uniformly across all editable kinds.
  - Window-rule size fields (`default_size`, `default_width`,
    `default_height`) were renamed upstream to
    `default_floating_size_px`/`default_floating_size`/
    `default_scrolling_extent_px`/`default_scrolling_extent`; rules.rs
    and document.rs updated to match, with new `rule_size_px`/
    `rule_size_fraction` document helpers mirroring the existing
    `rule_position` pattern.

### 📦 Other

- Change get.sh URL to use the dev branch for install

## [0.2.5-beta.2] — 2026-09-15

### Highlights

**Shaders page.** Discover, download and assign umbriel's GLSL animation
effects: the ones bundled with umbriel, your own, and a community
collection the app downloads and updates for you over HTTPS, no git
needed. Every animation event gets a dropdown, files umbriel would
reject are flagged before they can be saved, and a shaders.toml your
include chain ignores is called out with a one-click fix.

**Shader editor with a visual effect builder.** 
Build your own effects without writing GLSL: stack steps like Fade, Scale, Slide, Shatter,
Wobble and Glow pulse, each with its own sliders, reorder them, and the
shader composes itself with the generated code in view for hand-tweaking.
A preview renders the shader offscreen so you can scrub the animation
like a video timeline, and the GLSL errors it reports as you type are the
ones umbriel would report, down to the line numbers. Your own shaders
open for editing, bundled and community ones fork into editable copies,
and saving live-reloads in umbriel.

**Update channels, and updates from inside the app.** 
Choose Stable for tested releases or Pre-release for new features first, in Settings ->
Updates. Installed from a release tarball, the app can now download and
install a new version itself and restart into it; cargo, package-manager
and source builds are given the exact command for their setup instead.

### 🚀 Features

- (shaders) Add shader discovery and management for animation effects
- (shaders) Enhance community shader management with download and update functionality
- (shaders) Implement shader update availability check and UI indication
- (shaders) Visual effect builder and in-app shader editor
- (shaders) Preview scrubber with live GLSL error reporting
- (ui) Fluid resizing across pages, header, and editors
- (search) Surface whole pages and keybinds in results
- (updates) Choose between the Stable and Pre-release channels
- (updates) Install updates from the app on tarball installs
- (ui) One-time notice explaining the new update channels

### 🐛 Fixed

- Assigning a shader from a dropdown did nothing: the callback was
  declared but never connected, so no edit was recorded and Save
  stayed disabled.
- Clearing a shader back to "(no shader)" or any other edit that
  removes a key was invisible to Save: the change counter stayed at
  zero and the save popup found "nothing to save" even though the
  document was modified. Deletions now count as changes, appear in the
  popup as "(removed)", can be discarded, and reset restores them.
- The save popup preselected the wrong destination file for changes
  living in included files (off by one in the dropdown).
- The community-shaders download looked up the config directory from
  the environment, so with `XDG_CONFIG_HOME` unset the button failed
  with "Could not determine the config directory". It now targets the
  folder of the config the app opened, and the library refreshes as
  soon as the download lands.
- Save-time validation no longer appends "umbriel validate exited
  unsuccessfully without reporting diagnostics" to ordinary warnings.
  umbriel exits nonzero for any diagnostic, warnings included, and
  warning-only configs still apply. Warnings now show in the banner
  by themselves and never block a save.
- (ui) Update shader removal indication from "(built-in)" to "(no shader)"

### 📦 Other

- Update version numbers in README.md
- Update shader assignment and save behavior descriptions in changelog


## [0.2.0-beta.1] — 2026-09-11

First release on the Slint interface. The old EGUI interface is gone, and a lot
arrived with the new one.

### Features

- **Backups you can restore.** Every save snapshots your config into a
  dated backup run (keeping the last ten by default), the restore
  browser shows a per-file diff before you commit, and restoring first
  backs up the current files; a restore is itself reversible.
- **Keybinds editor.** Your binds and the compositor defaults in one
  searchable list, grouped by family. Press the keys to capture a chord,
  pick actions from your installed umbriel's live vocabulary, and
  conflicts are caught before they land; removing your bind restores the
  default.
- **Window rules, layer rules, and security contexts.** Visual editors
  for every rule family including rules that live in split-out files
  like windowrules.toml, which open and edit in place. Add or remove
  rules.
- **Outputs, per monitor.** One card per connected display with
  resolution and refresh offered from the modes it actually reports,
  HDR, VRR, scale and more, plus Add output for displays not yet
  configured.
- **Guided setup.** A short walk through the settings people change
  most, offered when umbriel is detected; values you already set are
  left alone.
- **What's new in the app.** Release notes are bundled into the binary:
  shown once per version after an update, and readable any time from
  Settings → View changelog. Update checks now also show the new
  version's notes.
- **Dark and light theme**, switchable in Settings.

### Changed

- The entire interface is rebuilt with Slint: a cleaner shell, a real
  theme system with bundled fonts, sidebar navigation with curated
  pages, and settings grouped into collapsible cards instead of dense
  tables.
- The shipped binary is about 27% smaller.
- Chain-wide search finds a setting by name across every page, and
  each row shows which file it lives in.
- Editing commits on Enter or when you click elsewhere, so a
  half-finished edit is never silently lost.

### Fixed

- Output workspace lists stored as TOML arrays are read and written
  correctly instead of falling back to "dynamic".
- Slider reset works and sliders hold their values at the extremes.
- The sidebar highlight follows dedicated pages (keybinds, outputs,
  rules) as well as regular settings pages.

## [0.1.2-alpha.3] — 2026-09-06

### Features

- Settings page with an update check, a once-a-day automatic check, and
  a startup badge when an update is available.
- Section pages now span your whole include chain: a topic overview
  shows where each setting lives, and per-file pages remain the editors.
  New settings can be placed in the main config or any include — even a
  brand-new file created on the spot.
- Settings that appeared since your last schema sync are marked with a
  ● badge everywhere they render, with a Dismiss on the drift banner.
- Easing-curve keys pick from umbriel's built-in curve names instead of
  a free-text field.
- Color settings can be unset again, and unset colors are only offered
  on files that can own them.
- Numeric editors show a live preview with units while you type.
- Sidebar exit button with an unsaved-changes confirmation.
- Packaging: desktop entry, icon, and install-friendly release tarballs
  for both x86_64 and aarch64, plus publishing to crates.io.

### Fixed

- The update check no longer hangs forever (10 second timeout) and
  works on repos that only publish pre-releases.
- Sidebar group headings are larger and render bullet markers in the
  right font.
- File pages scroll their embedded editors again.

## [0.1.0-alpha.1] — 2026-09-04

First alpha release of the umbriel-config GUI.

### Features

- Edit umbriel's `config.toml` and include chains: settings are grouped
  into sections mined from umbriel's own packaged default, with search
  across every file.
- Kind-aware editors: toggles, numbers, text, arrays, dropdowns built
  from mined vocabularies, and color pickers.
- Keybinds editor: chord capture, searchable actions, umbriel's built-in
  defaults shown, and binds merged across include files.
- Window and layer rule editors, plus a raw view of every key beyond
  the schema.
- Outputs page listing live outputs via wlr-output-management.
- Saves are validated through `umbriel validate` with error/warning
  banners; schema sync flags settings an umbriel update added or
  removed.
- Fresh start when no config exists, including directory creation.
- Debug CLI over the same config backend (`umbriel-config` binary).
