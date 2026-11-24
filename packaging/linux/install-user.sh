#!/usr/bin/env sh
# Installs MultiDeck for the current user, without touching system paths.
#
#   cargo build --release -p multideck-desktop
#   ./packaging/linux/install-user.sh
#
# The udev rule needs root and is only required for USB decks. The script
# prints the command instead of running it.

set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
binary="$root/target/release/multideck"

if [ ! -x "$binary" ]; then
    echo "release binary not found at $binary" >&2
    echo "run: cargo build --release -p multideck-desktop" >&2
    exit 1
fi

bin_dir="${XDG_BIN_HOME:-$HOME/.local/bin}"
apps_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
icons_dir="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"

mkdir -p "$bin_dir" "$apps_dir" "$icons_dir"
install -m 0755 "$binary" "$bin_dir/multideck"
install -m 0644 "$root/packaging/linux/multideck.desktop" "$apps_dir/multideck.desktop"
install -m 0644 "$root/assets/brand/logo-mark.svg" "$icons_dir/multideck.svg"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps_dir" || true

echo "installed multideck to $bin_dir"
echo "make sure $bin_dir is in PATH"
echo
echo "for USB decks, install the udev rule as root:"
echo "  sudo cp $root/packaging/linux/60-multideck.rules /etc/udev/rules.d/"
echo "  sudo udevadm control --reload-rules && sudo udevadm trigger"
