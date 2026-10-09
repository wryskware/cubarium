# UI fonts

The neon UI (`game/ui/neon/`) uses two fonts. The font files are never committed; `scripts/fetch-ui-fonts.sh` puts them here. The `.import` files are committed so the import settings travel with the repository.

| File | Font | Source | Import |
|---|---|---|---|
| `PixelifySans[wght].ttf` | Pixelify Sans (variable weight), OFL | https://github.com/google/fonts/tree/main/ofl/pixelifysans | antialiasing gray, hinting light |
| `GridlitePEVFTRIAL-All.ttf` | Gridlite PE variable, Rosetta Type | Rosetta's trial download, copied from `~/Downloads/` | antialiasing gray, hinting none, subpixel positioning off, no mipmaps, no MSDF |

Gridlite PE:
- The game needs a Rosetta Type licence before it ships (design/ui/design-language.md, "Gridlite licensing").
- The trial is licensed for testing only.
- The trial replaces 2, Q, X, q and x with a placeholder box, so sample numbers like 42 and keycap X show a box until the bought font replaces it.

After fetching, run `godot --headless --path game --import`.
