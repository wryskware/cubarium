# Round 2 pixel-font candidates (labels and menu tabs)

Gathered 2026-10-08. Each folder holds the font file(s) used, `LICENSE.txt` and `SOURCE.txt`. `fonts.json` carries the same data plus raw measurements (`measure`). The font folders are not in git (Wrysk 2026-10-09: no fonts in git); `fonts.json` gives each one's `download_url` to fetch it again into its folder. Only this README, `fonts.json` and `fonts.js` are committed.

## How the pixel grid was measured

fontTools, `python3 -I`, over the outlines of A-Z a-z 0-9 (composites decomposed, default instance for variable fonts). The pixel unit is the largest grid step (a divisor of the em, or a plain integer unit) that at least 92 % of outline coordinates sit on (3 % tolerance); the strict GCD is recorded too (`measure.strict_gcd`) and differs only where fonts have tiny corner cuts (Silkscreen, GCD 5 against a 125-unit pixel) or traced outlines (Cozette, Spleen 6x12). `native_px` = units per em / pixel unit, i.e. the CSS/Godot font size at which one font pixel is one screen pixel. Cap and x heights are measured from the H and x outlines in font pixels (OS/2 values are wrong for Bytesized and ignored). `native_px` is null when the outlines are not on a grid.

## Usable fonts (43)

| Family | Licence | Credit | native px | cap | x | lowercase | Coverage gaps (· − × °) | Folder |
|---|---|---|---|---|---|---|---|---|
| Silkscreen | OFL-1.1 | no | 8 | 5 | 5 | no | missing − | `silkscreen/` |
| Tiny5 | OFL-1.1 | no | 8 | 5 | 4 | yes | none | `tiny5/` |
| Pixelify Sans | OFL-1.1 | no | null | - | - | yes | none | `pixelify-sans/` |
| Jersey 10 | OFL-1.1 | no | 18.67 | 10 | 8 | yes | none | `jersey-10/` |
| Jersey 15 | OFL-1.1 | no | 27 | 15 | 11 | yes | none | `jersey-15/` |
| Micro 5 | OFL-1.1 | no | 11 | 5 | 4 | yes | none | `micro-5/` |
| VT323 | OFL-1.1 | no | null | - | - | yes | none | `vt323/` |
| Press Start 2P | OFL-1.1 | no | 8 | 8 | 6 | yes | missing − | `press-start-2p/` |
| DotGothic16 | OFL-1.1 | no | null | - | - | yes | none | `dotgothic16/` |
| Handjet | OFL-1.1 | no | null | - | - | yes | none | `handjet/` |
| Workbench | OFL-1.1 | no | null | - | - | yes | none | `workbench/` |
| Bytesized | OFL-1.1 | no | 8 | 3 | 2 | yes | none | `bytesized/` |
| Sixtyfour | OFL-1.1 | no | null | - | - | yes | none | `sixtyfour/` |
| Doto | OFL-1.1 | no | null | - | - | yes | none | `doto/` |
| monogram | CC0-1.0 | no | 16 | 7 | 5 | yes | none | `monogram/` |
| m5x7 | CC0-1.0 | no | 16 | 7 | 5 | yes | missing − | `m5x7/` |
| m6x11 | Free to use with attribution | yes | 16 | 11 | 8 | yes | missing · − | `m6x11/` |
| m6x11plus | Free to use with attribution | yes | 18 | 11 | 8 | yes | missing · − | `m6x11plus/` |
| Pixel Operator | CC0-1.0 | no | 16 | 9 | 7 | yes | missing − | `pixel-operator/` |
| Pixel Operator Mono | CC0-1.0 | no | 16 | 9 | 7 | yes | missing − | `pixel-operator-mono/` |
| Pixel Operator SC | CC0-1.0 | no | 16 | 9 | 7 | no | missing − | `pixel-operator-sc/` |
| Pixel Operator 8 | CC0-1.0 | no | 8 | 7 | 5 | yes | missing − | `pixel-operator-8/` |
| Departure Mono | OFL-1.1 | no | 11 | 8 | 6 | yes | none | `departure-mono/` |
| Pixeloid Sans | OFL-1.1 | no | 9 | 7 | 5 | yes | none | `pixeloid-sans/` |
| Pixeloid Mono | OFL-1.1 | no | 9 | 7 | 5 | yes | none | `pixeloid-mono/` |
| Kenney Mini | CC0-1.0 | no | 8 | 5 | 4 | yes | missing − | `kenney-mini/` |
| Kenney Pixel | CC0-1.0 | no | 16 | 7 | 6 | yes | missing − | `kenney-pixel/` |
| Kenney Mini Square | CC0-1.0 | no | 8 | 5 | 5 | no | missing − | `kenney-mini-square/` |
| Pixellari | Free for commercial use, credit requested | yes | 16 | 11 | 8 | yes | missing − | `pixellari/` |
| Cozette | MIT | yes | 13 | 8 | 6 | yes | none | `cozette/` |
| Spleen 6x12 | BSD-2-Clause | yes | 12 | 8 | 6 | yes | missing − | `spleen-6x12/` |
| Spleen 8x16 | BSD-2-Clause | yes | 16 | 10 | 7 | yes | missing − | `spleen-8x16/` |
| Terminus (TTF) | OFL-1.1 | no | null | - | - | yes | none | `terminus-ttf/` |
| Ark Pixel 10px | OFL-1.1 | no | 10 | 7 | 5 | yes | missing − | `ark-pixel-10/` |
| Fusion Pixel 10px | OFL-1.1 | no | 10 | 7 | 5 | yes | missing − | `fusion-pixel-10/` |
| Ark Pixel 12px | OFL-1.1 | no | 12 | 9 | 6 | yes | missing − | `ark-pixel-12/` |
| Fusion Pixel 12px | OFL-1.1 | no | 12 | 9 | 6 | yes | missing − | `fusion-pixel-12/` |
| Galmuri9 | OFL-1.1 | no | 10 | 9 | 6 | yes | none | `galmuri9/` |
| Galmuri11 | OFL-1.1 | no | 12 | 11 | 8 | yes | none | `galmuri11/` |
| Public Pixel | CC0-1.0 | no | 8 | 7 | 5 | yes | none | `public-pixel/` |
| Dogica | OFL-1.1 | no | 8 | 7 | 6 | yes | missing − | `dogica/` |
| Dogica Pixel | OFL-1.1 | no | 8 | 7 | 6 | yes | missing − | `dogica-pixel/` |
| Cubic 11 | OFL-1.1 per README | no | 12 | 8 | 5 | yes | none | `cubic-11/` |

## Notes per font

- **Silkscreen** (Jason Kottke; https://fonts.google.com/specimen/Silkscreen): Caps-only (lowercase maps to capitals); wide, low-contrast 5 px caps with bevelled corners; a Regular and a Bold file. (caps only or small caps) Credit line: Copyright 2001 The Silkscreen Project Authors (https://github.com/googlefonts/silkscreen).
- **Tiny5** (Stefan Schmidt; https://fonts.google.com/specimen/Tiny5): Small proportional 8 px sans with real lowercase (x-height 4 px); round-cornered, friendly, tight spacing. Credit line: Copyright 2022-2024 The Tiny5 Project Authors (https://github.com/Gissio/font_tiny5).
- **Pixelify Sans** (Stefie Justprince; https://fonts.google.com/specimen/Pixelify+Sans): Variable weight (wght); soft, rounded, game-menu feel; outlines are not on a clean pixel grid. Grid: not on a clean pixel grid (strict GCD of outline coordinates = 1 font units of 1000/em). Credit line: Copyright 2021 The Pixelify Sans Project Authors (https://github.com/eifetx/Pixelify-Sans).
- **Jersey 10** (Sarah Cadigan-Fried; https://fonts.google.com/specimen/Jersey+10): Condensed sports-jersey face; cap height is exactly 10 px (em is 18.67 px, so not an integer size). Credit line: Copyright 2023 The Soft Type Project Authors (https://github.com/scfried/soft-type-jersey).
- **Jersey 15** (Sarah Cadigan-Fried; https://fonts.google.com/specimen/Jersey+15): Same design as Jersey 10 at cap height 15 px; condensed, tall, em is 27 px. Credit line: Copyright 2023 The Soft Type Project Authors (https://github.com/scfried/soft-type-jersey).
- **Micro 5** (Sarah Cadigan-Fried; https://fonts.google.com/specimen/Micro+5): Tiny pixel sans, cap height 5 px, x-height 4 px, em 11 px; narrow and light, a very small-label face. Credit line: Copyright 2023 The Soft Type Project Authors (https://github.com/scfried/soft-type-micro).
- **VT323** (Peter Hull; https://fonts.google.com/specimen/VT323): Monospace DEC VT320 terminal look; tall, thin strokes; outlines drift off the pixel grid. Grid: not on a clean pixel grid (strict GCD of outline coordinates = 1 font units of 1000/em). Credit line: Copyright 2011, The VT323 Project Authors (peter.hull@oikoi.com).
- **Press Start 2P** (CodeMan38; https://fonts.google.com/specimen/Press+Start+2P): Classic arcade 8x8 monospace; very wide, heavy, cap height 8 px; Reserved Font Name "Press Start 2P" (rename if modified). Credit line: Copyright 2012 The Press Start 2P Project Authors (cody@zone38.net), with Reserved Font Name \.
- **DotGothic16** (Fontworks Inc.; https://fonts.google.com/specimen/DotGothic16): Japanese dot-matrix gothic with Latin; 16 px dot design; 2 MB file because of the CJK set; outlines not on an integer-unit grid. Grid: not on a clean pixel grid (strict GCD of outline coordinates = 1 font units of 1000/em). Credit line: Copyright 2020 The DotGothic16 Project Authors (https://github.com/fontworks-fonts/DotGothic16/).
- **Handjet** (Rosetta, David Březina; https://fonts.google.com/specimen/Handjet): Variable (element grid/shape + weight) dot-matrix printer look; default instance measured; not a clean grid. Grid: not on a clean pixel grid (strict GCD of outline coordinates = 5 font units of 8160/em). Credit line: Copyright 2018 The Handjet Project Authors (https://github.com/rosettatype/Handjet/).
- **Workbench** (Jens Kutílek; https://fonts.google.com/specimen/Workbench): Variable (bleed/scanline) CRT look based on home-computer fonts; display face, not clean grid. Grid: not on a clean pixel grid (strict GCD of outline coordinates = 1 font units of 1024/em). Credit line: Copyright 2021 The Workbench Project Authors (https://github.com/jenskutilek/homecomputer-fonts).
- **Bytesized** (Baltdev; https://fonts.google.com/specimen/Bytesized): Extremely small grid (cap 3 px, x-height 2 px) built to be set at tiny sizes; very compact, caps and lowercase barely differ. Credit line: Copyright 2024 The Bytesized Project Authors (https://github.com/balt-dev/bytesized-gf).
- **Sixtyfour** (Jens Kutílek; https://fonts.google.com/specimen/Sixtyfour): Variable (bleed/scanline) C64-style monospace display face; not clean grid. Grid: not on a clean pixel grid (strict GCD of outline coordinates = 1 font units of 1024/em). Credit line: Copyright 2021 The Sixtyfour Project Authors (https://github.com/jenskutilek/homecomputer-fonts).
- **Doto** (Óliver Lalan; https://fonts.google.com/specimen/Doto): Variable round-dot matrix (ROND, wght); each pixel is a dot, so outlines are circles, not squares. Grid: not on a clean pixel grid (strict GCD of outline coordinates = 1 font units of 1000/em). Credit line: Copyright 2024-2024 The Doto Project Authors (https://github.com/oliverlalan/Doto).
- **monogram** (datagoblin; https://datagoblin.itch.io/monogram): Monospace 5-pixel-wide face, cap 7 / x 5 px, em 16 px; clean, very legible, plain with slight quirks; the extended file has Latin accents and symbols (basic monogram.ttf lacks · − × ° and is not shipped). Credit line: datagoblin (appreciated, not required).
- **m5x7** (Daniel Linssen (managore); https://managore.itch.io/m5x7): Compact proportional 5x7 face, cap 7 / x 5 px, em 16 px; tight but very readable, the classic small game font; lacks − and ° (checked). Credit line: Daniel Linssen (managore.itch.io) - appreciated.
- **m6x11** (Daniel Linssen (managore); https://managore.itch.io/m6x11): Proportional 6x11 face, cap 11 / x 8 px, em 16 px (use sizes 16, 32, 48); taller and airier than m5x7, still tight spacing. Credit line: Font: m6x11 by Daniel Linssen (managore.itch.io/m6x11).
- **m6x11plus** (Daniel Linssen (managore); https://managore.itch.io/m6x11): m6x11 with a much larger character set; em 18 px (use sizes 18, 36, 54); same shapes, cap 11 / x 8 px. Credit line: Font: m6x11 by Daniel Linssen (managore.itch.io/m6x11).
- **Pixel Operator** (Jayvee Enaguas (HarvettFox96); https://www.dafont.com/pixel-operator.font): Proportional 16 px sans (cap 9 / x 7 px), neutral and open, Regular + Bold; a general UI body face. Credit line: Jayvee Enaguas (HarvettFox96) - not required.
- **Pixel Operator Mono** (Jayvee Enaguas (HarvettFox96); https://www.dafont.com/pixel-operator.font): Monospace sibling of Pixel Operator, cap 9 / x 7 px, em 16 px; Regular + Bold. Credit line: Jayvee Enaguas (HarvettFox96) - not required.
- **Pixel Operator SC** (Jayvee Enaguas (HarvettFox96); https://www.dafont.com/pixel-operator.font): Small-caps variant: lowercase is a 7 px small capital, so no true lowercase; good for tabs and labels. (caps only or small caps) Credit line: Jayvee Enaguas (HarvettFox96) - not required.
- **Pixel Operator 8** (Jayvee Enaguas (HarvettFox96); https://www.dafont.com/pixel-operator.font): The 8 px cut (em 8 px, cap 7 / x 5 px); denser and plainer than the 16 px cut. Credit line: Jayvee Enaguas (HarvettFox96) - not required.
- **Departure Mono** (Helena Zhang; https://departuremono.com): Technical monospace, em 11 px (cap 8 / x 6 px); wide, clean, slightly industrial. OTF (CFF) outlines. Credit line: Copyright 2022-2024 Helena Zhang (helenazhang.com).
- **Pixeloid Sans** (GGBotNet; https://ggbot.itch.io/pixeloid-font): Proportional 9 px sans (cap 7 / x 5 px, em 9 px; sizes 9, 18, 36...); round, open, big x-height, very legible; Regular + Bold; Reserved Font Name "Pixeloid". Credit line: Copyright 2020-2025 GGBotNet (https://ggbot.net/fonts/).
- **Pixeloid Mono** (GGBotNet; https://ggbot.itch.io/pixeloid-font): Monospace sibling of Pixeloid Sans, em 9 px, cap 7 / x 5 px. Credit line: Copyright 2020-2025 GGBotNet (https://ggbot.net/fonts/).
- **Kenney Mini** (Kenney (Kenney Vleugels); https://kenney.nl/assets/kenney-fonts): Tiny 8 px-em pixel sans (cap 5 / x 4 px), narrow and crisp; chunky blocky look. Credit line: Kenney / www.kenney.nl - not mandatory.
- **Kenney Pixel** (Kenney (Kenney Vleugels); https://kenney.nl/assets/kenney-fonts): Taller 16 px-em pixel sans (cap 7 / x 6 px), proportional, mid-weight; ascender 8 px. Credit line: Kenney / www.kenney.nl - not mandatory.
- **Kenney Mini Square** (Kenney (Kenney Vleugels); https://kenney.nl/assets/kenney-fonts): Square-proportioned caps-only cut of Kenney Mini (cap 5 px); lowercase maps to capitals; wide and blocky. (caps only or small caps) Credit line: Kenney / www.kenney.nl - not mandatory.
- **Pixellari** (Zacchary Dempsey-Plante; https://www.dafont.com/pixellari.font): Proportional 16 px sans, cap 11 / x 8 px; tall, open, wide range of letter widths. Credit line: Pixellari font by Zacchary Dempsey-Plante.
- **Cozette** (Ines (the-moonwitch / slavfox); https://github.com/the-moonwitch/Cozette): Vector-traced monospace of a 6x13 bitmap font; em 13 px (cap 8 / x 6 px); terminal feel, very wide coverage; outlines are traced so unit is 157.54 font units. Credit line: Copyright (c) 2020 Ines <ines@moonwit.ch>, MIT licence - keep the notice with the font.
- **Spleen 6x12** (Frederic Cambus; https://github.com/fcambus/spleen): Monospace 6x12 cell (em 12 px, cap 8 / x 6 px); sharp terminal face; no − or arrows in this size. Credit line: Copyright (c) 2018-2026, Frederic Cambus - BSD notice must accompany the font (binary form: reproduce it in docs/credits).
- **Spleen 8x16** (Frederic Cambus; https://github.com/fcambus/spleen): Monospace 8x16 cell (em 16 px, cap 10 / x 7 px); sharp, plain, a little wider; fuller glyph set. Credit line: Copyright (c) 2018-2026, Frederic Cambus - BSD notice must accompany the font (binary form: reproduce it in docs/credits).
- **Terminus (TTF)** (Dimitar Toshkov Zhekov (Terminus); Tilman Blumenbach (TTF build); https://files.ax86.net/terminus-ttf/): Monospace console face; the TTF is auto-traced outlines plus embedded bitmap strikes at 12-32 ppem (EBDT), so outline grid is not clean; use at its strike sizes. Reserved Font Names. Grid: not on a clean pixel grid (strict GCD of outline coordinates = 1 font units of 1000/em). Credit line: Copyright (c) 2010 Dimitar Toshkov Zhekov, (c) 2011-2023 Tilman Blumenbach.
- **Ark Pixel 10px** (TakWolf; https://github.com/TakWolf/ark-pixel-font): Proportional 10 px face, the "latin" variant of a CJK font (file is 0.5 MB for that reason); cap 7 / x 5 px; compact, crisp, quirky serif-free shapes. Credit line: Copyright (c) 2021, TakWolf (https://takwolf.com).
- **Fusion Pixel 10px** (TakWolf; https://github.com/TakWolf/fusion-pixel-font): Proportional 10 px merge of several pixel fonts (latin variant, 4 MB with CJK); cap 7 / x 5 px, Latin shapes inherited from Ark Pixel. Credit line: Copyright (c) 2022, TakWolf (https://takwolf.com); bundles Ark Pixel, Galmuri etc. glyphs under OFL.
- **Ark Pixel 12px** (TakWolf; https://github.com/TakWolf/ark-pixel-font): Proportional 12 px face, the "latin" variant of a CJK font (file is 5 MB for that reason); cap 9 / x 6 px; roomier, readable at small label sizes. Credit line: Copyright (c) 2021, TakWolf (https://takwolf.com).
- **Fusion Pixel 12px** (TakWolf; https://github.com/TakWolf/fusion-pixel-font): Proportional 12 px merge of several pixel fonts (latin variant, 7 MB with CJK); cap 9 / x 6 px, Latin shapes inherited from Ark Pixel. Credit line: Copyright (c) 2022, TakWolf (https://takwolf.com); bundles Ark Pixel, Galmuri etc. glyphs under OFL.
- **Galmuri9** (Lee Minseo (quiple); https://github.com/quiple/galmuri): Korean pixel face, proportional Latin, em 10 px (cap 9 / x 6 px: tall caps, small lowercase); bright, bold, game-UI feel. Credit line: Copyright (c) 2019-2025 Lee Minseo (quiple@quiple.dev).
- **Galmuri11** (Lee Minseo (quiple); https://github.com/quiple/galmuri): Larger Galmuri, em 12 px (cap 11 / x 8 px); proportional, plump, very legible. Credit line: Copyright (c) 2019-2025 Lee Minseo (quiple@quiple.dev).
- **Public Pixel** (GGBotNet; https://ggbot.itch.io/public-pixel-font): Extra. Monospaced 8x8-grid arcade face (cap 7 / x 5 px, em 8 px); clean, wide, 1324 glyphs; a legible alternative to Press Start 2P. Credit line: GGBotNet - not required.
- **Dogica** (Roberto Mocci; https://www.dafont.com/dogica.font): Extra. Monospace 8x8 face made for GB Studio (cap 7 / x 6 px, em 8 px); tall x-height, friendly, very legible. Credit line: Copyright (c) 2020 Roberto Mocci (patreon.com/rmocci); credit not required for raster works, required with distribution/modification; Reserved Font Name.
- **Dogica Pixel** (Roberto Mocci; https://www.dafont.com/dogica.font): Extra. Kerned proportional version of Dogica (cap 7 / x 6 px, em 8 px); the better one for labels. Credit line: Copyright (c) 2020 Roberto Mocci (patreon.com/rmocci); credit not required for raster works, required with distribution/modification; Reserved Font Name.
- **Cubic 11** (ACh-K (based on M+ BITMAP FONTS); https://github.com/ACh-K/Cubic-11): Extra. Traditional-Chinese pixel font with proportional-looking Latin, em 12 px (cap 8 / x 5 px); 2.7 MB; rounded, neat; commercial use is explicitly allowed. Credit line: Cubic 11 by ACh-K; based on M+ BITMAP FONTS (c) 2002-2004 COZ, (c) 2005 M+ FONTS PROJECT.

## Excluded or not shipped

| Font | Reason |
|---|---|
| Px437 IBM VGA 8x16 (int10h Ultimate Oldschool PC Font Pack) | Licence is CC BY-SA 4.0 (read at https://int10h.org/oldschool-pc-fonts/readme/): credit "VileR" plus a link is required, and ShareAlike applies to adaptations. It is not on the allowed list and whether embedding counts as an adaptation is unclear, so it is excluded and was not downloaded. Wrysk can re-admit it by accepting BY-SA. |
| Spleen 5x8 | Licence fine (BSD-2) but the release ships it only as bitmap (.otb/.bdf/.pcf), no TTF/OTF; 6x12 and 8x16 are included. Larger Spleen sizes (12x24+) were not wanted. |
| Kenney Pixel Square, Mini Square Mono, other Kenney fonts | Not requested (CC0, in the same zip if wanted). |

No candidate was excluded for a non-commercial-only, no-embedding or unreadable licence.

## Surprising or conditional

- **Informal licences, included:** m6x11 / m6x11plus ("free to use with attribution", itch page only, no licence file or licence field) and Pixellari (author note: free incl. commercial, "all I ask is that I get credit"). Credit them in the game; ask the author if a formal grant is wanted.
- **Pixel Operator source:** the official NotABug repo and pixeloperator.com are gone. Files come from the author's dafont upload, byte-identical (sha256) to the GitHub fork bauripalash/ttf-pixeloperator-fork; both state CC0 1.0 (changed to CC0 in release 2018.10.04-1).
- **Cubic 11:** README says OFL 1.1 (based on M+ BITMAP FONTS) but the shipped `OFL.txt` is the short M+ "free software, unlimited permission, derivatives under the same terms" text. Commercial use is allowed either way.
- **Reserved Font Names:** Press Start 2P, Pixeloid, Terminus, Dogica / Dogica Pixel. Subsetting or editing the file for the game counts as modification and needs a rename; ship the originals untouched.
- **OFL/MIT/BSD fonts** need the licence/notice text to travel with the font file in the game's third-party notices (MIT: Cozette; BSD-2: Spleen). On-screen credit is not required for OFL or CC0.
- **Off the pixel grid (native_px null):** Pixelify Sans, VT323, DotGothic16, Handjet, Workbench, Sixtyfour, Doto (round dots), Terminus TTF (auto-traced outlines; embedded bitmap strikes at 12, 14, 16, 18, 20, 22, 24, 28, 32 ppem exist but the outline path will blur at other sizes).
- **Non-integer or odd em:** Jersey 10 has a 10 px cap in an 18.67 px em (use that exact size, not 18 or 19); Jersey 15 em is 27 px; Micro 5 em 11 px; Cozette 13 px; Departure Mono 11 px; m6x11plus 18 px; Pixeloid 9 px (sizes 9, 18, 36...).
- **Tiny cap heights:** Bytesized cap 3 / x 2 px, Micro 5 cap 5 / x 4, Kenney Mini cap 5 / x 4, Tiny5 cap 5 / x 4. Press Start 2P (cap 8, 8 px wide) is the widest.
- **Caps only (lowercase glyphs are the capitals):** Silkscreen, Kenney Mini Square. Pixel Operator SC has small caps (7 px).
- **Missing U+2212 minus** in most fonts (use a hyphen or the real minus only for fonts marked as complete): Silkscreen, Press Start 2P, m5x7, m6x11 (also no ·), m6x11plus (also no ·), all Pixel Operator cuts, Kenney, Pixellari, Spleen, Ark/Fusion Pixel, Dogica. Full set of · − × ° exists in Tiny5, Pixelify Sans, Jersey, Micro 5, VT323, DotGothic16, Handjet, Workbench, Bytesized, Sixtyfour, Doto, monogram (extended file), Departure Mono, Pixeloid, Cozette, Terminus, Galmuri, Public Pixel, Cubic 11.
- **Big files:** Fusion Pixel 12px 7 MB, Ark Pixel 12px 5 MB, Fusion 10px 4 MB, Cubic 11 2.7 MB, DotGothic16 2 MB (CJK sets in the "latin" variants); whole folder is about 35 MB.
- **OS/2 metrics unreliable:** Bytesized (OS/2 capHeight 11 on a 16-unit em) and some others; use the measured values here.
