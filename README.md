# umbriel-config

A stupid-simple GUI configurator for the [Umbriel](https://github.com/noctalia-dev/umbriel)
Wayland compositor by the Noctalia team.

Open your config, click through the settings, hit **Save** — Umbriel
live-reloads and the change applies immediately.

> **Status: alpha.** The core is usable for daily driving; rough edges
> remain. [Bug reports and ideas are welcome](https://github.com/GhostEagle68/umbriel-config/issues).

## Features

- **Always in sync with your umbriel** — setting pages are assembled at
  runtime from umbriel's own packaged default config, and the keybind action
  list is mined live from your installed compositor. New umbriel options and
  actions appear on their own; no umbriel-config update required.
- **Keybinds editor** — one merged list of built-in defaults and your binds,
  chord capture (press the keys), searchable pickers for keys and actions,
  submap scopes, and conflict warnings with a safe replace.
- **Understands split configs** — follows your `[include]` chain; keybind
  edits land in whichever file owns them, and the changes page reviews every
  file at once.
- **Changes you can audit** — every unsaved edit is one row: what changed,
  before vs after, with a per-option reset. Discard anything before saving.
- **Outputs, window rules, layer rules** — including live monitor scanning.
- **Your file stays yours** — lossless TOML editing keeps every comment and
  piece of formatting; saves are atomic with a one-time `.bak` backup, and
  `umbriel validate` runs on every save.

## Install

Requires a recent stable Rust toolchain (edition 2024, Rust 1.88+).

```sh
cargo install --git https://github.com/GhostEagle68/umbriel-config
```

For the alpha, pin the tag to match a release:

```sh
cargo install --git https://github.com/GhostEagle68/umbriel-config --tag v0.1.0-alpha.1
```

## Usage

```sh
umbriel-config                # GUI on your config (same lookup as umbriel)
umbriel-config --config PATH  # open a specific config file
umbriel-config path           # print which config would be opened
umbriel-config get|set ...    # debug CLI for single keys
umbriel-config outputs        # list outputs reported by the compositor
```

## Alpha caveats

- Only the **keybinds** page follows the `[include]` chain so far; the
  schema, rules, and raw pages edit the main config file.
- No distro packaging yet (AUR/Flatpak later); no crates.io publish yet —
  install is from this repository.
- Eye-candy and polish pass still pending.

## Development

```sh
just run      # or: cargo run
just verify   # fmt --check, clippy -D warnings, tests
just test
```

Conventions and architecture: [CONTRIBUTING.md](CONTRIBUTING.md).
Licensed under [MIT](LICENSE.md).
