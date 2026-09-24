# Brief: latticevine concept art + botanical reference plate (2026-09-23)

Worker: sprite-forge. Subject: the new plant **D15 latticevine**, dossier
`design/art-direction/species-dossier-D15-latticevine-2026-09-23.md` — read it fully; it
is the prompt source (body plan, hexes, phase table, generation notes). Wrysk asked for
concept art and a botany-style reference sheet for his sign-off; a 3D model follows
only after he signs off, so nothing here builds models.

Model: Qwen-Image 2.1 only (local). Text-to-image: `art/gen/workflows/qwen21-official-t2i.json`
or the illustration graph at the canvas each item names. Reference sheet:
`art/gen/workflows/qwen21-ref-sheet.json` with `comfy_run.py --ref` (built in the
character-sheet smoke run). Style: kit v0.4 block (`art/gen/PROMPT-KIT.md`), last
sentence replaced by the per-image framing (authorised for this round). Never pixel art.

## Part 1 — concepts (text to image, seeds 2301 and 2302 each = 10 images)

| code | what | canvas |
| --- | --- | --- |
| LV-wide | in-world: a terrace rock face facing the camera, two or three latticevine stands at different sizes rooted along its foot and on a ledge, phases scattered (a few spurs in flower, a few in magenta bead fruit, most bare leaf), one bellwing hovering under a flowering spur with its siphon up into a trumpet; turf at the foot, a cropped bare band at the bottom (the grazing line) | 16:9 |
| LV-tile | the primary body plan alone as a rock-face tile: one full stand, root pocket at the bottom edge, diamond lattice with open rock in every diamond, holdfast pads at nodes, shingle rows, two low bowed runners with dark bowers behind them, 6–9 flowering and 6–9 fruiting spurs scattered | 2:3 tall |
| LV-roost | close in-world: the lower lattice of a full stand, one bellwing folded flat inside a small bower (magenta rim in the dark), a second bellwing sipping a flowering spur beside it, bead clusters nearby | 16:9 |
| LV-B | alternate (B) curtain vine as a rock-face tile: rooted on a ledge at the top, hanging strands down the face, flowers and beads at the strand tips | 2:3 tall |
| LV-C | alternate (C) knot-net as a rock-face tile: horizontal rows, ball knots at the crossings, some knots flower heads and some magenta fruit heads | 2:3 tall |

The seedporter's body plan is **not decided** (SP-A…D) — leave it out of every image.
Bellwing = BW-A as in the dossier D14 (two vane pairs, front pair swept forward, rear
pair swept back, vanes held out level — "not wings rising from the top"; the smoke run
showed that failure).

One fix round is allowed per code for a named body-plan failure (one change, seeds
held, as in the concept rounds) — e.g. the whole wall in bloom, closed diamonds with no
rock showing, beads drawn as earth-coloured berries.

## Part 2 — botanical reference plate (hero-anchored, seeds 2301 and 2302 = 2 images)

Pick the LV-tile (or its fix) that best matches the dossier as the **provisional hero**
and log why; Wrysk may swap it. Reference = hero. One wide plate (about 2048×1152), plain
flat dark neutral background, even lighting, **no text, no labels, no numbers**, laid
out like a botanical study sheet:

- left, large: the whole full-size stand face-on, with its rock;
- top middle: a side profile section of the same stand against the rock edge, showing
  runners flat to the stone above and bowing out into bowers below;
- right column: close details — one holdfast pad; one shingle row on a runner; one spur
  shown five times in a row in its phases bare, bud, flower, fruit, spent;
- bottom strip: the four growth sizes on one shared ground line, newborn (one runner),
  young, grown, full, each at its relative scale.

Crop the plate's parts with `art/gen/tools/crop_panels.py` into `crops/` (gap mode; note
if it can't separate the parts).

## Output

- `art/gen/runs/2026-09-23-latticevine/`: images + `.run.json` (existing schema), prompts
  in a file, `LOG.md`, `contact-sheet.png` (all concepts, the hero and both plates).
- Do not commit. Verdicts stay `unjudged`; you never decide look.
- Return ≤ 30 lines: per code what reads and the largest failure against the dossier;
  fixes made and whether they worked; which hero and why; how the plate's layout came
  out; anything in the dossier that the model can't draw and should be reworded.
