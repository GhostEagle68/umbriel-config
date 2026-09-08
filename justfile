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
    for f in ui/*.slint; do slint-lsp format -i "$f"; done

# Check .slint formatting without writing
format-slint-check:
    #!/usr/bin/env sh
    for f in ui/*.slint; do
        slint-lsp format "$f" | diff -u "$f" - || exit 1
    done

# Full local gate: formatting, lint, tests
verify: format-check lint test

# Owner-only workflow recipes (git-ignored; absent on other clones/CI)
import? 'local.just'
