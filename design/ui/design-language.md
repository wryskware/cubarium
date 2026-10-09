---
design_status: decided
decision_refs: [D-0006]
last_reviewed: 2026-10-09
---

# Game UI design language

Wrysk's picks from UI rounds 1–3 (2026-10-08/09), for the Godot app's menus: the field guide, populations, world and settings. The normal cube display stays free of UI.

These are his choices, but they aren't in the decision ledger. The mockups are `round1/`, `round2/` and `round3/index.html`. `round2/godot/` draws the type in Godot.

## Direction

The direction is **pixel Neon diorama**. In his words: "something around pixel neon diorama lands best for me". He ruled out hybrid pixel/smooth.

The UI is a 1920×1080 layout drawn at native pixels, with no stretch. On a 1440p screen it sits centred.

## Type

There are three roles.

| Role | Font | Setting | Sizes |
|---|---|---|---|
| Title | Gridlite PE variable (Rosetta Type) | `'wght' 700, 'BACK' 90, 'RECT' 120, 'ELSH' 3.5` | 91 px name (13 px grid ×7), 52 px big numbers (×4) |
| Chrome (tabs, labels, clock, keycaps) | Gridlite PE variable | `'wght' 691, 'BACK' 135, 'RECT' 71, 'ELSH' 2.65` | 26 px (×2), upper case, 2 px letter spacing; keycaps as typed, no spacing |
| Text (lists, field note, facts, chips) | Pixelify Sans | weight 400 | 23 px field note, 21 px lists and facts, 18 px chips and small text |

The title has an offset shadow one title pixel deep: 7 px, #5A1D9D.

**Gridlite's axes.** All four are continuous.

| Axis | Range | What it controls |
|---|---|---|
| `wght` | 1–900 | Size of the lit dots |
| `BACK` | 1–900 | Unlit dots behind each letter |
| `RECT` | 1–900 | Unlit dots across the whole line |
| `ELSH` | 1–4 | Dot shape: 1 rows, 2 columns, 3 squares, 4 rounds |

The grid is 400 units at 5200 upm, so 13 px per dot at 1×. Capitals are 7 dots tall.

**Gridlite licensing.**
- The game needs a Rosetta licence before it ships. Wrysk is fine with the roughly €90 cost.
- The Standard licence covers "App Projects", priced by app users, but doesn't name games, so confirm with Rosetta.
- The trial is licensed for testing only. It also replaces 2, Q, X, q and x with a placeholder box.
- The Adobe web kit has no minus sign. The bought font has one.

**Pixelify.** It is under the OFL. Its 5 reads like an S. Wrysk's call: "i kinda still want pixelify at 23px.. maybe revise later if it ends up being too distracting".

**Font files.** No font files go into git; `round2/fonts/fonts.json` lists where each one comes from.

## Colour

The palette is round 1's, and it hasn't been reviewed on its own.

| Token | Value | Use |
|---|---|---|
| ink | #F7F0FF | titles, values |
| ink2 | #DCD1F7 | body text, counts |
| accent | #42C5F8 | focus, live numbers, charts |
| hot | #F25BDB | selection, rules, kicker line |
| warm | #FDA243 | paused / warnings |
| labels | #A595D8 | fact and section labels |
| tabs | #E6DEFF on rgba(11,5,37,.88) | menu tabs |
| line | rgba(176,55,196,.32) | inner boxes, dividers |
| panel | #C23FD3 | main panel edge |

Wrysk chose brighter tabs only; labels keep the round 1 lavender. Gridlite's 2 px chrome dots show at about 60–80 % of their set colour, so its real contrast is lower than these values suggest.

## Panels

- **Border:** a notched 3 px line with its corners cut. The main panel uses `panel` with a 48 px soft glow, rgba(176,55,196,.30). Inner boxes (plate, live count, search, tab bar) use `line`.
- **Fill:** dithered bands.
  - Five steps, #2A1470, #24115F, #1D0D50, #160A41 and #100632, change at 18, 36, 56 and 76 % of the height.
  - Each edge is blended across ±15 px with a 4×4 Bayer ordered dither in 3 px cells.
- **Behind panels:** the world shows through a 6 px checker scrim: rgba(7,4,26,.88) and .62 in 3 px squares.

## Controls: lit dots

Gridlite's negative setting, `'wght' 1, 'BACK' 700, 'RECT' 700`, lights the dot plate and cuts the letters out.

- **Selected tab:** negative label in hot, with one space of plate on each side. Unselected tabs are plain chrome.
- **Main button:** negative label in #FFD6F6, with no frame.
- **Pressed:**
  - The main button's label switches to a bright plate, `'wght' 900, 'BACK' 240, 'RECT' 240`.
  - Both buttons drop 3 px.
- **Second button:** chrome label with a visible unlit plate, `'BACK' 320, 'RECT' 220`, inside a faint notched ring.
- **Disabled:** 40 % opacity.
- **Selected list row:** 2 px pink dots on a 4 px grid, rgba(242,91,219,.32), with a 6 px hot bar at the left.
- **Chips:** a faint dot plate; selected chips get a brighter plate and white text.
- **Keycaps:** negative labels.

## Focus: glow

A 3 px notched cyan line sits 6 px outside the focused thing, with an 18 px cyan glow, rgba(66,197,248,.55).

## Layout notes

- At 23 px the Lanternjaw entry is taller than the panel. The entry scrolls and fades out at the bottom, and the buttons stay fixed below it.
- Prey chips wrap to four rows. The real screen needs a decision on long entries.

## Godot notes

These come from `round2/godot/specimen.gd`.

- **Fonts:**
  - One `FontVariation` per role with `variation_opentype`, keyed by `TextServer.name_to_tag`. Fractional axis values work.
  - Gridlite: antialiasing gray, hinting none, subpixel positioning off. Use whole multiples of 13 px.
  - Pixelify: default antialiasing and light hinting.
  - Letter spacing: `FontVariation.spacing_glyph`.
  - Godot draws Pixelify almost exactly like Chrome.
- **Drawing:**
  - Notched rings are four rectangles, or a nine-patch with cut corners.
  - The dithered fill is a generated texture per panel size, or a Bayer shader.
  - The scrim is a three-line canvas shader.
- **Godot project:** no stretch mode.

## Not designed yet

- Settings controls: sliders, toggles, lists of options.
- The populations / viewer metrics screen.
- Long field-guide entries.
- Palette review.
- Sound.
