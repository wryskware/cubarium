---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Workstream B (Opus): make ecology v1's plant state readable on the cube

Fable orchestrates under
[the next-steps handoff](ecology-v1-next-fable-2026-09-15.md), section "B".
You own the presenter, the render crate and visual verification; Fable reviews
once, with at most two repair cycles. Model: Opus 5, high reasoning effort
(this is substantial UI construction). Time target: one working session.
Work under `AGENTS.md` and `WORKING_POLICY.md`.

**You are working in a separate git worktree of the repository.** Another
worker is editing `crates/cubarium-search` and core diagnostics on `main` at
the same time. Set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target` for
every cargo command so there is one build cache (the first build in the
worktree recompiles the workspace crates once; that is expected). Touch only:
`crates/cubarium/src/art_present/`, `crates/cubarium/src/art.rs`,
`crates/cubarium/src/present.rs`, `crates/cubarium-render/`,
`crates/cubarium/tests/`, `crates/cubarium/examples/`,
`crates/cubarium-core/src/view.rs` and `crates/cubarium-core/src/world/view.rs`
(only to add read-only fields to `RenderView`, see below), and
`design/7_Research/`. `assets/atelier` only if a small authored tile is
unavoidable; say so. Commit on your worktree branch, staging by path. Do not
push, tag, restart the cube, or touch `state/`, port 7393 or the running
`cubarium` process; the physical cube and the shim are owned by the live
runner and are not available to you. Commit messages end with
`Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

At the real 64 × 64 face resolution, with the normal `assets/atelier` pack and
the established visual language, make three things readable:

1. **Living structure persists when foliage is stripped.** A stand with wood
   `W > 0` and foliage `P ≈ 0` is visibly a plant, not empty soil.
2. **Foliage loss and recovery happen on that structure**, following the actual
   stocks continuously, without flicker and without visual regrowth that masks
   ecological depletion.
3. **Dead wood** (`Wd > 0`, `W = 0`) is visibly distinct from living structure
   and from soil, and fades as its stock decays.

Do not imply seven independently simulated species: the seven plant forms are
visual picks over shared per-cell stocks, as today. Remains keep the existing
fleck treatment. No analytical overlays in the normal display. No core
ecological change.

## Read first

- `design/ecology-v1-contract.md` §3 (state), §12 (presentation boundary),
  §11 (the arithmetic: ungrazed steady states near `P ≈ 0.50, W ≈ 0.33` in
  average light and `P ≈ 0.56, W = 0.6` in bright light; `α = 2`,
  `W_max = 0.6`, `W_min = 0.02`; a stripped stand reflushes to at most
  `0.25·α·W`).
- `design/7_Research/ecology-v1-implementation-2026-09-15.md` "Run 3", rows
  B0 (measured stand states: bright `W ≈ 0.39` at 30 min, reserve full), B1a
  and B3 (what stripping and reflush look like in numbers), B4b (dead wood).
- `crates/cubarium-core/src/view.rs` `RenderView`: `producer` (foliage `P`),
  `wood`, `plant_reserve`, `dead_wood`, `carrion`, `detritus`, `fruit`,
  `producer_max`. Filled in `crates/cubarium-core/src/world/view.rs`
  `render_view`.
- `crates/cubarium/src/art_present/`: `mod.rs` (`observe_with_fruit`,
  `draw_with_fruit`), `habitat.rs` (`plant_density`, `stage_thresholds`,
  `next_stage`, `stage_opacity`, `STAGE_HYST`, the soil ramp constants),
  `growth.rs` (paced stage steps), `tall.rs` (`column_density`, `tall_target`,
  `next_tall`, `draw_column`), `environment.rs`. The presenter's contract: a
  `draw` mutates nothing, `observe` advances by one tick, re-observing a tick
  is idempotent, hysteresis prevents stage flicker. Keep all of that.
- `crates/cubarium/tests/art_*.rs` and `art_present/tests.rs`: the pixel-exact
  test style you extend. `crates/cubarium/examples/meal_capture.rs`: the
  precedent for drawing a synthetic view to PNG.
- `~/vuzic/led-cube-shim/docs/ARCHITECTURE.md` face convention; reuse
  `cubarium_surface` geometry helpers. `cubarium run --sink web --web-port N`
  serves the same viewer as the cube.

## Design decisions already made (state their visible effect in your note; Wrysk may veto)

- **Structure comes from wood.** The stage a cell's plant shows (and a tall
  column's height) is driven by `wood / wood_max` through the existing band
  thresholds and hysteresis, replacing the producer read for the foliage and
  canopy bands. Calibrate the normalisation so B0's measured average-light
  stand reads as stage 1–2 and a bright stand as stage 2, a stand near
  `W_min` as stage 0, and state the mapping. Add `wood_max: f64` to
  `RenderView` beside `producer_max` (filled from `config.plant.wood_max`) and
  update every fixture that builds a `RenderView` by hand. Nothing else in
  core.
- **Foliage fullness is a continuous layer on the structure.** Fullness
  `f = clamp(P / (k · W), 0, 1)` with a presenter constant `k` chosen so B0's
  ungrazed stands read as full (`P/W ≈ 1.5` average, `≈ 0.95` bright, so `k ≈ 1`
  is the starting point; justify what you pick). The normal stage sprite is
  drawn at opacity scaled by a smooth ramp of `f`; underneath it, the same
  sprite's silhouette is drawn filled with a **living-wood tone**: dim, warm
  and clearly not the soil ramp (`SOIL_LOW_SRGB`/`SOIL_HIGH_SRGB` are dark
  plum → violet-mauve). A stripped living stand therefore reads as a dark
  plant-shaped silhouette on the ground. No new stage sprites.
- **Dead wood is the same silhouette in a dead tone**, desaturated and cooler
  than living wood, at a stage derived from `dead_wood` through the same wood
  mapping and an opacity that fades with the stock. When a stand dies its
  living structure disappears and the dead silhouette takes over; as `Wd`
  decays the silhouette fades to soil. No memory of the former species is
  needed if the cell's species pick is deterministic per cell, as it is today.
- **Soil-band plants and flecks read litter plus remains** (`detritus +
  carrion`), as the contract allows.
- **Fruit accent** keeps its current rule, keyed on `fruit`.
- **Tall columns**: height from wood, crown/cap foliage by fullness, trunk
  drawn while alive, dead column in the dead tone. If the column path cannot
  be done in the session, drive at least its height from wood so stripped
  columns do not collapse, and report the rest as not done.
- **Pacing**: fullness follows the stocks per tick with the presenter's
  existing per-frame interpolation; no extra low-pass filter unless a measured
  flicker needs one (say so with the measurement). Stage steps keep their
  paced clips and hysteresis.

## Deliverables

1. The presenter change above, with the `RenderView` field.
2. **Tests, authored as their own pass** (pixel-level, in the existing style):
   healthy, half-grazed, stripped-but-living, dead-wood and empty cells draw
   pairwise distinguishable images at the same cell; fullness is monotone
   (lower `P` at fixed `W` never draws more foliage); a stripped stand at fixed
   `W` draws the same structure as the healthy one under the foliage; dead
   wood fades monotonically with `Wd` and reaches soil at zero; a tick-by-tick
   strip-then-reflush sequence produces no frame-to-frame change larger than
   a stated bound (no flicker); `draw` still mutates nothing and re-observing
   a tick is idempotent; existing suites pass or are updated with a stated
   reason per change.
3. **Visual evidence**: a small contact sheet (one PNG, a few hundred KB at
   most, under `design/7_Research/assets/`) drawn through the real
   `ArtPresenter` from synthetic views: the five states side by side for one
   foliage-band and one canopy-band pick and one tall column, plus one
   recovery sequence (strip → reflush → regrow) as a strip of frames from a
   real `World` stepped with a pinned grazer (`World::pin_cell_habitat` and the
   B1a fixture in `crates/cubarium-core/examples/ecology_v1_scenarios.rs` show
   how). Not an asset campaign.
4. **Viewer check**: run a fresh schema-16 world headless-to-web on a scratch
   state directory and a port other than 7393 (for example
   `cubarium run --sink web --web-port 7394 --state <scratch>/state --fresh`)
   for a few minutes; inspect the normal viewer for seams, readability and
   frame pacing, and report exactly what was observed. Say plainly that the
   physical cube was not inspected.
5. **Result note** `design/7_Research/ecology-v1-presentation-2026-09-15.md`:
   the mapping constants and why, the visible effect of each decision above,
   the contact sheet, test totals, the viewer observations, what was not done,
   and the commands to reproduce the sheet.
6. `graft build`; commit on the worktree branch.

## Constraints

- No change to any core file other than the two `view.rs` files, and there
  only the added field(s). No change to `fields.rs`, `step.rs`, config,
  snapshot, neural, search.
- Keep the 60 fps budget: report the presenter's per-frame cost before and
  after on the same fixture (`crates/cubarium/tests/animation_load.rs` is the
  precedent).
- Normal display stays free of diagnostics.
- One build cache (`CARGO_TARGET_DIR` as above). Scratch state under your
  worktree or `/tmp/claude-1000/...`; delete it when done.

## Decision authority

Yours: the exact tones, ramps, thresholds and normalisation constants; whether
the silhouette is derived from the sprite alpha or from a per-pixel luminance
rule; test structure; the sheet layout. Fable's: any core change beyond the
`RenderView` field, any change to what the stocks mean, any new asset.
Wrysk's: the look, after he sees the sheet.

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-render
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-core
```

Fable's review looks at the contact sheet, reads the mapping code and the
`RenderView` diff, and re-runs the flicker and distinguishability tests.

## Return format

The result note, plus in the final message: branch and commit hashes, test
totals for the three crates, the sheet path, the mapping constants, the
per-frame cost before/after, the viewer observations, what was left undone,
and measured usage. Link files; paste no logs.

## Stop

Stop after the note and commit. Deployment of the fresh display world (D) is
Fable's, after integration.
