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

# Draft user-facing changelog notes from conventional commits between two
# refs (default: latest tag..HEAD). Curate the output into CHANGELOG.md.
changelog-draft from='' to='HEAD':
    #!/usr/bin/env bash
    set -euo pipefail
    from="{{from}}"
    to="{{to}}"
    if [ -z "$from" ]; then
        from="$(git describe --tags --abbrev=0)"
    fi
    range="${from}..${to}"
    echo "Draft for ${range} — curate into CHANGELOG.md, not every line deserves to survive." >&2
    echo "" >&2

    # Commits whose message carries a BREAKING CHANGE footer.
    breaking="$(git log --no-merges --format='%s' --grep='BREAKING CHANGE' -F "$range" | sort -u)"

    # Kept types route to sections; everything else in the conventional
    # vocabulary is skipped silently; malformed or unknown subjects land in
    # Other so nothing disappears without a human looking at it.
    git log --no-merges --format='%s' "$range" | awk -v breaking="$breaking" '
        BEGIN {
            n = split("Breaking\nFeatures\nFixed\nChanged\nOther", order, "\n")
            skip = " chore ci docs deps refactor test build style release wip "
        }
        {
            subject = $0
            if (subject == "" || seen[subject]++) next
            is_breaking = index("\n" breaking "\n", "\n" subject "\n") > 0
            if (match(subject, /^[A-Za-z]+(\([^)]*\))?!?:[ \t]+/)) {
                head = substr(subject, 1, RLENGTH)
                sub(/[!?:][ \t]*$/, "", head)
                type = head
                sub(/[(!].*/, "", type)
                text = substr(subject, RLENGTH + 1)
                line = toupper(substr(text, 1, 1)) substr(text, 2)
                sub(/[.]+$/, "", line)
                if (line == "") next
                if (is_breaking)             sec = "Breaking"
                else if (type == "feat")     sec = "Features"
                else if (type == "fix")      sec = "Fixed"
                else if (type == "perf" || type == "revert") sec = "Changed"
                else if (index(skip, " " type " ") > 0) next
                else                         sec = "Other"
            } else {
                sec = "Other"
                line = subject
            }
            out[sec] = out[sec] "\n- " line
        }
        END {
            printed = 0
            for (i = 1; i <= n; i++) {
                sec = order[i]
                if (out[sec] == "") continue
                printf "### %s\n%s\n\n", sec, out[sec]
                printed = 1
            }
            if (!printed) print "(nothing changelog-worthy found — every commit was chores/docs/ci)"
        }
    '

# Owner-only workflow recipes (git-ignored; absent on other clones/CI)
import? 'local.just'
