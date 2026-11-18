#!/bin/sh
# Baut Klassenwerk.app für macOS (Apple Silicon + Intel in einer Datei) und packt sie als .zip.
# Auf einem Mac mit Rust (https://rustup.rs) und den Xcode-Kommandozeilenwerkzeugen ausführen:
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
#   sh packaging/macos/build-macos.sh
set -e
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT/app"
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
APP="$ROOT/dist/Klassenwerk.app"
rm -rf "$APP" && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/Klassenwerk" \
    target/aarch64-apple-darwin/release/Klassenwerk target/x86_64-apple-darwin/release/Klassenwerk
sed "s/@VERSION@/$VERSION/g" "$ROOT/packaging/macos/Info.plist" > "$APP/Contents/Info.plist"
# icon: .icns from the 1024 px PNG
ICONSET="$(mktemp -d)/Klassenwerk.iconset"
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
    sips -z $s $s assets/icon.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
    sips -z $((s*2)) $((s*2)) assets/icon.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Klassenwerk.icns"
cp "$ROOT/LICENSE" "$ROOT/THIRD_PARTY_LICENSES.txt" "$APP/Contents/Resources/"
# ad-hoc signature (needed on Apple Silicon); not notarized
codesign --force --deep --sign - "$APP"
cd "$ROOT/dist" && rm -f "Klassenwerk-$VERSION-macos.zip"
ditto -c -k --keepParent Klassenwerk.app "Klassenwerk-$VERSION-macos.zip"
echo "Fertig: $ROOT/dist/Klassenwerk-$VERSION-macos.zip"
