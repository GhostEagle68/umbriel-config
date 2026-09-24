#!/bin/sh
# Installs umbriel-config from this extracted release folder.
#
# Defaults to ~/.local, the per-user prefix most desktops already read:
#   ./install.sh
#   PREFIX=/usr/local ./install.sh     (system-wide; needs sudo)
#
# Re-running overwrites an existing install, which is how you downgrade
# or repair one by hand.
set -eu

prefix="${PREFIX:-$HOME/.local}"
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

if [ ! -f "$here/bin/umbriel-config" ]; then
    echo "error: run this script from the extracted release folder" >&2
    exit 1
fi

install -Dm755 "$here/bin/umbriel-config" "$prefix/bin/umbriel-config"
install -Dm644 "$here/share/applications/umbriel-config.desktop" \
    "$prefix/share/applications/umbriel-config.desktop"
# Absolute Exec: some sessions (Umbriel's among them) start apps with a
# PATH of just /usr/local/bin:/usr/bin, where a bare name isn't found.
sed -i "s|^Exec=umbriel-config$|Exec=$prefix/bin/umbriel-config|" \
    "$prefix/share/applications/umbriel-config.desktop"
install -Dm644 "$here/share/icons/hicolor/scalable/apps/umbriel-config.svg" \
    "$prefix/share/icons/hicolor/scalable/apps/umbriel-config.svg"
for license in "$here"/share/licenses/*; do
    [ -f "$license" ] || continue
    install -Dm644 "$license" "$prefix/share/licenses/umbriel-config/$(basename "$license")"
done

# Without this the launcher can take until the next login to appear.
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$prefix/share/applications" 2>/dev/null || true
fi

echo "Installed $prefix/bin/umbriel-config"
case ":$PATH:" in
    *":$prefix/bin:"*) echo "Run it with: umbriel-config" ;;
    *) echo "Add $prefix/bin to your PATH, then run: umbriel-config" ;;
esac
