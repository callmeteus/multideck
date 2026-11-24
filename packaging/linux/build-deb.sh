#!/usr/bin/env sh
# Builds a Debian package for the current architecture.
#
#   cargo build --release -p multideck-desktop
#   ./packaging/linux/build-deb.sh
#
# Output: dist/multideck_<version>_<arch>.deb
# Requires dpkg-deb (Debian or Ubuntu). Run it on Linux.

set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -n 1)
version=${version:-0.1.0}
arch=$(dpkg --print-architecture)
binary="$root/target/release/multideck"

if [ ! -x "$binary" ]; then
    echo "release binary not found; run: cargo build --release -p multideck-desktop" >&2
    exit 1
fi

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT

install -Dm0755 "$binary" "$stage/usr/bin/multideck"
install -Dm0644 "$root/packaging/linux/multideck.desktop" "$stage/usr/share/applications/multideck.desktop"
install -Dm0644 "$root/assets/brand/logo-mark.svg" "$stage/usr/share/icons/hicolor/scalable/apps/multideck.svg"
install -Dm0644 "$root/packaging/linux/60-multideck.rules" "$stage/usr/lib/udev/rules.d/60-multideck.rules"
mkdir -p "$stage/DEBIAN"

size=$(du -sk "$stage/usr" | cut -f1)
cat > "$stage/DEBIAN/control" <<EOF
Package: multideck
Version: $version
Section: utils
Priority: optional
Architecture: $arch
Installed-Size: $size
Maintainer: MultiDeck contributors
Homepage: https://github.com/callmeteus/multideck
Description: Host several stream decks behind one adapter layer
 MultiDeck drives a virtual deck, Elgato Stream Deck hardware and, in a later
 milestone, the Stream Deck Mobile app, with one profile format and one action
 set.
EOF

mkdir -p "$root/dist"
dpkg-deb --build --root-owner-group "$stage" "$root/dist/multideck_${version}_${arch}.deb"
echo "built $root/dist/multideck_${version}_${arch}.deb"
