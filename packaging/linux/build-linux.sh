#!/bin/sh
# Baut Klassenwerk für Linux und packt es als .tar.gz (auf einem Linux-Rechner mit Rust ausführen).
set -e
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT/app"
cargo build --release
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(uname -m)
OUT="$ROOT/dist/Klassenwerk-$VERSION-linux-$ARCH"
rm -rf "$OUT" && mkdir -p "$OUT"
cp target/release/Klassenwerk "$OUT/klassenwerk"
strip "$OUT/klassenwerk" 2>/dev/null || true
cp assets/icon-256.png "$OUT/klassenwerk.png"
GLIBC=$(objdump -T "$OUT/klassenwerk" 2>/dev/null | grep -o "GLIBC_[0-9.]*" | sort -Vu | tail -1 | cut -d_ -f2)
echo "Dieses Paket braucht glibc ${GLIBC:-?} oder neuer (Ubuntu 22.04 = 2.35, Ubuntu 24.04 = 2.39)." > "$OUT/SYSTEM.txt"
cp "$ROOT/LICENSE" "$ROOT/THIRD_PARTY_LICENSES.txt" "$OUT/"
cp "$ROOT/packaging/linux/klassenwerk.desktop" "$ROOT/packaging/linux/install.sh" "$ROOT/packaging/linux/uninstall.sh" "$ROOT/packaging/linux/LIESMICH.txt" "$OUT/"
chmod +x "$OUT/install.sh" "$OUT/uninstall.sh" "$OUT/klassenwerk"
tar -C "$ROOT/dist" -czf "$OUT.tar.gz" "$(basename "$OUT")"
echo "Fertig: $OUT.tar.gz"
