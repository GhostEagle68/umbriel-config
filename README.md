# umbriel-config

A simple, easy-to-use GUI configurator for the Umbriel Wayland compositor by
the Noctalia team.

**Status: early development.** The config backend is being built first; there
is no GUI yet.

## How it works

- Finds your config the same way Umbriel does: `UMBRIEL_CONFIG` pin, then the
  XDG lookup chain, then the packaged default.
- Edits `~/.config/umbriel/config.toml` in place with lossless TOML editing —
  comments and formatting in your hand-written config survive every change.
- Umbriel live-reloads its config on save, so applied changes are immediate.
- Before the first modification, a one-time `config.toml.bak` backup is
  written next to your config.

## Development

Requires a recent stable Rust toolchain (edition 2024).

```sh
cargo run     # debug CLI (GUI in a later phase)
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
```

Conventions: [CONTRIBUTING.md](CONTRIBUTING.md)
