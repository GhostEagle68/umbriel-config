# umbriel-config

[![Release](https://img.shields.io/github/v/release/GhostEagle68/umbriel-config?include_prereleases&label=release)](https://github.com/GhostEagle68/umbriel-config/releases)
[![crates.io](https://img.shields.io/crates/v/umbriel-config)](https://crates.io/crates/umbriel-config)
[![Build](https://img.shields.io/github/actions/workflow/status/GhostEagle68/umbriel-config/rust.yml?branch=dev)](https://github.com/GhostEagle68/umbriel-config/actions/workflows/rust.yml)
[![Downloads](https://img.shields.io/github/downloads/GhostEagle68/umbriel-config/total)](https://github.com/GhostEagle68/umbriel-config/releases)
[![License](https://img.shields.io/crates/l/umbriel-config)](LICENSE.md)

A simple GUI configurator for the [Umbriel](https://github.com/noctalia-dev/umbriel)
Wayland compositor by the Noctalia team.

Open your config, click through the settings, hit **Save** — Umbriel
live-reloads and the change applies immediately.

> **Status: beta.** The core is usable but may encounter rough edges.
> [Bug reports and ideas are welcome](https://github.com/GhostEagle68/umbriel-config/issues).

## Disclaimer

Zcode GLM 5.3 and Claude Opus 5.5 models are used to help plan, review code, fix/find bugs and research only. All code is looked over and written by me and tested locally.

## Features

- **Always in sync with your umbriel** — setting pages are assembled at
  runtime from umbriel's own packaged default config, and the keybind action
  list is mined live from your installed compositor. New umbriel options and
  actions appear on their own; no umbriel-config update required.
- **Keybinds editor** — one merged list of built-in defaults and your binds,
  chord capture (press the keys), searchable pickers for keys and actions,
  submap scopes, and conflict warnings with a safe replace.
- **Understands split configs** — follows your `[include]` and
  `[include.optional]` chain
- **Changes you can audit** — every unsaved edit is one row: what changed,
  before vs after, with a per-option reset, including keybinds and rules.
  Discard anything before saving.
- **Backups you can restore** — every save snapshots your config into
  dated backup runs with a keep-limit, restoring first backs up the
  current files, and the restore browser shows a per-file diff before
  you commit.
- **Outputs, window rules, layer rules, security contexts** — including
  live monitor scanning and add-output; collapsible cards keep long
  lists readable.
- **Your file stays yours** — lossless TOML editing keeps every comment and
  piece of formatting; saves are atomic with a one-time `.bak` backup,
  a symlinked config (dotfiles) stays a symlink, and `umbriel validate`
  runs on every save.

## Install

Prebuilt binaries are attached to every release, no compiler needed.

```sh
curl -fsSL https://raw.githubusercontent.com/GhostEagle68/umbriel-config/dev/packaging/get.sh | sh
```

That picks the right build for your machine, verifies its checksum, and
installs into `~/.local`. Add `-s -- --prerelease` to follow the
pre-release channel, or `-s -- --version XXX` for an exact
version. Replace 'XXX' with exact version.

Prefer to do it manually? Download the tarball for your machine from the
[releases page](https://github.com/GhostEagle68/umbriel-config/releases)
(`x86_64` for most PCs, `aarch64` for ARM), then:

```sh
tar -xzf umbriel-config-x86_64-linux.tar.gz
cd umbriel-config-x86_64-linux
./install.sh
```

Either way you get the binary in `~/.local/bin` plus a launcher entry and
icon under `~/.local/share`, so most desktops list it in the app menu
after the next login. Make sure `~/.local/bin` is on your `PATH`, then run
`umbriel-config`. `PREFIX=/usr/local ./install.sh` installs system-wide
instead, and a `.sha256` checksum sits beside every tarball.

## Updating

Installed from a tarball, the app updates itself: **Settings → Updates →
Install update**, then Restart. The same card picks your channel —
**Stable** for tested releases, **Pre-release** for new features first.

Installed another way, the app tells you the command for it instead:

```sh
# cargo (pre-releases need the version)
cargo install umbriel-config@0.3.0-beta.1
# a clone of this repository
git pull && cargo build --release
```

### Build from source

**Dependencies:**

- Rust 1.88+ (includes cargo) install it with [rustup](https://rustup.rs);
  distro-provided Rust is usually older than the edition this needs
- Edition 2024

If you don't have Rust installed:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Then build and install umbriel-config, either from crates.io:

```sh
cargo install umbriel-config@0.3.0-beta.1
```

While every release is a prerelease, the version must be named since cargo's
plain `cargo install umbriel-config` skips prerelease versions by rule.
From the first stable release on, the unversioned command works. Or go
straight from the repository (tracks `main`):

```sh
cargo install --git https://github.com/GhostEagle68/umbriel-config
```

Want the latest, unreleased changes? The `dev` branch is work-in-progress,
expect rough edges and it has no releases, so the clean way to follow it
is from a clone:

```sh
git clone --branch dev https://github.com/GhostEagle68/umbriel-config
cd umbriel-config
cargo run --release
```

Updating is `git pull` plus a rebuild, and `git log HEAD..origin/dev` shows
exactly what changed since your last build — something a plain install can
never tell you. To keep the dev build on your PATH next to a stable install,
copy it under a dev name:

```sh
cp target/release/umbriel-config ~/.local/bin/umbriel-config-dev
```

`cargo install --git … --branch dev` also works, but it replaces your existing
`umbriel-config` binary and can't tell you what's new.

## Uninstall

How to remove umbriel-config depends on how it was installed.

**Installed with `cargo install`:**

```sh
cargo uninstall umbriel-config
```

This removes the binary from `~/.cargo/bin` and cargo's install registry.
To update instead, just re-run the install command above.

**Installed from a release tarball:**

```sh
rm ~/.local/bin/umbriel-config
rm ~/.local/share/applications/umbriel-config.desktop
rm ~/.local/share/icons/hicolor/scalable/apps/umbriel-config.svg
rm -r ~/.local/share/licenses/umbriel-config
```

The same list is in the `INSTALL.txt` that ships in the tarball.

Uninstalling only removes the app. Your Umbriel config files. And the
`.bak` backups it made on save are your own data and are left untouched.

## Usage

```sh
umbriel-config                # GUI on your config (~/.config/umbriel/config.toml)
umbriel-config --config PATH  # open a specific config file
umbriel-config path           # print which config would be opened
umbriel-config get|set ...    # debug CLI for single keys
umbriel-config outputs        # list outputs reported by the compositor
```

## Beta caveats

- Core settings work
- Expect bugs and missing features.
- Umbriel Config may have frequent updates due to how fast Umbriel changes, staying up-to-date is highly recommended.
- Every release so far is a pre-release, so new installs follow the
  **Pre-release** channel; Stable starts offering updates with the first
  stable release.
- Installs from GitHub release tarballs (x86_64 and aarch64), crates.io, or
  `cargo install --git`. AUR packaging is planned but not started.

## Development

```sh
just run      # or: cargo run
just verify   # fmt --check, clippy -D warnings, tests
just test
```

Conventions and architecture: [CONTRIBUTING.md](CONTRIBUTING.md).

## License

umbriel-config is [MIT](LICENSE.md)-licensed. The binary embeds two
typefaces under the [SIL Open Font License 1.1](https://openfontlicense.org)
(Inter, JetBrains Mono — full license texts in `assets/fonts/`) and is
built on the [Slint](https://slint.dev) UI toolkit, which SixtyFPS GmbH
distributes under its own licenses; this project uses and ships Slint
under the Slint Royalty-free Desktop, Mobile, and Web Applications
License 2.0 — attribution is given via the `AboutSlint` widget on the
Settings → About screen. Full details in the third-party notices at the
end of [LICENSE.md](LICENSE.md).
