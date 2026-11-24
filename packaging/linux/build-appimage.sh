#!/usr/bin/env sh
# Builds an AppImage for the current architecture.
#
#   cargo build --release -p multideck-desktop
#   ./packaging/linux/build-appimage.sh
#
# Output: dist/MultiDeck-<version>-x86_64.AppImage
# Requires linuxdeploy and appimagetool on PATH, or network access to fetch them
# into packaging/linux/.tools. Run it on Linux.

set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -n 1)
version=${version:-0.1.0}
tools="$root/packaging/linux/.tools"
appdir="$root/dist/MultiDeck.AppDir"
binary="$root/target/release/multideck"

if [ ! -x "$binary" ]; then
    echo "release binary not found; run: cargo build --release -p multideck-desktop" >&2
    exit 1
fi

mkdir -p "$tools" "$root/dist"
fetch() {
    url=$1
    target=$2
    if [ ! -x "$target" ]; then
        echo "fetching $url"
        curl -L --fail -o "$target" "$url"
        chmod +x "$target"
    fi
}

fetch "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage" "$tools/linuxdeploy"
fetch "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" "$tools/appimagetool"

rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/scalable/apps"
install -m0755 "$binary" "$appdir/usr/bin/multideck"
install -m0644 "$root/packaging/linux/multideck.desktop" "$appdir/usr/share/applications/multideck.desktop"
install -m0644 "$root/assets/brand/logo-mark.svg" "$appdir/usr/share/icons/hicolor/scalable/apps/multideck.svg"

export APPIMAGE_EXTRACT_AND_RUN=1
"$tools/linuxdeploy" --appdir "$appdir" --desktop-file "$appdir/usr/share/applications/multideck.desktop" --icon-file "$appdir/usr/share/icons/hicolor/scalable/apps/multideck.svg"

out="$root/dist/MultiDeck-${version}-x86_64.AppImage"
"$tools/appimagetool" "$appdir" "$out"
echo "built $out"
