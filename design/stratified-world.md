---
design_status: leaning
last_reviewed: 2026-09-12
decision_refs: []
---

# The stratified cube: soil, foliage, canopy

Wrysk's direction (2026-09-12): a top-down field on every face is boring. Give
the cube biomes by height. The lower third of the side faces is soil, above
it a foliage layer where plants grow and land fauna roam, and the top face is
a canopy layer. This note records the design Fable chose to realize that
without abandoning the continuous-surface substrate, the closed material
accounting, or the shim contract. It is `leaning`, adopted for implementation
under Wrysk's task authorization; the ledger is unchanged.

## Why height, and what already exists

The habitat already knows height: `h = p.y` of the embedded cell center,
Top = 1, rim = −1, continuous across seams, and base light and moisture are
linear in it (`design/m2-world-spec.md` "Habitat and weather"). The bands are
therefore not new geometry. They are a stronger light gradient plus one new
transport process, and a presentation that shows the result as three places.

Bands, by `h` of a cell center:

| Band | Where | `h` |
| --- | --- | --- |
| Soil | lower third of the four side faces | `h < soil_top`, default `soil_top = −0.33` |
| Foliage | upper two thirds of the side faces | `soil_top ≤ h < 1` |
| Canopy | the whole top face | `h = 1` |

Bands are a consequence of two mechanisms, not a lookup table the simulation
consults. Nothing in the world reads `soil_top`; only the presenter does, to
draw the horizon.

## Mechanisms (both conservative)

1. **Light falls with depth.** Habitat defaults become `light_base = 0.45`,
   `light_height_gain = 0.55` (were 0.55 and 0.35), noise gain unchanged. At
   the rim `L₀ ≈ 0` before noise, at the soil top about 0.27, mid-foliage about
   0.7, canopy 1.0. Moisture still falls with height, so the canopy is bright
   and dry, the foliage bright and moist, the soil dark and wet. Producers
   therefore grow in foliage and canopy and hardly at all in soil; the initial
   field `P = 0.6 · L₀ · W₀ · P_max` starts the soil nearly bare on its own.

2. **Detritus falls.** Each tick a fraction `detritus_fall · dt` of a cell's
   `D`, with `De` in the same proportion, moves to its downhill neighbor: the
   graph neighbor whose center has the lowest `h`, if that is strictly lower
   than the cell's own. Top-face cells and the bottom row of the side faces
   have no downhill neighbor and keep their detritus. Default
   `detritus_fall = 0.02 /s`. A new world also starts with the litter such
   shedding would have left: `D = 1.2 · (1 − L₀)` per cell, fully charged, so
   the soil's scavengers have something to eat from the first minute while
   the canopy starts clean. Material and energy are transferred, never
   created; the closed-box invariant and the energy audit are unchanged.
   Dead bodies, feces and producer mortality anywhere on the sides therefore
   drift down over minutes and accumulate in the soil, where decomposition
   returns them to nutrient. Nutrient diffusion is isotropic, so the soil
   becomes the nutrient source and the foliage the sink; the gradient drives
   nutrient back up. Whether that return is fast enough to sustain the
   foliage is the tuning question of this slice, answered with short runs.

Fauna are unchanged in this slice: one genotype, grazing and scavenging.
Behavior sorts them anyway. Grazers follow producer gradients up into the
foliage and canopy; hungry ones that find the soil's edible detritus
scavenge there. Diet genes and rigs tied to diet are the next slice, once the
bands exist and read.

## Presentation (art mode only; the M2 disc image is untouched)

- **Soil** is drawn as ground, not as a lawn: the night floor plus a
  seam-filtered detritus ramp from dark plum to violet-mauve by `D`, no
  flecks, no producer ramp. Lichen motifs only, dim, where `D` is high.
- **Foliage** keeps the decided producer ramp and the rosette and fern motifs
  by producer density, as the live art mode draws them today.
- **Canopy** keeps the producer ramp and grows fern motifs at a lower
  threshold and higher opacity, so a healthy top reads as covered.
- The **horizon** between soil and foliage is a soft blend about two cells
  wide in `h`, so it is one horizontal line around the cube rather than a
  jagged cell edge.

Creature rigs, clips and the state mapping are unchanged. Bodies on the side
faces read as walking along a wall with the soil below them; bodies on the
canopy read top-down. The current three-quarter-view rigs survive both.

## What this slice must show

- A fresh world at 8× in the art viewer within a few simulated minutes: a
  dark soil band with detritus accumulating, a lit foliage band with growth
  and grazers, and a canopy on top.
- Short runs (two simulated hours, three seeds; one six-hour run) showing the
  population survives, producers persist in foliage and canopy, and the soil
  accumulates detritus and nutrient. These runs take seconds to a minute of
  wall time each. Long batches are deferred until the picture is right.

## Deferred

Diet genes and rig-by-diet, climbing cost or gravity on movement, canopy as a
distinct food, dormancy in the soil (M4), and the controller's turning
behavior, which is parked in a stash and becomes moot for a fauna redesigned
around these bands.
