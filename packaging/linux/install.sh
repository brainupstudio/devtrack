#!/bin/sh
# Installs DevTrack for the current user: the program in ~/.local/bin, the icon and the menu
# entry in ~/.local/share. Run it from the unpacked folder: ./install.sh
set -eu
cd "$(dirname "$0")"
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons/hicolor/512x512/apps"
install -m 755 devtrack "$HOME/.local/bin/devtrack"
install -m 644 devtrack.png "$HOME/.local/share/icons/hicolor/512x512/apps/devtrack.png"
install -m 644 devtrack.desktop "$HOME/.local/share/applications/devtrack.desktop"
echo "DevTrack installed. Open it from the applications menu, or run: devtrack"
