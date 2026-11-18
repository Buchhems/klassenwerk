#!/bin/sh
# Installiert Klassenwerk für den aktuellen Benutzer (ohne Root-Rechte).
set -e
DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$BIN" "$DATA/applications" "$DATA/icons/hicolor/256x256/apps"
install -m 755 "$DIR/klassenwerk" "$BIN/klassenwerk"
mkdir -p "$DATA/doc/klassenwerk"
install -m 644 "$DIR/LICENSE" "$DIR/THIRD_PARTY_LICENSES.txt" "$DATA/doc/klassenwerk/"
install -m 644 "$DIR/klassenwerk.png" "$DATA/icons/hicolor/256x256/apps/klassenwerk.png"
sed "s|^Exec=.*|Exec=$BIN/klassenwerk|" "$DIR/klassenwerk.desktop" > "$DATA/applications/klassenwerk.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$DATA/applications" || true
echo "Klassenwerk ist installiert: $BIN/klassenwerk"
echo "Es erscheint im Anwendungsmenü unter „Bildung“ bzw. „Büro“."
echo "Für den KI-Assistenten (MCP): $BIN/klassenwerk --mcp"
