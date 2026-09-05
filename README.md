# umbriel-config

A simple GUI configurator for the [Umbriel](https://github.com/noctalia-dev/umbriel)
Wayland compositor by the Noctalia team.

Open your config, click through the settings, hit **Save** — Umbriel
live-reloads and the change applies immediately.

> **Status: alpha.** The core is usable but has rough edges, recommended for using as testing right now.
> [Bug reports and ideas are welcome](https://github.com/GhostEagle68/umbriel-config/issues).

# Disclaimer
Zcode GLM 5.3 models were used to help plan, review code and research only. All code is looked over by me and tested locally.

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

Prebuilt binaries are attached to every release — no compiler needed.

```sh
mkdir -p ~/.local/bin
curl -fLO https://github.com/GhostEagle68/umbriel-config/releases/latest/download/umbriel-config-x86_64-linux.tar.gz
tar -xzf umbriel-config-x86_64-linux.tar.gz -C ~/.local/bin
```

Make sure `~/.local/bin` is on your `PATH`, then run `umbriel-config`. A
`.sha256` checksum sits next to the tarball on the release page.

Build from source instead (Rust 1.88+, edition 2024):

```sh
cargo install --git https://github.com/GhostEagle68/umbriel-config
```

## Uninstall

How to remove umbriel-config depends on how it was installed.

**Installed with `cargo install`:**

```sh
cargo uninstall umbriel-config
```

This removes the binary from `~/.cargo/bin` and cargo's install registry.
To update instead, just re-run the `cargo install --git` command above.

**Installed from a release tarball:**

```sh
rm ~/.local/bin/umbriel-config
```

Uninstalling only removes the app. Your Umbriel config files. And the
`.bak` backups it made on save are your own data and are left untouched.

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
- Linux x86_64 only for now; no AUR or crates.io packaging yet.
- Eye-candy and polish pass still pending.

## Development

```sh
just run      # or: cargo run
just verify   # fmt --check, clippy -D warnings, tests
just test
```

Conventions and architecture: [CONTRIBUTING.md](CONTRIBUTING.md).
Licensed under [MIT](LICENSE.md).
