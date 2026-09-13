---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Spiretree wind room: a centred dome and never-drawn trunk rows

Evidence for root's reconciliation, not a decision. The roadmap's open item 1 ("wind
readability at 1×: the shipped art leaves only 0.30 px at the spiretree cap") for the one
family it names first. Owned and changed: `art/parts/plant_spiretree_crown.svg` and
`plant_spiretree_trunk.svg` (through the recorded transform `art/plants/author_spire_wind.py`),
the regenerated `assets/atelier/tall.png` (spiretree trunk and crown rows only; `pack.json`
is byte-identical), the trunk-strip ownership rule in `art_present.rs` (`trunk_strip`,
`TALL_STRIP_FLOOR`, `TALL_STRIP_TOP`, `draw_column`), the loader's periodicity unit test in
`art.rs`, the replicas and three focused tests in `tests/art_wind.rs` and
`tests/art_plants.rs`, a before/after capture in `tests/art_wind_capture.rs`, and the
budget sentence of `art/README.md` "Wind" (root's authored-growth text preserved). No core,
care, hunter, snapshot, runner or README-root edit; nothing live; no species, no global
wind change; the shared 30 s / 12 s quiet breeze is untouched.

## Why the spiretree barely moved

A column's admitted tip is `min(desired, budget / 1.1)` with the budget the smallest
[`Sprite::bend_headroom`] of any painted texel in any of its parts at their highest
placement: a texel at offset `(x, y)` from the tile pivot may carry a displacement `a·s`
only while `hypot(|x| + a·s + 1, |y| + 1) ≤ 9`. Two texels bound the spiretree:

| texel | why | headroom |
| --- | --- | --- |
| crown (13, 4) and (13, 5) — the dome's far edge | the dome was drawn one pixel **right** of the trunk axis (dome centre 7.5, trunk centre 6.5 in the 15-wide sprite), so its far edge sat 6.5 px from the pivot in a row where the circle allows 7.79 − 1 | **0.30 px** (the recorded "cap f0") |
| trunk row 0 (and row 15), cols 6 and 9 | 7.5 px above the pivot the footprint circle is only ±1.96 px wide; a 4-wide trunk's outline columns are at ±1.5 | **0.47 px** (glasscane's and the vine's recorded "trunk f0") |

Recentring the dome alone would have moved the family from 0.30 to 0.47; the trunk's end
rows had to go too. They can, because of how columns are drawn: every row is composited
exactly once and each trunk tile draws only a **strip** of its rows. The original strips own
tile rows 0–3 (`TALL_JOIN..TILE_ROWS`), so row 0 is the top of every strip; row 15 was
never drawn by any strip at all.

## What changed

**Art** (`author_spire_wind.py`, refused once applied):

- Crown rows 0–7 redrawn centred on the pivot, widths 2, 6, 8, 10, 12, 12, 10, 8 (a
  two-pixel tip, the same 12 px dome, the same four colours, the mint/cyan vein kept on the
  trunk's cyan column); rows 8–14 keep the trunk pattern; row 15 left unpainted so the cap
  the loader derives is the dome alone. Cap headroom 0.30 → **1.31 px** (row 4's edge at
  offset 5.5).
- Trunk rows 0 and 15 left unpainted; rows 1–14 are the unchanged 4-periodic pattern. Trunk
  headroom 0.47 → **2.52 px** (row 1 at `y + 1 = 7.5`).
- Base untouched; its headroom is 74 px (its wide rows sit where the bend is nearly zero).

**Renderer** (`art_present.rs`): a family whose trunk frames all leave tile row 0 unpainted
**opts in** (`trunk_strip`) to strips one row lower — tile rows 1–4 (`TALL_STRIP_FLOOR =
TALL_JOIN − 1`, `TALL_STRIP_TOP = TILE_ROWS − 1`), the last possible segment still cut at
the tile top under its cap, the first segment keeping `TALL_FIRST_JOIN` as its floor.
Because the pattern is 4-periodic the strips still partition the column exactly, meeting
at the same heights; the tile's row 0 is then drawn only by the ninth segment (covered by
the dome) and row 15 by none. Glasscane and the vine paint their row 0 and keep the
original strips, so their images are untouched — including the newest row's fade-in while
a segment grows, which the shifted strips would shape slightly differently (the strip's own
`unit(reveal − floor)` ramp starts a row lower); making the rule an art-derived opt-in
rather than a global shift is what keeps them bit-identical. `tall_grown_px` (growth pacing
in pixels per height) is unchanged; the column's drawn top `4h + 8` is unchanged.

**Result** (`ArtPresenter::bend_budgets`): spiretree **0.299 → 1.314 px**; a bare
spiretree column is admitted its whole authored 0.9 px tip (`effective_tip(0.9, 1.314) =
0.9`, was 0.272); a vined spiretree column is held to the vine's 0.465 (tip 0.423, was
0.272). Vinecoil, glasscane and every small plant: unchanged.

## Verification (foreground, exit 0)

Bake (Godot 4.7.2): baseline first — the unchanged tree reproduces the shipped atlases
byte for byte. With the change, two bakes into `/tmp/bake-spire-NIkJmP` and a second
fresh dir are identical to each other; against the shipped pack only two rows of
`tall.png` differ (spiretree trunk: 192 px = the two cleared rows × 24 frames; spiretree
crown: the dome), the spiretree base row and every glasscane and vinecoil row are
pixel-identical, and `creatures.png`, `plants.png`, `ground.png`, `habitat.png` and
`pack.json` are byte-identical.

Tests, run in an isolated `git archive HEAD` copy (`83e562e`) with the owned files
overlaid, because the shared checkout carries other workers' in-flight core edits:

| target | result |
| --- | --- |
| `cargo test -p cubarium --lib --tests` (whole host crate, `--no-fail-fast`) | 518 passed, 0 failed, 16 ignored |
| of which `art_wind` | 22 passed (3 new) |
| `art_plants` 25, `art_motion` 13, `art_wind_top` 5, `astra_wind_regressions` 6, lib 222 | all passed |

New or extended, all reusing the file's own fixtures:

- `the_spiretree_column_is_admitted_its_whole_desired_tip`: budget ≥ 0.9 · 1.1, effective
  tip exactly 0.9, cap and trunk headroom each above that, trunk rows 0 and 15 unpainted
  and rows 1–14 4-periodic, `column_budget` = own for bare columns and `min(own, vine)` for
  vined ones.
- `the_shifted_strips_are_opted_into_by_the_art_and_still_composite_each_row_once`:
  glasscane, the vine and the striped synthetic column read the original strips; the
  spiretree and a striped variant with rows 0/15 cleared read the shifted ones; that
  variant, hand-built on the shifted strips at eight heights and three amplitudes, carries
  exactly one stamp of green in every interior row and the stripe the global height asks
  for (the original strips' exactly-once test, applied to the new rule).
- `a_trunk_tiles_end_rows_are_never_drawn_below_the_top_segment`: through the presenter,
  every family with its trunk row 15 painted magenta draws every calm frame identically
  while all columns grow from bare ground to the rim; hand-built on the shifted strips, the
  spiretree with a magenta row 0 is the shipped column bit for bit at seven heights and
  three amplitudes, and differs at the ninth segment (the row exists under the cap).
- The loader's periodicity unit test now requires rows 1–14 periodic and rows 0/15 either
  empty or the pattern; the `crown`/`base` join checks are unchanged.
- Unchanged and still green: quiet-interval identity (`a_quiet_tick_bends_nothing_anywhere`),
  the exact per-root quiet interval, the budget sweep against an independent
  `bend_headroom` recomputation, every frame drawing completely at its own budget, no
  per-frame jump at 60 fps, 30/60/120 fps agreement, the vine sharing its host's amplitude,
  the striped exactly-once column, growth pacing and the cap/tail derivation.

## Capture

`tests/art_wind_capture.rs::spiretree_before_after_capture` (ignored; `CUBARIUM_OLD_PACK`
is a copy of the previous `assets/atelier`, taken with `git show HEAD:…`): a bare spiretree
column (Front cx 11) and a vined one (Front cx 10), each fed alone, grown to the rim, then
20 s at 60 fps from calm through a packet's rise and hold, from both packs, every second
frame, native 256×128 nets in `/tmp/cubarium-spire-…/{old,new}-{bare,vine}` (dir recorded
in `/tmp/fable-spire-cap`, `sheet.py` and `sheet.png` beside them). Sheet:
`captures/spiretree-wind.png` (gitignored), ×5, calm and 4–12 s. Measured on the frames
(horizontal centroid of the dome's light, Top rows above the rim plus the two Front rows
under it, calm frame vs the largest excursion):

| column | old | new |
| --- | --- | --- |
| bare | −0.21 px | **−0.53 px** |
| vined | −0.26 px | −0.35 px |

The old dome's per-pixel offset (0.27 px admitted) hardly moved a nearest-drawn crown; the
new one visibly leans with the gust and settles back, the trunk with it on one curve, the
vine on the vined column still riding its own smaller room. The new tip is pointed rather
than flat; the dome is the same width and colours.

## Limits

- The vined spiretree is now bound by the vine's own 0.47 px (its tile paints its row 0,
  and its topmost tile owns up to the tile top). Clearing the vine's end rows the same way
  is the obvious next family, together with glasscane's trunk (same 0.47) and its crown.
- The centroid numbers are a measurement of a sub-pixel-resampled 12 px dome, not a claim
  of room-distance legibility; judge on the cube.
- The trunk tile is no longer 4-periodic over all 16 rows; `art/PLANTS.md` "Tall plants"
  still says it is (that section is not in this package's ownership — one sentence to add:
  rows 0 and 15 may be unpainted, never drawn by a strip).
- `art_wind.rs`, `art_plants.rs` and `art_present.rs` were not rustfmt-clean before and were
  not reformatted.
