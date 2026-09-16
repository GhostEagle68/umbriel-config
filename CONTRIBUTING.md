# Contributing to umbriel-config

umbriel-config is a GUI configurator for the Umbriel Wayland compositor
(Noctalia team). Rust + Cargo, edition 2024. This document is the authoritative
reference for conventions.

## Project layout

Two layers, strictly separated:

- `src/config/` — pure logic: discovery, document editing, typed model,
  validation. No GUI dependencies. Unit tests live in-file (`#[cfg(test)]`).
- `src/slint_ui/` — the shell: `mod.rs` (state and startup), one
  `page_*.rs` per page, plus `rows.rs`, `save.rs`, `search.rs`,
  `sections.rs`, `common.rs`. Layout lives in `ui/`: `app-window.slint`
  (shell, overlays), `ui/pages/*.slint` (one per page), `widgets.slint`,
  `types.slint`, `theme.slint`. A new page is a `page_*.rs` beside a
  `ui/pages/*.slint`, never another arm in an existing file.
- UI code reads and writes config only through the `config` module. No
  config parsing or path logic in UI code.
- `packaging/` — `install.sh` and `INSTALL.txt` ship inside the release
  tarball; `get.sh` backs the one-line install in the README.

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

| Kind            | Convention                     | Example                      |
| --------------- | ------------------------------ | ---------------------------- |
| Files / modules | snake_case                     | `document.rs`                |
| Types           | PascalCase                     | `ConfigDocument`             |
| Functions       | snake_case                     | `resolve_path()`             |
| Constants       | SCREAMING_SNAKE_CASE           | `CONFIG_RELATIVE_PATH`       |
| TOML keys       | exactly as umbriel spells them | `duration_ms`, `prefer_no_csd` |

## Testing

- `cargo test`. Tests live in-file under `#[cfg(test)]`; fixtures are
  inline `const` strings next to the tests that use them, copied from
  umbriel's `examples/config.toml` where a realistic file is needed.
- Discovery tests isolate `XDG_*` / `UMBRIEL_CONFIG` env vars.
- Format preservation is a tested guarantee: editing one value must leave all
  unrelated bytes of the file unchanged.

## Branching and commits

- All work lands on `dev` (topic branches required for PRs:
  `type/short-description`, e.g. `feat/config-discovery`). **Nothing is
  ever committed directly to `main`** — a stable release fast-forwards
  `main` to `dev`, so the two are identical afterwards.
- Conventional Commits: `type(scope): imperative summary`. Types: `feat`,
  `fix`, `refactor`, `docs`, `test`, `build`, `chore`, `perf`, `style`, `ci`.
  Scope = module or area (`config`, `ui`, `shaders`, `docs`).
- Small diffs, one concern per commit.
- **`feat:` and `fix:` subjects are the changelog.** git-cliff turns them
  into release notes as `- (scope) Subject`, so write them for users, not
  for reviewers. A fix nobody outside the repo would notice is a `chore:`.
- Two commit footers change what ships in the notes:
  - `Changelog: <text>` replaces the generated line with your wording.
  - `Changelog: skip` drops the commit from the notes entirely.
  - Matching is a substring over the footer, so avoid the word "skip"
    inside a `Changelog:` note you do want published.

## Versioning

- Semver; the single source of truth is `version` in `Cargo.toml`.
- Version bumps happen only when cutting a release, never per commit, and
  the release recipes do them for you in a `chore(release):` commit.
- **Two channels, one command each**, both run from `dev`:

  | | Pre-release | Stable |
  | --- | --- | --- |
  | Command | `just prerelease 0.3.0-beta.1` | `just stable 0.3.0` |
  | Branches | tags `dev`, leaves `main` alone | tags `dev`, fast-forwards `main` to it |
  | Changelog section | since the previous tag of any kind | since the previous **stable** tag (betas rolled up) |
  | GitHub | pre-release | latest |
  | crates.io | needs `cargo install umbriel-config@<version>` | plain `cargo install umbriel-config` |

  Each recipe checks the tree, runs the gate, bumps `Cargo.toml`,
  generates the CHANGELOG.md section with git-cliff, opens it in
  `$EDITOR` for a once-over, then commits, tags and pushes. Pushing the
  tag triggers the release workflow: it refuses a tag on the wrong
  branch, publishes to crates.io, then attaches both arch tarballs with
  checksums and notes built from the changelog section. Published
  versions are immutable, so a burned tag is deleted and re-made.
- 0.x while the GUI matures: features and breaking changes bump the minor,
  fixes bump the patch.
  