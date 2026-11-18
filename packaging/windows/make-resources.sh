#!/bin/sh
# Erzeugt die Windows-Ressourcen (Programmsymbol + Versionsangaben) für Klassenwerk.exe:
#   app/assets/windows/klassenwerk.res  (wird beim MSVC-Build eingebunden)
#   app/assets/windows/klassenwerk.obj  (wird beim GNU-Build eingebunden)
# Nach jeder Versionsänderung in app/Cargo.toml ausführen (braucht llvm-rc und llvm-cvtres,
# z. B. aus dem Paket "llvm" unter Linux oder aus LLVM für Windows).
set -e
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
D="$ROOT/app/assets/windows"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/app/Cargo.toml" | head -1)
V1=$(echo "$VERSION" | cut -d. -f1); V2=$(echo "$VERSION" | cut -d. -f2); V3=$(echo "$VERSION" | cut -d. -f3 | sed 's/[^0-9].*//')
TMP=$(mktemp -d)
sed -e "s/@VERSION@/$VERSION/g" -e "s/@V1@/$V1/g" -e "s/@V2@/$V2/g" -e "s/@V3@/$V3/g" "$D/klassenwerk.rc" > "$TMP/klassenwerk.rc"
cp "$D/klassenwerk.ico" "$TMP/"
(cd "$TMP" && llvm-rc /C 65001 /l 0x407 /fo klassenwerk.res klassenwerk.rc)
llvm-cvtres /machine:X64 /out:"$D/klassenwerk.obj" "$TMP/klassenwerk.res" >/dev/null
cp "$TMP/klassenwerk.res" "$D/klassenwerk.res"
rm -rf "$TMP"
echo "Windows-Ressourcen für Version $VERSION erzeugt."
