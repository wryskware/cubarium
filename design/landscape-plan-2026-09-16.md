---
design_status: exploration
last_reviewed: 2026-09-16
---

# From strata to landscape: what the synthetic scene had that the live ring does not

LP-A, Opus, 2026-09-16, against
[the brief](handoffs/landscape-design-opus-2026-09-16.md). Read-only apart from
this file. Every claim about code cites `file:line` in the `tachyon-screen`
worktree.

## Summary

1. **The synthetic scene has no terrain.** Its ground is the same flat
   `h = 1 − 2v/H` banding the live ring uses (`synthetic.rs:582`). The "hills"
   are *vegetation masses against black sky*, not a height profile.
2. **The one number that separates the two pictures is negative space.** In the
   matched 320×180 pair — same raster, same 80×45 cells, same 16-px art — the
   synthetic has 27.0 % of pixels below luminance 0.05 and 18.9 % fully dark
   8×8 blocks; the ring has **0.0 % and 0.0 %**. Not one dark pixel anywhere.
3. **It is not a density problem.** At the *deployed* world (`ring:640x360
   --world-scale 2`, 80×45 cells) the live presenter draws ≈1,800 plants against
   the synthetic's 1,495. The instance counts already match. The `ring_640` golden
   is 160×90 cells at S = 1 and overstates the gap.
4. **It is a field-shape problem.** The synthetic's producer field is a
   hard-shouldered patch field whose dominant harmonic is *one turn of the ring*;
   58 % of its cells are bare. The live habitat noise has a wavelength of
   19–45 raster pixels — 7–33 cycles round the world — and never falls to zero.
5. **And a wash problem.** The producer ramp lifts every pixel it touches from
   `t ≠ 0` (`present.rs:236`). Inverting the ramp out of the ring golden: the
   *first percentile* of the foliage band still sits at `t ≈ 0.29`.
6. **Structure vs noise, measured**: block-mean std ÷ within-block std is 1.30
   for the synthetic (large forms dominate) and 0.95 for the ring (noise ties
   structure). That ratio *is* "confetti layer cake".
7. **The rain cannot read, structurally.** One weather blob peaks at `amplitude
   = 0.30` and `rain_threshold = 0.35` (`config.rs:487,553`), so a single shower
   makes **no rain at all**; two coincident blobs give one streak at 7.5 % opacity.
   The synthetic's column peaks at 0.97 and moves 48× faster.
8. **The wind is 5× too small.** Shipped `tip_px` is 0.45–0.9
   (`wind.rs:288–340`) against the synthetic's 2.5 and 5.0
   (`synthetic.rs:67,69`), and the live gust is one global envelope with 12 s of
   exact calm, not a travelling wave.
9. **Motion is mostly there**: `interpolate_on` and `turn_heading` are already
   used by the GPU driver (`sink/gpu.rs:862,864`). What is missing is a bob.
10. **Order**: the ramp shoulder first (renderer only, no schema), then the noise
    wavelength (config only), then rain, skyline, bob, wind, and terrain last.

## 1. What made the synthetic read as a landscape

The honest comparison is `synthetic-320x180-s1.png` against `ring_320x180.png`:
both 320×180, both 80×45 cells, both 16-px art, and the GPU adapter is a
transcription of the CPU presenter by construction (`adapter.rs:1–34`), so the
difference is *content*, not rasterisation.

| measured on the 320×180 pair | synthetic | ring | why |
|---|---|---|---|
| mean / median luminance | 0.162 / 0.095 | 0.294 / 0.291 | the ring has no dark values at all |
| pixels below L = 0.05 | **27.0 %** | **0.0 %** | `appearance.md` asks for negative space; the ring has none |
| fully dark 8×8 blocks | 18.9 % | 0.0 % | the synthetic has *places* that are empty |
| block-mean std ÷ within-block std | **1.30** | **0.95** | large forms vs per-cell mottle |
| skyline (first row with L > 0.25) | mean 26, **std 20** | mean 1, **std 2** | a silhouette vs a full-bleed blanket |
| foliage-band substrate `t`, p5 / p25 | 0.00 / 0.00 | 0.29 / 0.39 | the ramp never lets go |

The causes, in order of contribution:

**a. The producer field.** The synthetic builds it as
`smoothstep(0.44, 0.86, wrapped_noise(…)) × band(h)` (`synthetic.rs:273–283`).
Two things matter. The shoulder clips 58 % of cells below the first stage
threshold — computed over the 80×45 grid: producer mean 0.088, 63.4 % of cells
below 0.05, 2.8 % above 0.5, 41.6 % drawing any plant and only 12.8 % reaching
stage 2. And the noise's dominant harmonic is `ku = 1.0` at full amplitude
(`synthetic.rs:699`) — one cycle round the whole ring. The live field's shape
comes from `habitat.noise_wavelength = [0.6, 1.4]` embedded units with 6 waves
(`config.rs:476–477`); one embedded unit is `32·S` raster pixels, so features are
**19–45 px** wide, i.e. 7 cycles round a 320-px ring and 33 round a 640-px one.
Mottle, not landform.

**b. The substrate wash.** `draw_ramp_field_with` paints wherever `t ≠ 0`
(`present.rs:236`, and the same in `background.frag:19–22`), with brightness
`0.06 + 0.49·t²`. Inverting that curve out of the ring golden's foliage band
gives `t ≈ 0.29` at the 5th percentile and 0.39 at the 25th — the standing crop
is everywhere, so the floor colour `#12093A × 0.12` is never seen. In the
synthetic the bottom quartile of the same band is exactly the floor.

**c. Terrain: neither has any.** `synthetic.rs:582` is literally
`h = 1.0 − 2.0·(py + 0.5)/layout.h`, the same rule as
`ArtGeometry::band_of_height` (`habitat.rs:479–487`) against `CANOPY_TOP = 0.67`
(`habitat.rs:351`). The mounded shapes in the synthetic capture are clumps of
foliage with sky above them.

**d. Instance counts — not the gap.** Synthetic at 640×360 S = 2: 1,495 plants,
≤ 880 ground-cover lattice points, 12 tall columns (`synthetic.rs:203`) at ~8–11
stamps each, ~1,914 rain marks, 12 bodies. Live at the deployed
`ring:640x360 --world-scale 2` (`tachyon-w2-deploy-2026-09-16.md:60–61`): 3,600
cells, ~50 % gated off by `plant_cap`'s `rank_cap = 0` (`habitat.rs:569–573`,
`RANK_FULL = 0.22`, `RANK_MID = 0.50` at `:229,:232`) → **≈1,800 plants**, 880
lattice points, `0.22 × 80 ≈ 17.6` columns (`tall.rs:20,150–160`), and ~200 bodies
from 24 founders (`config.rs:519–545`). The same ink, near enough the same number
of marks. What differs is *where* they are and what is behind them.

**e. Bodies and wind** are covered in §5.

## 2. Terrain along the ring

**Proposal.** `ground(u) = G · Σₖ aₖ sin(2π kₖ u + φₖ)` over three harmonics with
integer turns `k ∈ {1, 2, 3}` seeded from `config.seed` — the construction
`wrapped_noise` already uses (`synthetic.rs:696–719`) and the one plan §8 proposes
for its biome field `R(p)`. It wraps exactly by construction. Recommended
`G ≈ 0.25` height units ≈ 5.6 cell rows ≈ 45 raster pixels at H = 360. An
authored `ground.png` read nearest-column is a drop-in later.

**Where it lives.** Three places, and `Topology::height` is the wrong one.

| option | what moves | cost |
|---|---|---|
| **(a) `Topology::height(p) = 1 − 2v/H − ground(u)`** (`geometry.rs:306`) | light, moisture, bands, `downhill`, **both controllers' height channel** | `Topology` is a `Copy` enum with no room for a profile and `height` is hot; and it silently redefines the neural policy's `height` input, invalidating every trained policy. **Reject for v1.** |
| **(b) habitat-level, beside `terrain`** (`habitat.rs:39,105,115`) | `light_base`/`moisture_base` read `y − ground[i]`; `terrain[i] = y − ground[i] + basin_gain·noise` | one new `HabitatConfig` pair (`ground_gain`, `ground_turns`) ⇒ `CONFIG_VERSION` bump ⇒ schema 17→18 ⇒ **fresh world** (the standing rule anyway) and FW-2's `ConfigProjection` hash re-pinned. **Recommended.** |
| **(c) presenter-only `ArtGeometry::ground(u)`** | `band_of_height`, `w_soil`, `foliage_rows`, `tall_anchor` | no schema, cube untouched behind a `Topology::Ring` guard; but the picture and the world disagree — a plant on a hilltop is lit as if it were in the valley |

**Why (b) is cheap where it counts.** `terrain` already exists and is already a
height-plus-noise field (`habitat.rs:115`, `basin_gain = 0.15` at `config.rs:478`),
and `water.rs:116–117` already flows on `s = terrain + depth_gain·w`. Adding
`ground(u)` to `terrain` puts pools in the valleys **with no change to
`water.rs` at all**. Today's moat exists because the bottom row has no outflow
and because nothing refills a hillside basin: at `evap_floor = 0.5` and
`evap = 0.008` a 0.5-deep pool dries in ~125 s, and rain almost never falls (§4).
Fix §4 and the basins fill on their own — worth a short run before assuming it.

**What the cube keeps: nothing changes.** `Topology::Cube` takes `ground ≡ 0`, so
`terrain`, `height` and every band are bit-identical; only the existence of the
new config keys moves the projection hash.

**What breaks, stated plainly.** The ring's `downhill` is *not* height-derived —
it is `neighbors[Bottom]` when `cy > 0` (`field.rs:212–217`). Under a ground
profile it still means "the cell below", so litter settles along raster rows
rather than into valleys. Making it gradient-following is one match arm in
`field.rs` and a retune of FW-6's `ring_field.rs`; it is a separate call.
Separately, **the canopy row stops being a row**: `foliage_rows(face)`
(`tall.rs:76–82`) and `tall_max_segments` (`tall.rs:93–98`) are computed from
column 0 and would have to become per-column.

**Visible effect.** The soil/foliage horizon becomes a ridgeline sweeping ±45 px;
the canopy line follows it; tall columns stand on the hills and their crowns
inherit the ragged skyline for free; water pools in the valleys instead of along
the floor.

## 3. Density and composition

At S = 2 the counts are already right, so the knobs that matter are the ones
that put *holes* in the field and *scale* into it.

| knob | today | recommend at 640×360, S = 2 | kind |
|---|---|---|---|
| producer ramp shoulder | none; paints from `t ≠ 0` | `t' = clamp((t − 0.45)/0.55)` — drops ring p25 from 0.39 to 0.07 | renderer + presenter constant |
| `habitat.noise_wavelength` | `[0.6, 1.4]` = 38–90 px at S = 2 | `[3.0, 8.0]` = 192–512 px ≈ 1.2–3.3 turns | **world config** (fresh world) |
| `habitat.basin_gain` | 0.15 | 0.25 with the `ground(u)` term | world config |
| `RANK_FULL` / `RANK_MID` | 0.22 / 0.50 | **leave** — the gate is already right | presenter constant |
| `TALL_COLUMN_P` | 0.22 ⇒ 17.6 columns / 640 px | 0.15 ⇒ 12 columns, the synthetic's spacing | presenter constant |
| tall height jitter | none — every crown on one row | hashed ±0.5 segment on `tall_rise` (`tall.rs:116–118`), ring-only | presenter constant |
| `founders.count` | 24 on 3,600 cells | raise proportionally per plan §5, later | world config |
| world scale S | 2 (deployed) | **keep** | world scale |

The "all crowns on one row" finding (FW-5 §2 item 3) is the same cause as the
flat skyline: `column_density` is a mean of wood over the column's foliage cells
(`tall.rs:240–243`), and with 24 animals on 3,600 cells every column's mean is
nearly equal. The synthetic dodges it by giving each column its own phase into a
slow height breath (`synthetic.rs:624–629`), which spreads twelve crowns over
`8 × 8.65 ≈ 69` raster pixels at S = 2. A hashed per-column offset inside
`tall_rise` buys the same ragged skyline from the real density.

## 4. Weather that reads

The rain *drawing* is identical on both sides — the synthetic transcribes
`environment::rain_marks` verbatim (`synthetic.rs:499–539` against
`environment.rs:316–362`). The difference is entirely the **field**.

| | synthetic | live ring |
|---|---|---|
| shape | triangular column, `d < 7` cells, hard edge (`synthetic.rs:304`) | raised-cosine cap over 55° (`habitat.rs:266`, `config.rs:552`) — soft over a fifth of the sphere |
| peak rate | **0.97** | `rain_rate·(Σ amp·cap − 0.35)`; one blob peaks at 0.30 < 0.35 ⇒ **zero**; two coincident ⇒ 0.15; all three ⇒ 0.33 |
| streaks per cell | `ceil(0.97×3) = 3` | `ceil(0.15×3) = 1` |
| opacity | `0.5 × 0.97 = 0.49` | `0.5 × 0.15 = 0.075` |
| width | 14 of 80 cells = 17.5 % of the ring | ~21 % of the world, soft-edged, ×2.03 vertical stretch at the rims (plan §5a) |
| speed | 3.2 cells/s = 25.6 px/s | one orbit in 20–47 min ⇒ **≤ 0.53 px/s**, 48× slower, and on a great circle, not along `u` |

**Cheapest change that gives a moving column**, in order:

1. `water.rain_threshold` 0.35 → **0.12** (config only). A single blob then rains
   at `0.6 × (0.30 − 0.12) = 0.11` d/s; raising `weather.amplitude` 0.3 → **0.6**
   takes a single blob to 0.29 d/s — a real column, without touching the model.
2. `weather.periods_min` `[20, 33, 47]` → `[3, 5, 8]` so a shower crosses the ring
   in minutes rather than half an hour.
3. Only if that still under-reads: `environment::RAIN_DENSITY` 3 → 8 and
   `RAIN_OPACITY` 0.5 → 0.6 (presenter constants, both already marked
   review-tunable at `environment.rs:244–248`).

Do **not** redesign the weather axis for v1. Plan §5a already accepted the cap's
shape distortion, and (1)+(2) are pure config on a fresh world.

## 5. Motion

**Interpolation is already used.** `sink/gpu.rs:862` calls
`present::interpolate_on(topology, &o.moved, o.pos, o.heading, f)` and `:864`
smooths the heading through `turn_heading` (`art_present/mod.rs:298–308`), which
spends the tick's whole turn across its frames. `BODY_FADE_SECONDS = 0.3` already
cross-fades state clips. So "continuous position" and "heading smoothing" are
done; do not re-implement them.

**What is missing is the bob.** The synthetic adds
`y = start_y + bob₀·sin(2π·s·bob₁)` with `bob₀ ∈ [1, 4]` px and
`bob₁ ∈ [0.3, 0.9]` Hz (`synthetic.rs:219,548`), plus a **damped** tilt: the path
slope is scaled by 0.2 before it becomes the heading (`synthetic.rs:554`),
because nearest-neighbour rotation of a 16-px tile past ~20° breaks its outline.
That is the whole of "floaty". It belongs in `sink/gpu.rs::creature` as a
presentation offset on the drawn anchor only — it must not move the world
position. Gate the amplitude on the phenotype: gliders and skimmers bob, the
burrower does not.

Pace is not the problem: cruise is `1.0 BL/s = 5.0 px/s` (`config.rs:566–573`)
against the synthetic's 6–16 px/s at S = 2, which is 3–8 px/s in S = 1 terms.

**The sway.** Shipped per-species `tip_px`: lanternstalk 0.45, tendrilfan 0.55,
reedspire 0.70, spiretree 0.9, glowcap 0.12, rootveil 0 (`wind.rs:283–340`) —
against the synthetic's `PLANT_TIP_PX = 2.5` and `TALL_TIP_PX = 5.0`
(`synthetic.rs:67,69`). **5.6× and 5.5×.** The synthetic states why the cube's
budget does not bind on a ring (`synthetic.rs:60–66`); GS-1c item 2 is already
making `bend_budget` topology-aware, which lifts the ceiling from the cube's 9-px
footprint to the ring's `max_local_radius` (90 px at 320×180). This package only
decides what to *spend* it on, and must land after GS-1c.

Timing differs too: the live gust is one global envelope on `WIND_PERIOD = 30 s`
with `WIND_QUIET_SECONDS = 12` of exact calm and a 0.6 s travel lag
(`wind.rs:19,41,51`) — nearly synchronous across the panel. The synthetic runs a
permanent travelling wave, `0.65·sin(2π(2u − 0.11s)) + 0.35·sin(2π(5u − 0.19s))`
(`synthetic.rs:684–689`): two and five crests round the ring, moving at 70 and
122 px/s at 640. A ring should carry the gust *across* it, not blink it on and off.

## 6. Packages

Ordered. Each is independently verifiable and none coordinates writes with
another. GS-1c owns `crates/cubarium-gpu/**` and the host sink/CLI files until it
reports, so P1's shader edit and P5's `sink/gpu.rs` edit queue behind it.

| # | package | owns (files) | schema | verification | size / effort |
|---|---|---|---|---|---|
| **P1** | **The dark floor comes back.** A producer-ramp shoulder `t' = clamp((t − PRODUCER_SHOULDER)/(1 − PRODUCER_SHOULDER))`, `PRODUCER_SHOULDER = 0.45`, ring-only guard so the cube stays pinned | `cubarium/src/present.rs`, `cubarium-gpu/src/palette.rs`, `shaders/background.frag` | none | re-record `ring_320x180.png`; target ≥ 15 % of pixels below L = 0.05 and ≥ 10 % dark 8×8 blocks; cube golden byte-identical | small / **medium** |
| **P2** | **Hills by wavelength.** A `landscape.toml` under `runs/` with `habitat.noise_wavelength = [3.0, 8.0]`, `basin_gain = 0.25`; no code at all | a new config file only | fresh world (rule, not migration) | fresh 640×360 S = 2 run, capture at tick 3,000; block-mean std ÷ within-block std ≥ 1.2 | small / **low** |
| **P3** | **Rain that reads.** `rain_threshold 0.35 → 0.12`, `weather.amplitude 0.3 → 0.6`, `periods_min → [3,5,8]` in the same TOML; `RAIN_DENSITY`/`RAIN_OPACITY` only if still thin | the same config file; `art_present/environment.rs` if needed | fresh world | a capture with a visible moving column; measured peak rate ≥ 0.6 d/s and ≥ 3 streaks/cell | small / **medium** |
| **P4** | **A ragged skyline.** Hashed ±0.5-segment offset inside `tall_rise`, ring-only; `TALL_COLUMN_P 0.22 → 0.15` | `art_present/tall.rs` | none | crown-row spread ≥ 40 px at S = 2; cube golden byte-identical | small / **medium** |
| **P5** | **The bob.** Per-phenotype vertical bob + damped tilt on the drawn anchor, transcribed from `synthetic.rs:548,554` | `cubarium/src/sink/gpu.rs` (after GS-1c) | none | a 10 s capture at 60 fps; the world position is unchanged (assert the anchor offset is presentation-only) | small / **medium** |
| **P6** | **Wind that reads on a ring.** Raise `WIND_RESPONSE.tip_px` ~5× for the ring; replace the global envelope with a travelling wave | `art_present/wind.rs` (after GS-1c item 2) | none | per-clip ring budgets from GS-1c; capture showing crests crossing the panel | medium / **high** |
| **P7** | **Terrain `ground(u)`.** Option (b) of §2: a `ground` array beside `terrain`, `HabitatConfig.{ground_gain, ground_turns}`, presenter bands per column | `cubarium-core/src/habitat.rs`, `config.rs`, `art_present/{habitat,tall}.rs` | **17 → 18, fresh world** | ridgeline visible; water pools off the floor; cube `CubeProjection` unchanged but for the new keys | large / **high** |
| **P8** | FW-7 (pack v6 at `TILE = 32`) and FW-8 (biomes) as already planned | per plan §9 | per plan | per plan | large / high |

P1 is first because it is the largest change to the picture for the least risk:
three files, no world, no schema, and it restores the one property
`design/appearance.md` names first. P2 is second because it is *zero* code.

## 7. What this pass could not determine

* Whether hillside basins actually hold water once rain reads — the mechanism is
  there (`water.rs:116–117`) but the evaporation balance needs a short run.
* The live world's plant count at tick 3,000 on the panel; I inferred ≈1,800 from
  the rank gate and the saturated field rather than measuring a running world.
* Whether `PRODUCER_SHOULDER = 0.45` is the right value. It is derived from the
  ring golden's own percentiles; it is a knob for a viewing session, and it
  belongs on `design/backlog.md`'s operator-control list either way.
