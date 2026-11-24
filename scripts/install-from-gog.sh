#!/bin/sh
# Unpack the GOG Jagged Alliance installer (setup_jagged_alliance_*.exe) into game/ beside this script.
# Needs innoextract (Debian/Ubuntu: sudo apt install innoextract; Fedora: sudo dnf install innoextract).
set -e
cd "$(dirname "$0")"
command -v innoextract >/dev/null || { echo "install-from-gog: innoextract is missing (sudo apt install innoextract)" >&2; exit 1; }
SETUP=$(ls game/setup_jagged_alliance_*.exe ./setup_jagged_alliance_*.exe "$HOME"/Downloads/setup_jagged_alliance_*.exe 2>/dev/null | grep -v deadly_games | head -1)
[ -n "$SETUP" ] || { echo "install-from-gog: put setup_jagged_alliance_*.exe into game/ (or ~/Downloads)" >&2; exit 1; }
echo "unpacking $SETUP"
mkdir -p game/Jagged-Alliance
innoextract -s -e "$SETUP" -d game/Jagged-Alliance
ls game/Jagged-Alliance/JA1.gog game/Jagged-Alliance/JA.EXE >/dev/null 2>&1 || ls -R game/Jagged-Alliance | grep -qi "ja1.gog" \
  || { echo "install-from-gog: JA1.gog not found after unpacking" >&2; exit 1; }
echo "done: start ./ja1front"
