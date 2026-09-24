# umbriel-config — development tasks

# List available tasks
default:
    @just --list

# Build the debug binary
build:
    cargo build

# Run the binary
run:
    cargo run

# Run all tests
test:
    cargo test

# Lint: clippy with warnings as errors
lint:
    cargo clippy --all-targets -- -D warnings

# Format all source files (Rust + Slint; slint-lsp: cargo install slint-lsp)
format: format-slint
    cargo fmt

# Check formatting without writing (Rust + Slint)
format-check: format-slint-check
    cargo fmt --check

# Format .slint files in place
format-slint:
    #!/usr/bin/env sh
    for f in ui/*.slint ui/pages/*.slint; do slint-lsp format -i "$f"; done

# Check .slint formatting without writing
format-slint-check:
    #!/usr/bin/env sh
    for f in ui/*.slint ui/pages/*.slint; do
        slint-lsp format "$f" | diff -u "$f" - || exit 1
    done

# Full local gate: formatting, lint, tests
verify: format-check lint test

# Refresh the bundled copy of umbriel's user docs, which the settings pages
# are built from (the app also fetches them itself when checking for updates)
refresh-umbriel-docs:
    #!/usr/bin/env bash
    set -euo pipefail
    api="https://api.github.com/repos/noctalia-dev/umbriel/contents/docs/user?ref=main"
    out="assets/umbriel-docs.md"
    tmp="$(mktemp)"
    trap 'rm -f "$tmp"' EXIT
    curl -fsSL -H "User-Agent: umbriel-config" "$api" |
        jq -r '.[] | select(.type == "file" and (.name | endswith(".md"))) | "\(.name) \(.download_url)"' |
        LC_ALL=C sort | while read -r name url; do
            printf '<!-- umbriel-config page: %s -->\n' "$name" >> "$tmp"
            curl -fsSL -H "User-Agent: umbriel-config" "$url" >> "$tmp"
            printf '\n' >> "$tmp"
        done
    [[ -s "$tmp" ]] || { echo "error: no docs downloaded" >&2; exit 1; }
    mv "$tmp" "$out"
    trap - EXIT
    # umbriel is MIT-licensed: its notice travels with the copy.
    curl -fsSL -H "User-Agent: umbriel-config" \
        "https://raw.githubusercontent.com/noctalia-dev/umbriel/main/LICENSE" \
        -o assets/LICENSE-umbriel-docs.txt
    echo "Wrote $out ($(grep -c '^<!-- umbriel-config page:' "$out") pages)"

# Owner-only workflow recipes (git-ignored; absent on other clones/CI)
import? 'local.just'
