# Contributing to umbriel-config

umbriel-config is a GUI configurator for the Umbriel Wayland compositor
(Noctalia team). Rust + Cargo, edition 2024. This document is the authoritative
reference for conventions; keep it accurate after every change that affects
them.

## Project layout

Two layers, strictly separated:

- `src/config/` — pure logic: discovery, document editing, typed model,
  validation. No GUI dependencies. Unit tests live in-file (`#[cfg(test)]`).
- UI (`src/main.rs`, later `src/ui/`). Reads and writes config only
  through the `config` module. No config parsing or path logic in UI code.

## Config editing guarantees

- The user's config is edited only through `toml_edit` documents; never
  round-trip via serde. Comments and layout must survive every save.
- Saving is atomic (write a temp file, rename over the target) and leaves a
  one-time `.bak` next to the config before the first modification.
- Mirror umbriel's config lookup exactly: `$XDG_CONFIG_HOME/umbriel/config.toml`,
  then `$XDG_CONFIG_DIRS` (default `/etc/xdg`), then the packaged
  `share/umbriel/config.toml`. `UMBRIEL_CONFIG` is harness-only; a user-pinned
  path (like `umbriel -c`) is a GUI feature, not part of env lookup.
- Config key names, action names, and file paths must match umbriel's canonical
  naming (see its `examples/config.toml`) — never invent variants.

## Code style

- `cargo fmt` before committing; `cargo clippy --all-targets -- -D warnings`
  passes with zero warnings.
- Keep it simple: minimal abstractions, no speculative generality, no filler
  comments. Stay within the task's scope, no drive-by refactors.
- Errors: `thiserror` types in the lib, `anyhow` in the bin.

## Naming

| Kind              | Convention             | Example                    |
|-------------------|------------------------|----------------------------|
| Files / modules   | snake_case             | `document.rs`              |
| Types             | PascalCase             | `ConfigDocument`           |
| Functions         | snake_case             | `resolve_path()`           |
| Constants         | SCREAMING_SNAKE_CASE   | `CONFIG_RELATIVE_PATH`     |
| TOML keys         | exactly as umbriel spells them | `duration_ms`, `prefer_no_csd` |

## Testing

- `cargo test`. Fixtures live in `tests/fixtures/` as copies of umbriel's
  `examples/config.toml`; re-copy when the upstream file changes.
- Discovery tests isolate `XDG_*` / `UMBRIEL_CONFIG` env vars.
- Format preservation is a tested guarantee: editing one value must leave all
  unrelated bytes of the file unchanged.

## Branching and commits

- `main` holds confirmed-working state; `dev` is the integration branch.
  New work lands on `dev` (topic branches optional:
  `type/short-description`, e.g. `feat/config-discovery`). After a change
  is confirmed working, merge `dev` into `main`. Tag releases on `main`.
- Conventional Commits: `type(scope): imperative summary`. Types: `feat`,
  `fix`, `refactor`, `docs`, `test`, `build`, `chore`, `perf`, `style`, `ci`.
  Scope = module or area (`config`, `ui`, `docs`).
- Small diffs, one concern per commit.

## Versioning

- Semver; the single source of truth is `version` in `Cargo.toml`.
- Version bumps happen only when cutting a release, never per commit. A
  release is: bump `version` in `Cargo.toml` in its own `chore(release):`
  commit, merge `dev` into `main`, and tag `v0.x.y` on `main`. Pushing the
  tag triggers the release workflow, which builds the Linux binary and
  attaches it (with checksums) to the GitHub Release.
- 0.x while the GUI matures: features and breaking changes bump the minor,
  fixes bump the patch.
- Release automation (cargo-release or release-plz) is added together with
  the first release, not before.

## Phase roadmap

1. Project setup — scaffolding, docs, private GitHub repo.
2. Config backend — discovery, toml_edit document, typed model, validation,
   debug CLI.
3. Minimal egui GUI — sidebar, first pages (Appearance, Animation, General),
   save action and validation-error banner.
4. Full section coverage — Input, Layout, Overview, Hot corners, Colors,
   outputs.
5. Advanced editors — keybind capture, window/layer rule lists, output config
   seeded from `umbriel outputs`.
6. Integration and polish — IPC events, profiles/undo, packaging, CI, and an
   optional UI-framework upgrade for polish.

