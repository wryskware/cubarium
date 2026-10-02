# Glowcaps flicker like slow candles; the vine draws as tiles without textures — 2026-10-02

Wrysk, 2026-10-02: "glowcaps should have an animation with brightness that varies a bit
like a slow candle. maybe 5-10% of caps are at max brightness at any time, with a
reciprocal curve distribution. and later we can make it so that caps react to local
animal movement to trigger a bright glow spike" / "lets enable texture just for the vine".

Worktree `.claude/worktrees/plants-glow`, branch `plants-glow` off `plants` (5c46a41d).
Commit with explicit paths; never merge, never deploy, never touch the live world, never
open a window (AGENTS.md: headless captures only). Follow `WORKING_POLICY.md` (fast
iteration, short checks; animation evidence is a real-time MP4, x264 yuv444p crf 12).
Presentation numbers are config fields with a doc comment and unit (no registry row: the
registry is world/flora/fauna). Make the routine design calls yourself and report them.

## A. Vine tiles without textures

Today `VoxelConfig::textures` (`crates/cubarium/src/voxel/mod.rs` ~165) switches both the
terrain face textures and the latticevine's tile layer: the atlas loads only with it
(`sink/gpu/voxel.rs` ~1420), the packer and `Stands::vine_cells` pick tiles vs plain cells
(`sink/gpu/voxel.rs` ~713/781, `voxel/stand.rs` ~306/622, `voxel/present.rs` ~389). With it
off, every covered face is one solid leaf-blue cell, so a covered wall reads as blue rock.

- New `VoxelConfig::vine_tiles: bool`, default **false** (plain voxels stay the main draw
  everywhere else; the panel and cube configs do not change). With `textures` off and
  `vine_tiles` on: terrain and every other organism draw exactly as today, and the vine
  draws as its tile layer (load only what the vine needs). `textures` on behaves as today.
- `config/desktop/terrarium.toml`: `vine_tiles = true`.

## B. Glowcap candle

Glowcap lips emit (`colours.rs` ~555–600 `Lip`, `plant_emission`; `stand.rs` ~725 lip
state). The emitting texel draws unlit at full value and its light goes into the glow
volume (`sink/gpu/light.rs` ~416 `Glow`, rebuilt in `prepare`, `sink/gpu/voxel.rs` ~848,
when emitters or terrain move). Give every glowcap with a lip a brightness `b(t)` that
multiplies **both** its texel's emission and the light it casts:

- **Distribution across caps at any instant: density ∝ 1/b on [`candle_min`, 1]**
  (reciprocal), so `b = candle_min^(1 − u)` for a `u` uniform over time. With
  `candle_min` **0.25**, about 7.6 % of caps sit within 10 % of full (`b ≥ 0.9`):
  `ln(1/0.9) / ln(4)`. That is Wrysk's 5–10 % at max.
- **Slow and continuous:** each cap's `u(t)` wanders smoothly with a period of the order
  of `candle_period_s` (**8 s**, a knob) on the presenter's every-frame clock
  (`(tick − 1 + f) · DT`), its own phase from the stand id (deterministic, decorrelated
  between caps). It must stay uniformly distributed over time (a plain sine is not:
  map it through its own CDF, or equivalent). No step ever pops (animation-never-pops):
  per-frame change is small at 60 fps.
- Knobs under `[light]` (or a `[light.candle]` table, your call): `candle_min`,
  `candle_period_s`. `candle_min = 1` is today's picture exactly.
- **Cost:** the glow volume is CPU-built. If rebuilding it per frame is too dear, rebuild
  it per tick (20 Hz) with the tick's brightnesses while the texel follows the frame
  clock — measure and say which. Keep the desktop terrarium at 60 fps; report the pack and
  GPU ms per frame before and after.
- **Leave room, don't build:** a later package adds a bright spike when an animal moves
  near a cap (brightness = max(candle, spike) or similar). Keep the per-cap brightness
  one function a spike can join; implement nothing for it now.

Note for the capture: in a dense carpet, a cap's lip (its front face's lowest rows) is
hidden by the cap one row nearer, so only the front row's lips show (477 of 1,062 caps on
seed 1 at minute 116 have a cap directly in front). Not this package's to change; pick a
cave where lips are visible.

## Tests first (one failing commit), then the implementation

1. `b(t)` over 2,000 caps × 600 sampled instants: share with `b ≥ 0.9` within 5–10 %
   at `candle_min` 0.25; the median within ±0.05 of `candle_min^0.5` (0.5); every sample
   in [`candle_min`, 1].
2. Continuity: for every cap, `|b(t + 1/60 s) − b(t)| ≤ 0.02` over a minute of frames.
3. Determinism: same stand id → same curve; two ids' curves are not equal.
4. `candle_min = 1` → `b ≡ 1`.
5. `vine_tiles` defaults false; with `textures` off and `vine_tiles` on, the vine draws
   through the tile layer and a terrain face draws as the plain path does.

`cargo nextest run -p cubarium` (plus any crate you touch).

## Checks and return

- Build the state with this branch's `light_capture` (`--config config/desktop/terrarium.toml
  --seed 1 --ticks 132000 --state DIR`, `CUBARIUM_THREADS=16`, nice 10; ~15 min). The flora
  snapshot carries its config, so a state from another build may not load.
- One **MP4**, ~10 s real time, lit tier, night (`--weather-preview fixed:0.0,0.2`), cropped
  on a glowcap cave, `--anim` at 60 Hz.
- One still with `textures` off and `vine_tiles` on, the whole terrarium at 6 px.
- Absolute paths for both. Return ≤ 40 lines: commits, the design calls you made, the
  frame-cost numbers, anything that refused a decision here. Stop your wait loops; leave no
  background shells.
