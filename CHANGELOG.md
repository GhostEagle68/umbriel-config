# Changelog

## [Unreleased]

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
