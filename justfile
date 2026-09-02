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

# Format all source files
format:
    cargo fmt

# Check formatting without writing
format-check:
    cargo fmt --check

# Full local gate: formatting, lint, tests
verify: format-check lint test
