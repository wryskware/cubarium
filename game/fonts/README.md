# UI fonts

The neon UI (`game/ui/neon/`) uses two fonts. The font files are never committed; `scripts/fetch-ui-fonts.sh` puts them here. The `.import` files are committed so the import settings travel with the repository.

| File | Font | Source | Import |
|---|---|---|---|
| `PixelifySans[wght].ttf` | Pixelify Sans (variable weight), OFL | https://github.com/google/fonts/tree/main/ofl/pixelifysans | antialiasing gray, hinting light |
| `GridlitePEVFTRIAL-All.ttf` | Gridlite PE variable, Rosetta Type | Rosetta's trial download, copied from `~/Downloads/` | antialiasing gray, hinting light (matches the browser mockups; none renders dimmer and blurrier), subpixel positioning off, no mipmaps, no MSDF |

Trial fonts for the chrome-dots comparison (2026-10-11, `game/tests/font_trial_shots.gd`). Nothing in the game uses them; `scripts/fetch-ui-fonts.sh` doesn't fetch them. The trial loads each file directly with gray antialiasing, hinting none and subpixel positioning off, so they need no `.import` file.

| File | Font | Source | Licence |
|---|---|---|---|
| `Doto[ROND,wght].ttf` | Doto (variable: ROND dot shape, wght dot size) | https://github.com/google/fonts/tree/main/ofl/doto (upstream https://github.com/oliverlalan/Doto) | OFL 1.1 (`OFL.txt` in both repos) |
| `Punctum[ROND,wght].ttf` | Punctum v1.000 (variable: ROND dot shape, wght dot size) | https://github.com/sundyme/punctum, `fonts/` | OFL 1.1 for the fonts (`OFL.txt`); the repo's MIT `LICENSE` covers only its site and tooling |
| `MatrixMonoScreen-Regular.ttf` | Matrix Mono Screen v1.800 (static, square dots) | https://github.com/FriedOrange/MatrixSans, release v1.800 zip, `fonts/ttf/` | OFL 1.1 (`OFL.txt`) |

Gridlite PE:
- The game needs a Rosetta Type licence before it ships (design/ui/design-language.md, "Gridlite licensing").
- The trial is licensed for testing only.
- The trial replaces 2, Q, X, q and x with a placeholder box, so sample numbers like 42 and keycap X show a box until the bought font replaces it.

After fetching, run `godot --headless --path game --import`.
