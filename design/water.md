---
design_status: leaning
last_reviewed: 2026-09-12
decision_refs: []
---

# Water: rain, flow, pools

Wrysk asked for water on the cube: pooling in low places, rain, and flow,
"even if it's just a simulated flow". This note is the design Fable chose. It
is `leaning`, adopted for implementation under Wrysk's task authorization.

Water is not material. It is an open quantity with an audited budget, like
energy: rain adds it, evaporation removes it, and every tick
`Δ water_total = rain_in − evap_out` to rounding. Both sides are logged.

## Quantities

Per cell: water depth `w ≥ 0` (unit `d`, checkpointed with the fields; a
snapshot without it loads dry). Static terrain height per cell
`z = h + basin_gain · n_b(p)`, where `h` is the embedded height (Top = 1,
rim = −1), `n_b` is the habitat patch noise sampled at `p + (0.53, 0.29,
0.71)`, and `basin_gain = 0.15`: hollows deeper than the standing water is
tall, so the flat top and the bottom row of the sides break into separate
pools rather than one moat (0.06 and a depth gain of 0.1 levelled the floor in
the first runs). Surface level `s = z + depth_gain · w` with `depth_gain = 0.4` (0.25 still levelled the floor under a heavy shower).

## Processes (all from pre-step values)

| Process | Rule | Defaults |
| --- | --- | --- |
| Rain | `rain_c = rain_rate · max(0, B_c − rain_threshold)` per second, where `B_c` is the moisture *weather blob sum* at the cell (not the static habitat), so showers are the cores of the moving blobs and sweep the cube on the weather's 20–47 minute periods; static weather gives standing showers | `rain_rate = 0.6 d/s` (1.0 kept the whole soil floor under water), `rain_threshold = 0.35` |
| Flow | for each graph edge `(a, b)` with `s_a > s_b`: `q = flow · dt · (s_a − s_b)` from `a` to `b`, capped at `0.25 · w_a` per edge per substep (four edges, so depth never goes negative), substepped like nutrient diffusion; no flux across the open rim | `flow = 3.0 /s` |
| Evaporation | `evap · max(L_c, evap_floor) · w_c` per second (light-driven: the canopy dries fastest; the floor keeps the dark soil drying slowly so a moat cannot fill without bound) | `evap = 0.008 /s` (0.02 emptied a canopy pond in half a minute), `evap_floor = 0.5` |
| Wet growth | producer growth uses `W_eff = clamp(W + wet_gain · min(w, 1), W_min, 1)` | `wet_gain = 0.5` |
| Drowning | producer growth is multiplied by `max(0, 1 − (w − flood) / flood)` when `w > flood`, so a standing pool is bare water, not lawn | `flood = 1.5 d` |
| Wading | an organism's movement speed is divided by `1 + w` of its cell | none |
| Algae | producer growth sees `L_eff = max(L, algae_light · min(w / algae_depth, 1))`: standing water lights its own mat, so shallow pools on the dark floor grow food (drowning still bares deep water). The pool's light is a modeling choice booked as ordinary `light_in`; it is what gives the skimmer a food of its own | `algae_light = 0.5` (0.35 left the skimmer niche too thin), `algae_depth = 0.3 d` |

On a side face vertically adjacent cells differ by 0.125 in `z`, so a shower
runs down in roughly a second per cell and reads as streams; along the bottom
row and on the top face only `depth_gain · w` differences drive flow, so water
spreads into the hollows and stands. Pools therefore form as a broken line of
puddles along the soil floor and as ponds in the canopy's basins, both fed by
showers and lost slowly to the light.

## Render view and presentation (art mode)

The render view gains `water: Vec<f64>` (depth per cell) and `rain: Vec<f32>`
(this tick's rain rate per cell).

- **Water layer**, drawn after the ground and before motifs and bodies:
  color `mix(#1E9BF2, #42C5F8, min(w, 1))`, coverage `1 − exp(−w / 0.25)`
  source-over the ground, with the same seam-aware bilinear filter the
  producer ramp uses so pools are not cell blocks. A shimmer of ±8 %
  brightness on a 2.5 s cycle, phase from a pixel hash, driven by simulated
  time so a paused world holds still.
- **Rain**: in cells with `rain > 0`, one-pixel streaks of `#B8F0FF` at half
  opacity, count proportional to the rate, falling down the side faces at
  about 20 px/s and appearing as brief sparkles on the top face. Positions
  come from a hash of the tick and streak index; the sub-tick offset comes
  from the frame's interpolation fraction, so streaks move smoothly at 60 fps
  without extrapolating state.
- The M2 disc image is unchanged.

## Deferred

Water as a diet or a habitat for a swimming creature, infiltration into the
soil's stored moisture, freezing or drying cycles, and wave motion.
