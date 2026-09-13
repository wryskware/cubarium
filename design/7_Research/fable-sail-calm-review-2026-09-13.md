---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Sail calm fins: independent review of `bc7b2a9` and bounded adoption of brace

Reviewed Astra's `astra-sail-calm-study-2026-09-13.md`, `art/studies/sail-calm/`
(README, `bake.gd`, renderer and meal tools) and the listed captures. Exactly the three
variants the study baked were considered: original, brace, settle. No sweep.

**Disposition: brace adopted, by its four rest/feed fin rotation tracks only, ported into
the current production sail scene and rebaked with the current exporter.** Settle stays
rejected as the study retained it. The concentrated rest gesture is judged acceptable; the
tradeoffs are stated below and the max-frame-change increase is not called an improvement.

## What was examined (and what was not)

- The study's source pose strips at 4× (`render-final/source-{rest,feed}-4x.png`) and my
  own 8× row sheets of the three variants' sail rest and feed atlas rows
  (`captures/sail-calm-fable-review-2026-09-13/atlas-{rest,feed}-8x.png`).
- The two real-meal contact sheets at 6× (`sail-calm-meal-review-seed{1,8}/meal-crops-6x.png`:
  original / brace / settle rows, −1 … 30 ticks from the first sail meal onset).
- The full `measurements.json` (48 records × 3 variants) and my own frame-to-frame
  measurement over the real meal frame streams (below).
- Not examined: the playback viewers were not opened, the isolated debugger was not used
  (its only page was `about:blank`; no root activity was disturbed), and no physical panel
  was viewed. Nothing here is a claim of watching video; it is stills, atlas rows and
  numbers.

## What brace is, in texels

Rest (4 s, 16 point-baked frames): the original oscillates 0.12 ↔ 0.16 rad every second,
which bakes as fin-tip texels stepping in frames 2–6 and 10–14. Brace holds the fins at 0
rad; the authored keys 0 → 0.2 → 0 at 2.5/3.0/3.5 s bake to **a single differing frame
(12, 21 texels)**: the 0.1 rad half-way poses land on the held texels. In the presenter
that is a cross-fade into and out of the peak pose over 0.25 s each way, once per 4 s.
Feed (2 s): the original flicks fins 0.2 ↔ 0.1 rad every 0.5 s (11–22 texels per frame);
brace holds them at 0.2 and only the body's 1 s chew moves (the same 23 texels in every
non-still frame, disjoint from the fin texels). Move and bud rows are byte-identical.

## Evidence

Study metrics (fixed-anchor, rooted adult / 0.6): rest alpha-area SD 1.20 → 0.81 /
0.53 → 0.14; feed 1.54 → 0.26 / 0.33 → 0.09; mean temporal second difference down about
half in rest and feed at both scales. Against that, **native rest's largest single frame
L1 change rises 0.084 → 0.250 (3×), juvenile 0.036 → 0.078**: the one gesture is bigger
than any old flutter step. Settle: rest SD 0, feed SD 1.89 (worse than original), a fin
collapse during the first chew.

My own real-meal measurement (crop of 18 px around the tracked first-onset sail, 2 s
before to 6 s after onset, 60 fps; the sail and its neighbours move, so this is the
crowded picture, not an isolated rig):

| seed, sail | variant | frame steps ≥ 64/255 in the crop, original → variant | mean step | worst step |
| --- | --- | --- | --- | --- |
| 1, adult 8:4 | brace | 86 → 75 | 0.0033 → 0.0032 | 120 → 123 |
| 1, adult 8:4 | settle | 86 → 84 | 0.0033 → 0.0033 | 120 → 134 |
| 8, juvenile 5:3 | brace | 67 → 54 | 0.0045 → 0.0046 | 106 → 106 |
| 8, juvenile 5:3 | settle | 67 → 68 | 0.0045 → 0.0047 | 106 → 106 |

In both real worlds brace lowers the count of large frame steps around the sail and does
not raise the worst step; settle does neither. Organism clip phase is hashed per organism,
so resting sails do not gesture in unison.

Judgement of the crops: at 8× the original rest row visibly jitters fin tips every frame
or two; brace's row is still for fifteen frames and shows one fin swing; settle's row is
uniform. At 6× in the crowded meal crops the three rows are hard to tell apart; the
braced feeding fins read as a steadier silhouette around the chewing body in the adult
case, and the juvenile case is too small to judge either way. The 3× larger single step
is a deliberate, rare event on an otherwise still animal; I judge that preferable to
continuous edge chatter for an ambient piece, with the caveat that its 0.5 s fade is a
point-bake blink, not a smooth swing. Not all metrics improve; the concentrated gesture
is the price.

## What was integrated

- `art/creatures/sail.tscn`: only the `LeftFin:rotation` and `RightFin:rotation` tracks of
  `rest` (keys 0/2.5/3/3.5/4 s → 0, 0, ±0.2, 0, 0) and `feed` (keys 0/0.25/0.5/0.75/2 s →
  ±0.2 held), transitions, interpolation and update mode unchanged. Body, bud, move and
  RESET tracks untouched.
- `assets/atelier/creatures.png`: regenerated with the current exporter
  (`scripts/art-bake.sh` → `art/bake.gd`, Godot 4.7.2) into a fresh directory, then
  adopted. The rebaked `creatures.png` hashes `3b1c3e44…`, **byte-identical to the study's
  brace candidate atlas**; `ground.png`, `habitat.png`, `plants.png`, `tall.png` and
  `pack.json` hash identically to the shipped files, so every non-sail row and both crown
  selectors and the vine selector are exactly preserved. Within `creatures.png` only atlas
  rows 4 (sail rest) and 6 (sail feed) differ from the previous atlas.
- No core, controller, timing, anti-aliasing or persistence change; no `sail_fin_coverage4`.

## Regression checks (new `crates/cubarium/tests/sail_calm_fins.rs`, fixture
`tests/support/sail-calm-rows.png` = the adopted 256×64 sail block)

1. The shipped sail block equals the fixture byte for byte (rest, move, feed, bud).
2. Rest: every frame but 12 is the held pose; frame 12 differs by 8–24 fin texels.
3. Feed: frame f equals f + 8; every non-still frame moves the same 8–32 chew texels;
   those are disjoint from the rest adjustment's fin texels.
4. The pack loads with 16-frame clips and still carries `vine_strips` and both
   `corner_cap_owner` selectors, and the loader sets both host caps' flags.

Host suites rerun green on the adopted atlas: `--lib` 240, `art_mode` 25, `lanternjaw` 16,
`meal_present` 13, `hunter_present` 29, `art_motion` 26, `run_present` 2.

## Limits

- Stills and numbers only. Root's clean release and any panel viewing remain the gate.
- The real-meal crops include neighbouring creatures and plants; the numbers are for the
  crowded crop, not an isolated sail.
- The gesture's readability at 1× on LEDs is unverified; its single-frame point-bake means
  it appears as a 0.5 s fade, and root may find it either a welcome sign of life or a
  blink. If the latter, the bounded revision is a second peak key (e.g. 3.25 s) to widen
  the pose, not a new sweep.
