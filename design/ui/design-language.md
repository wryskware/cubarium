---
design_status: decided
decision_refs: [D-0006]
last_reviewed: 2026-10-09
---

# Game UI design language

Wrysk's picks from UI rounds 1–3 (2026-10-08/09), for the Godot app's menus: the field guide, populations, world and settings. The normal cube display stays free of UI.

Recorded as D-0006, a living direction: when Wrysk changes something, update this page. The mockups are `round1/`, `round2/` and `round3/index.html`. `round2/godot/` draws the type in Godot.

## Direction

The direction is **pixel Neon diorama**. In his words: "something around pixel neon diorama lands best for me". He ruled out hybrid pixel/smooth.

The UI is a 1920×1080 layout drawn at native pixels, with no stretch. On a 1440p screen it sits centred. The window opens at 1920×1080, clamped to a smaller screen's usable area (his 2560×720 display: 1920×720). A window smaller than 1920×1080 shrinks the whole UI, and there Gridlite text draws solid: each dot zone and the letters fill their cells flat at the dotted version's light per area (Wrysk, 2026-10-11: "when we scale, we default back to solid").

## Type

There are three roles.

| Role | Font | Setting | Sizes |
|---|---|---|---|
| Title | Gridlite PE variable (Rosetta Type) | `'wght' 700, 'BACK' 90, 'RECT' 120, 'ELSH' 3.5` | 91 px name (13 px grid ×7), 52 px big numbers (×4) |
| Chrome (tabs, labels, clock, keycaps) | Gridlite PE variable | `'wght' 691, 'BACK' 135, 'RECT' 71, 'ELSH' 2.65` | 26 px (×2), upper case, 2 px letter spacing; keycaps as typed, no spacing |
| Text (lists, field note, facts, chips) | Pixelify Sans | weight 400 | 23 px field note, 21 px lists and facts, 18 px chips and small text |

The title has an offset shadow one title pixel deep: 7 px, #5A1D9D.

Titles carry **one space on each side**, so Gridlite's unlit dot field runs one letter past each end. Wrysk, 2026-10-09: "yes space. definitely." In Godot this is the `NeonTitle` label (`game/ui/neon/neon_title.gd`): set its `title`, and its `pad` adds the spaces.

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

## Settings controls (round 4)

Wrysk's picks, 2026-10-09: "Arrows, Dot Slider, Checkbox", and the dot row for progress ("dot row is honestly cleaner, and it matches the dot slider"). The mockup is `round4/index.html`.

- **Choice from a list: arrows.**
  - The value sits centred in an inner box (52–60 px tall), with 3×5 pixel chevrons at 4× on each side. They are labels-coloured, or cyan when focused.
  - Left and right step through the options; a click on an arrow steps too.
  - Under the value is a row of 6 px pips, one per option, with the current one in hot.
- **Steps: dot slider.**
  - A track of 6 px square dots on a 9 px pitch, lit cyan up to the handle and rgba(165,149,216,.25) beyond it.
  - The handle is a 12 × 36 px plate of #FFD6F6 dots on a 3 px grid, inside a 3 px hot ring.
  - Stop names sit under the track in chrome; the current one is in ink.
  - Two-stop choices (Smooth / Stepped) use it too.
- **On/off: checkbox.**
  - A 33 px notched box. Off is the line colour, empty.
  - On is a hot ring filled with the selected-chip dot plate, with a white pixel tick.
  - Its text sits beside it in body text. Focus glows around the box and the text.
- **Progress: dot row.**
  - The same 6 px dots on a 9 px pitch as the slider track: done in cyan, the newest dot white, the rest unlit.
  - The percentage sits to the right in chrome.
  - The small version, for terrain cards, uses 3 px dots on a 6 px pitch.
- **Disabled** is 40 %, as elsewhere.

## Focus: glow

A 3 px notched cyan line sits 6 px outside the focused thing, with an 18 px cyan glow, rgba(66,197,248,.55).

## Layout notes

- **The pause menu and the tabbed page.**
  - The pause menu stays as its own hub. Wrysk, 2026-10-09: "i think hybrid is probably better. the pause menu should be different. it has shortcuts and 'quit'. probably add other stuff there eventually too.."
  - The pause menu holds Resume, Save, Load, New world and Quit, plus shortcuts to the tabs (Field guide, Settings) and Dev metrics.
  - The tabbed page holds Field guide, Populations and Settings. G or Y opens it on Field guide.
  - The tabbed page never pauses the world; only the pause menu does. Wrysk: "field guide doesnt pause the world".
    - From a pause-menu shortcut, the world runs while the page is open, unless the player paused it with Select or P.
    - Back in the pause menu, it's paused again.

- At 23 px the Lanternjaw entry is taller than the panel. The entry scrolls and fades out at the bottom, and the buttons stay fixed below it.
- Prey chips wrap to four rows. The real screen needs a decision on long entries.
- The species list scrolls under a fixed search box, with the same fade, and focus keeps the selected row in view. Wrysk: "i think the compendium area definitely needs scrolling cuz it overflowed easily".

## Godot notes

These come from `round2/godot/specimen.gd`.

- **Fonts:**
  - One `FontVariation` per role with `variation_opentype`, keyed by `TextServer.name_to_tag`. Fractional axis values work.
  - Gridlite: antialiasing gray, hinting light (none renders dimmer and blurrier than the mockups), subpixel positioning off. Use whole multiples of 13 px.
  - Gridlite's unlit dots are drawn by the game, not the font (chrome-dots, 2026-10-11): its dot centres fall on pixel corners, where hinting turned them into stripes. Each Gridlite font keeps `wght` and `ELSH`, sets `BACK` and `RECT` to 1, and carries the role's `BACK` and `RECT` above as metadata. `NeonDotField` draws both zones cell for cell where the font would, one whole-pixel dot per cell with the light of the font's own dot (`game/ui/neon/README.md`).
  - Pixelify: default antialiasing and light hinting.
  - Letter spacing: `FontVariation.spacing_glyph`.
  - Godot draws Pixelify almost exactly like Chrome.
- **Drawing:**
  - Notched rings are four rectangles, or a nine-patch with cut corners.
  - The dithered fill is a generated texture per panel size, or a Bayer shader.
  - The scrim is a three-line canvas shader.
- **Godot project:** no stretch mode.

## Not designed yet

- The populations / viewer metrics screen.
- Long field-guide entries.
- Palette review.
- Sound.
