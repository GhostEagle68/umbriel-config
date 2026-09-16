#!/bin/sh
# Downloads the newest umbriel-config release, checks it against the
# published sha256, and installs it into ~/.local.
#
#   curl -fsSL https://raw.githubusercontent.com/GhostEagle68/umbriel-config/main/packaging/get.sh | sh
#
# Flags (after `| sh -s --`):
#   --prerelease   take the newest pre-release instead of the newest stable
#   --version X    install exactly that version, e.g. --version 0.3.0-beta.1
#
# PREFIX=/usr/local picks another install prefix, as in install.sh.
set -eu

repo=GhostEagle68/umbriel-config
channel=stable
version=

while [ $# -gt 0 ]; do
    case "$1" in
        --prerelease) channel=prerelease ;;
        --version) shift; version="${1:-}" ;;
        *) echo "error: unknown option '$1'" >&2; exit 1 ;;
    esac
    shift
done

for tool in curl tar sha256sum; do
    command -v "$tool" >/dev/null 2>&1 || {
        echo "error: $tool is required" >&2
        exit 1
    }
done

case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) echo "error: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac

# First tag in the API response: /releases/latest is the newest stable,
# /releases is every release newest-first.
newest_tag() {
    curl -fsSL "$1" 2>/dev/null |
        sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' |
        head -n 1
}

if [ -z "$version" ]; then
    if [ "$channel" = stable ]; then
        tag=$(newest_tag "https://api.github.com/repos/$repo/releases/latest")
        if [ -z "$tag" ]; then
            echo "No stable release yet — installing the newest pre-release."
            tag=$(newest_tag "https://api.github.com/repos/$repo/releases?per_page=1")
        fi
    else
        tag=$(newest_tag "https://api.github.com/repos/$repo/releases?per_page=1")
    fi
else
    tag="v$version"
fi

[ -n "$tag" ] || { echo "error: could not find a release to install" >&2; exit 1; }

asset="umbriel-config-$arch-linux.tar.gz"
base="https://github.com/$repo/releases/download/$tag"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

echo "Downloading umbriel-config $tag ($arch)…"
curl -fsSL "$base/$asset" -o "$work/$asset"
curl -fsSL "$base/$asset.sha256" -o "$work/$asset.sha256"

# The published file names the asset, so verify from inside the folder.
(cd "$work" && sha256sum -c "$asset.sha256" >/dev/null) || {
    echo "error: checksum mismatch — nothing was installed" >&2
    exit 1
}

tar -xzf "$work/$asset" -C "$work"

# Newer tarballs hold one top-level folder; older ones were flat.
installer=$(find "$work" -maxdepth 2 -name install.sh -type f | head -n 1)
if [ -n "$installer" ]; then
    sh "$installer"
else
    # Releases before install.sh shipped: the tarball is the prefix.
    prefix="${PREFIX:-$HOME/.local}"
    mkdir -p "$prefix"
    tar -xzf "$work/$asset" -C "$prefix" ./bin ./share
    chmod 755 "$prefix/bin/umbriel-config"
    echo "Installed $prefix/bin/umbriel-config"
fi
