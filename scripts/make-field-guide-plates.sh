#!/usr/bin/env bash
# Remake the field guide's plates (game/ui/field_guide/plates/<id>-px.png) from the artbook:
# design/ui/round1's recipe (trim, box-filter down, 40 colours without dither, then 4x point
# up). Every plate keeps one art pixel = 4 texture px = 3 screen px (the page draws it at
# 0.75x). Art is fitted inside the plate box by the small size, at most 176 x 163 art
# pixels (528 x 489 screen px), never by scaling at draw time.
#
#   scripts/make-field-guide-plates.sh
set -euo pipefail
cd "$(dirname "$0")/.."
src=art/gen/artbook
out=game/ui/field_guide/plates
box=176x163
mkdir -p "$out"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
# guide id (game/data/field_guide.md) and its artbook image
while read -r id img; do
  magick "$src/$img" -trim +repage -filter box -resize "$box" -colors 40 -dither None "$tmp/small.png"
  magick "$tmp/small.png" -filter point -resize 400% "$out/$id-px.png"
  echo "$out/$id-px.png $(magick identify -format '%wx%h' "$out/$id-px.png")"
done <<'LIST'
lanternjaw lanternjaw.png
chorister chorister.png
frondgrazer frondgrazer.png
littershredder littershredder.png
bellwing bellwing.png
loftstrider loftstrider.png
capgnawer capgnawer-rose-bark.png
ripple-snail ripple-snail-organic.png
seedporter seedporter.png
bloomcrown bloomcrown-pink-preferred.png
umbrellafrond umbrellafrond.png
springturf springturf.png
stonecushion stonecushion.png
velvetpad velvetpad-provisional.png
glowcap glowcap-on-vaulttree.png
vaulttree vaulttree.png
lanternberry lanternberry-blue.png
siphonreed siphonreed.png
LIST
