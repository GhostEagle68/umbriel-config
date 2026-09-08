# Changelog

## [Unreleased]

### Changed

- The entire interface is rebuilt with Slint: a cleaner shell, a real
  theme system with bundled fonts, and settings pages grouped into
  cards instead of dense tables.
- Ranged number settings (opacity, timeouts, sizes) get a slider beside
  the value box — the box shows the value live while you drag.
- Editing now commits when you press Enter *or* click anywhere else, so
  a half-finished edit is never silently lost.
- The sidebar scrolls on short windows, its highlight follows the page
  you are on, and dropdown vocabularies (like pointer acceleration
  profiles) populate reliably.

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
