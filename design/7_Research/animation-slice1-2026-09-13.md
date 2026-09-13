---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Animation slice 1 — what landed, measurements, limitations

Research record for the first animation pass (authorization:
`animation-implementation-handoff-2026-09-13.md`; plan inputs:
`astra-animation-plan-2026-09-13.md`, `astra-animation-review-2026-09-13.md`; work order
and its post-review revisions: `animation-slice1-brief-2026-09-13.md`). Evidence, not
canon. Nothing here changes the simulation, its tick rate, saved worlds, the shim, or the
display contract; without `--art` the image is unchanged.

## Baseline (build 9413f9f, before the slice)

Captured with the baseline binary from a copy of the live `state/strata-latest`
snapshot at tick 382800 (population 95–96), `--art assets/atelier --sink png`, 1×, 60 fps,
native 64×64 net PNGs (256×128). "Jump" is the largest single-pixel change (sRGB, 0–1)
between consecutive captured frames.

| capture | frames | mean \|Δ\| per pixel per step | steps with a pixel jump > 0.5 |
| --- | --- | --- | --- |
| every 6th frame, 12 s | 120 | 0.0151 | 119 of 119 |
| every frame, 4 s | 237 | 0.0032 | 79 of 236 (one in three: every tick boundary) |

Median per-step maximum jump at 60 fps was 0.031 between ticks and ≈ 0.83 at tick
boundaries: between ticks only interpolated body travel and rain moved; at each tick every
discrete clip frame, plant pose, ground breath and body heading could pop. Release draw
cost of the crowded synthetic scene (`plant_and_body_draw_cost`, 622 plants, 200
organisms): **6.79 ms/frame** (41 % of 16.7 ms).

## What landed

Pack and bake (`art/bake.gd`, `assets/atelier`, pack **v4**):
- Sample counts are data: 16 creature samples per clip (was 8), 24 plant/tall samples per
  3 s clip (was 4, i.e. 750 ms holds). Atlases: creatures 256×256, plants 384×384, tall
  384×112; ground tiles unchanged (4 authored frames). The bake is byte-reproducible
  (rebaked twice, identical) and every phase-0 sample equals the v3 pack's. All 24
  samples of every clip pass the loader's nine-pixel extent, trunk-periodicity and
  cap-join checks.

Renderer (`cubarium-render::sprite`):
- `Pose { first, second, mix }`, `Mask::{None, Axial, Radial, Strip}`, `stamp_pose`,
  `stamp_layers`: one surface unfold, per-pixel premultiplied linear mix of up to three
  temporally blended poses, one source-over, coverage masks with a start envelope so a
  reveal tending to zero tends to an empty stamp. `stamp_sprite` is the still case, bit
  for bit. `Sprite::subtract`/`paints_like` derive a tall plant's **cap**.

Host (`cubarium::art`, `cubarium::art_present`):
- `Clip::sample` (temporal pair + mix; loops wrap last→first, the bud clip clamps).
- One presentation clock, `present_seconds(tick, f) = (tick − 1 + f)·DT` (tick 0 → 0),
  for every looping clip, the water shimmer, the ground breath and the rain — the same
  interval the bodies travel along. Rain streaks are weighted over three pixels (constant
  light as they fall); the top-face sparkle is a raised cosine.
- Paced growth, advanced only in `observe` by simulated tick time: one stage per
  `STAGE_GROW_SECONDS` (4 s) up / `STAGE_WILT_SECONDS` (2 s) down, reversible mid-step,
  drawn as the lower stage fading under the upper stage revealed up the stalk (side
  faces) or from the center (top face); the hysteresis targets of `next_stage`/`next_tall`
  are unchanged. Tall columns extend at 1.5 px/s and decline at 3 px/s, every trunk/vine
  row composited exactly once (`Mask::Strip` up to `tall_grown_px`), the cap gliding at
  the continuous height; tall parts at `TALL_OPACITY` 0.95. `draw` interpolates the last
  two observed states by the frame fraction (`growth_between`, `tall_between`) and never
  mutates; a fresh presenter, or a tick going backwards, snaps to the fields' targets from
  an empty baseline (no forest replay, no inherited hysteresis).
- Bodies: a 0.3 s cross-clip fade on state change with each clip keeping its temporal
  blend, a mid-fade change starting from the blend on screen; and the per-tick turn the
  world applies to a body is spent over the tick's frames (`turn_heading`), art mode only.
- Fruit accent: fades in over 2 s once a full-grown plant's cell holds fruit; gone the
  frame the field drops below `FRUIT_SHOW` (and `draw_with_fruit(.., None)` always
  suppresses it) — the accent is the food signal, deliberately not a lingering decoration.

Docs: `art/README.md` (Live world: presentation clock, paced growth), `art/PLANTS.md`
(24-sample contract, pack v4, cap derivation).

## Measurements after

Same snapshot copy, same commands, final build:

| capture | frames | mean \|Δ\| per pixel per step (max) | median / p90 / max per-step jump | steps with a jump > 0.5 |
| --- | --- | --- | --- | --- |
| baseline, every frame, 4 s | 237 | 0.0032 (0.0115) | 0.031 / 0.831 / 0.890 | 79 of 236 |
| slice, every frame, 4 s | 237 | 0.0029 (0.0032) | 0.273 / 0.314 / 0.576 | 2 of 236 |
| slice, every 6th frame, 12 s | 120 | 0.0132 (0.0146) | 0.776 / 0.827 / 0.859 | 119 of 119 |

Reading: the per-frame change is now spread evenly (the maximum per-step mean fell from
0.0115 to 0.0032, the median per-step maximum jump rose from 0.031 to 0.273 because
motion is distributed across frames, and the measured tick-boundary spikes largely
disappear in this short clip; this is not a guarantee for every world event). The two
remaining > 0.5 steps in 4 s are single pixels at the rim's bottom row (a body reaching
the open rim), state changes rather than presentation steps. At 0.1 s spacing (every 6th
frame) consecutive captures still differ by whole sprites, as they must: 0.1 s of a 1.6 s
walk cycle is a different pose.

Before the turn smoothing the 60 fps capture still showed 67 of 236 steps above 0.5, all
on tick boundaries and all body-coloured pixels — the world turns a body in one step per
tick and the presenter used to draw that step at once.

Release draw cost, crowded synthetic scene (`cargo test --release -p cubarium --lib --
--ignored plant_and_body_draw_cost --nocapture`): **9.86 ms/frame** (59 % of 16.7 ms;
baseline 6.79). The increase is the second sprite sample per pixel of the temporal blend
plus the interpolation bookkeeping; it leaves 6.9 ms of headroom at 200 organisms and 622
plants, more than the live worlds carry (≈ 95 organisms).

Root's additional all-effects release fixture (`art_water::everything_on_draw_cost`:
wet, raining, rich, tall columns and 200 organisms) measured **10.453 ms/frame**, about
63 % of the 16.7 ms draw budget on this machine. These are draw timings, not proof of
browser or hardware end-to-end frame pacing.

Tests (all green at the end of the slice):
- `cargo test -p cubarium-render`: 24 unit + 7 (`astra_regressions.rs`) + 10
  (`pose_layers.rs`, independent authoring pass).
- `cargo test -p cubarium`: 111 unit (1 ignored timing test), `art_bands` 6, `art_mode`
  25, `art_plants` 13, `art_water` 21, `astra_motion_regressions` 4, `art_motion` 25 (+ 1
  ignored capture; independent authoring pass from the public doc comments — clip
  sampling, temporal continuity against the discrete path, growth pacing/reversal/
  interruption/flooding/backwards tick, render-rate independence and draw purity,
  60 fps growth continuity, tall rate/glide/crisp-segment/whole-segment crossing, rain
  weights and sparkle, body fades including double switches, fruit gating, cap
  derivation), and the unchanged `png_layout`, `raycast_inversion`, `run_*`, `shim_sink`
  suites. The final root rerun of both packages with `--all-targets` also passed.
  The debug test build was warning-free; the release build still emits the existing
  unrelated `cubarium-core::AUDIT_TOLERANCE` dead-code warning.
- The independent pass found no implementation violation of the documented behaviour;
  it did find that a repeated `observe` of the same tick folded the previous-tick copy
  forward (so a mid-tick frame stopped interpolating after a duplicate call). Fixed: the
  copies advance only on a new tick, and the test now asserts that a duplicate observe
  changes neither growth, nor the previous copy, nor any frame inside the tick.
- Existing tests adapted to the new signatures without weakening: `clip_time` takes
  presentation seconds; `water_brightness` takes seconds; `ground_frame` → `ground_pose`;
  `rain_marks` returns weighted marks; hand-stamped expectations use `Clip::sample` +
  `stamp_pose` at `present_seconds(tick, 0)`; the legacy "rules as written" tall column now
  paints single-ownership strips and the cap; two tests that asserted "the frame fraction
  must not move the water/pulse" now assert the opposite, which is the point of the slice;
  the fruit test observes the fruit field it draws with.

## Captures

Convenient review videos are in the gitignored `captures/animation-review/` directory:
`ambient-preview.mp4` (4 seconds at 60 fps) and `growth-preview.mp4` (45 seconds,
sampled at 10 fps). Both are nearest-neighbour enlargements of native 64×64-face
output, not higher-resolution rendering. Root also started a separate 1×/60 fps
web preview at `http://127.0.0.1:7396/` using a copied snapshot under
`/tmp/cubarium-animation-preview-My6gKf`; the original runners and state were untouched.

Native 1× net PNGs (256×128) under `/tmp` (not in the repository):
`/tmp/cap-base` and `/tmp/cap-base-60` (baseline, every 6th frame for 12 s, every frame
for 4 s), `/tmp/cap-new` and `/tmp/cap-new-60b` (final build, same). Nearest-neighbour
enlargements were made for review (`/tmp/cmp-f0-x4.png` baseline vs new first frame,
`/tmp/new60-front-strip-x8.png` six consecutive 60 fps frames of a front-face column). At
1× the first frames of baseline and slice are the same scene to the eye — the plants,
columns and bodies stand where they stood — with the caps sitting on their trunks as the
crowns did; the difference is in the motion, which the statistics above carry.

A scripted bare → rich → bare growth capture is the `#[ignore]`d
`growth_sequence_capture` test in `crates/cubarium/tests/art_motion.rs`
(`CUBARIUM_CAPTURE_DIR`, default `/tmp/cubarium-growth`): 45 simulated seconds at 1×,
every 6th frame, 450 native net PNGs, the last byte-identical to the first. Reviewed as
a 6× nearest-neighbour grid at 3 s spacing (`/tmp/growth-grid.png`): the spiretree
column extends with its cap gliding and the vine coiling up behind it, the tendrilfan
and the top-face umbrellafrond reveal stage by stage, the lanternstalk's warm fruit
accent fades in and later leaves, and everything declines back to bare ground; the
solid blue bar above the young column is the fixture's saturated producer ramp being
covered as the tree grows, not a rendering artifact.

## Limitations and follow-ups

- **Restart truth.** A transition in flight is not persisted: after a restart the plants
  and columns stand at the fields' targets (snap), not part-way. Persisting visual growth
  would be a separate change with its own state schema.
- **Targets step at 20 Hz.** The hysteresis targets change on ticks; the visuals
  interpolate toward them at the render rate, and the sway/shimmer/rain/ground/bodies are
  continuous at the render rate. The remaining tick-locked events are real state events
  (births, deaths, a body reaching the rim, a fruit eaten).
- **Fruit leaves in one frame** by decision (food signal), so a bite is a pop of the
  accent pixels. If it reads harsh at the cube, the alternative is a ≤ 1 s decorative
  fade that would briefly show food the field no longer holds.
- **Reveal masks stand in for authored growth poses.** The upper stage is uncovered from
  the base (or the center) over the authored stage clips; an "extending stalk / unfolding
  leaf" clip per plant would read better and would replace the masks, not the pacing.
- **Spatial quantization of the bake.** `bake.gd` still floors source coordinates, so a
  slowly turning one-pixel stem holds the same pixels through several samples and then
  changes; the temporal blend spreads that over one 125 ms sample interval. Astra's 4×4
  coverage bake would remove it at the cost of softer pixel art; deferred.
- **Trunk density changed** from an accidental triple stacking of 0.85 (≈ 0.997 effective)
  to a single 0.95 (`TALL_OPACITY`, review-tunable). Glass canes' translucent pixels are
  now as translucent as authored.
- **Body-fade memory is bounded** to two remembered states plus the current one; a third
  change inside one 0.3 s fade drops the lightest layer (a step of at most one third of a
  fade, never seen in the captures).
- **Cost** rose to 9.8 ms/frame on the crowded fixture; a nearest-sample fast path when
  `mix` is tiny, or sampling both frames through one bilinear weight set, would win most of
  it back if a denser world needs it.
- Shared wind, runtime part rigs, droplets and nibbles remain deferred as authorized.
