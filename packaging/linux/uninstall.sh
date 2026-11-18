#!/bin/sh
# Entfernt das Programm. Ihre Daten in ~/.local/share/Klassenwerk bleiben erhalten.
BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
rm -rf "$DATA/doc/klassenwerk"
rm -f "$BIN/klassenwerk" "$DATA/applications/klassenwerk.desktop" "$DATA/icons/hicolor/256x256/apps/klassenwerk.png"
echo "Klassenwerk wurde entfernt. Ihre Daten liegen weiterhin in $DATA/Klassenwerk."
