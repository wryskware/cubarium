#!/usr/bin/env bash
# Put the game UI's fonts into game/fonts/ (they are never committed; see game/fonts/README.md).
#
#   scripts/fetch-ui-fonts.sh
#
# - Pixelify Sans (OFL) is downloaded from Google Fonts' repository.
# - Gridlite PE is copied from ~/Downloads/GridlitePEVFTRIAL-All.ttf if it is there. That
#   file is Rosetta Type's trial, licensed for testing only.
# Prints what is still missing. Run `godot --headless --path game --import` afterwards.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
dest="$root/game/fonts"
mkdir -p "$dest"
missing=0

pixelify="PixelifySans[wght].ttf"
if [ ! -s "$dest/$pixelify" ]; then
  url="https://raw.githubusercontent.com/google/fonts/main/ofl/pixelifysans/PixelifySans%5Bwght%5D.ttf"
  if curl -fsSL "$url" -o "$dest/$pixelify.part"; then
    mv "$dest/$pixelify.part" "$dest/$pixelify"
    echo "fetched $pixelify"
  else
    rm -f "$dest/$pixelify.part"
    echo "missing: $pixelify (download failed: $url)"
    missing=1
  fi
else
  echo "have $pixelify"
fi

gridlite="GridlitePEVFTRIAL-All.ttf"
if [ ! -s "$dest/$gridlite" ]; then
  if [ -s "$HOME/Downloads/$gridlite" ]; then
    cp "$HOME/Downloads/$gridlite" "$dest/$gridlite"
    echo "copied $gridlite (trial: testing only)"
  else
    echo "missing: $gridlite (Rosetta Type's Gridlite PE trial; put it in ~/Downloads and rerun)"
    missing=1
  fi
else
  echo "have $gridlite"
fi

exit $missing
