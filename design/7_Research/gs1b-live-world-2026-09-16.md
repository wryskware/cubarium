---
design_status: exploration
last_reviewed: 2026-09-16
---

# GS-1b result: the live ring world on the GPU

Stage B of the brief `design/handoffs/gs1b-opus-live-world-2026-09-16.md`. The
real world instead of the synthetic scene:

```
cubarium run --fresh --topology ring:640x360 --world-scale 2 \
             --sink gpu --art assets/atelier
```

runs on the Tachyon **at 59.9 fps over five minutes, as the unprivileged
`particle` user, through `cube-screen-shim`'s frame socket**, and on the desktop
the same flag opens a window. Branch `tachyon-screen`; the CPU presenter, the
cube and FW-5's files are untouched.

## 1. Where the adapter is, and why it is in two halves

The brief put "the adapter" in `cubarium-gpu`. Half of it is: `cubarium_gpu::
adapter` turns **one stamp into one instance** and its module doc carries the
mapping table. The other half — **which stamps a world wants** — is in
`crates/cubarium/src/sink/gpu.rs`, because it has to ask `art_present`, which
lives in `cubarium` and which `cubarium-gpu` cannot depend on without a
dependency cycle.

Transcribing `art_present` into `cubarium-gpu` was the alternative the brief
allowed for ("copy the logic into the adapter and cite the source line") and I
did not take it. FW-5 landed a complete, public, ring-aware `ArtGeometry`, so a
copy would have been ~2,900 lines that (a) diverge the first time FW-5 touches
the original, (b) make the fidelity test measure the transcription rather than
the renderer, and (c) need re-verifying against the CPU picture anyway. Two
`pub(super)` helpers genuinely had to be copied and both cite their source:
`tall::silhouette_layers` and `environment::band_ground_weight`.

What that buys is in §3: the two renderers **cannot** disagree about what to
draw, so every difference the comparison finds is a rasterisation difference.

### The mapping table

| CPU pass (`art_present::draw_with_fruit`) | layer | instance fields |
|---|---|---|
| 1–4 floor, producer ramp + horizon, detritus flecks, soil wash | — | `Fields.producer`, `.detritus` (= `litter_density · SOIL_SCALE`), one full-screen shader |
| 5 ground cover, `geom.ground_points` | `GroundCover` | anchor = `pixel_center`, heading `(1,0)`, one pose of `atlas.ground(band)` at `ground_phase_of`, opacity = `ground_opacity · band_ground_weight` |
| 6 water + shimmer + algae | — | `Fields.water`, `.producer`, one full-screen shader |
| 7 plants | `Plants` | anchor = `slot.at`, heading/bend = `geom.slot_wind`, poses = `stage_layers` (stage + fruit) or the authored `Grow(lo,hi)` clip at `growth_weights`, opacity = `stage_opacity`, mask = `Axial{gu·PLANT_REVEAL_PX}` or `Radial{gu·(extent+0.5)}`, tone = living wood at `1 − foliage_ramp_at` |
| 7 dead silhouette | `Plants` | `silhouette_layers`, tone = dead wood, `DEAD_WOOD_OPACITY` |
| 8 soil snag | `Plants` | stage 0, `Axial{SOIL_SNAG_PX}`, dead tone, `soil_snag_opacity` |
| 9 tall columns | `Tall` | base at `tall_anchor_at(0)`, trunk strips `Strip{trunk_strip(plant)}` per segment, crown at `height+1`, vine on the derived strips; one `tall_amplitude` for the whole column, `bend_base = 4i − 8` |
| 10 rain | `Rain` | `geom.rain_marks`, one white atlas texel, tone = `RAIN_SRGB`, `RAIN_OPACITY · rate · weight` |
| 11 bodies | `Bodies` | `interpolate_on` + `turn_heading`, poses = `BodyMemory::layers_at` × `mode_weight` plus the meal's feed clip, `scale = JUVENILE_SCALE` for a juvenile |
| 12 hunters | `Bodies` | `HunterMemory::living_pose` → `Lanternjaw::parts_living` → **one instance per part**, each through the scratch page |

`Fields.revision` is the world tick, so the six cell textures are uploaded once
per tick and not once per frame.

### The three trait methods

`FrameSink` gains `observe_world(view, hunters, events)` (per tick),
`observe_view(view, seconds, f)` (per frame) and `wants_pixels()`. The first two
are the presenter's own two-method contract offered to a sink; the third lets the
host skip the canvas entirely, which is not an optimisation but the point — a GPU
sink that still paid for the CPU rasterisation it threw away would be *slower*
than the CPU path. All three are defaulted and object-safe; `FanOutSink` forwards
them and wants pixels if any child does.

## 2. Four frame slots, and what they cost

`SpriteInstance` carries four `(frame, weight)` slots, 120 bytes. That is not an
approximation of `Σ wᵢ · poseᵢ`: a weighted sum of poses that are themselves
lerps is a weighted sum of the underlying frames, so four slots express **two
whole poses exactly** — an idle plant (one), a fruiting one (two), and an
authored growth step, which names three layers but never has three live at once
because `GROW_BLEND = 0.12 < ½`.

The case that exceeds it is a body that is **both cross-fading and feeding**:
two states × two frames plus the meal clip's two is six. `Stamp::instance` then
drops the lightest frames and renormalises, and counts it. Measured:

| run | population | slots dropped per frame | share of stamps |
|---|---|---|---|
| 320×180 S=1, 60 s | 24 | 0.083 | 0.003 % |
| 640×360 S=2, 60 s | 24 | 0.14 | 0.004 % |
| 640×360 S=2, **300 s** | 39 | **0.79** | 0.023 % |

It rises with population, as it must. The fix, if anyone ever sees it, is six
slots and two more `texelFetch`es; at 0.02 % of stamps, each losing its *lightest*
layer of a 0.3 s cross-fade on a 16-pixel body, nobody will.

## 3. Fidelity: the CPU presenter is the reference

`crates/cubarium/tests/gpu_fidelity.rs`, on the desktop. One `RenderView` (seed
20260916, 3,000 ticks, `ring:320x180`) rendered both ways and compared.

**With `--gpu-filter bilinear`** — the GPU using `Sprite::sample`'s own four taps
at the CPU's own un-snapped sub-pixel anchor, so that *nothing* is left between
the two renderers:

| configuration | mean \|Δ\| per channel | pixels off by > 8 | worst |
|---|---|---|---|
| 320×180 S=1, f = 0 | **0.501** | 0.53 % | 40 |
| 320×180 S=1, f = 0.5 | **0.501** | 0.51 % | 40 |
| 640×360 S=2, f = 0 | **0.499** | 0.54 % | 31 |
| 320×180 S=1, 40 ticks | 0.873 | 2.65 % | 129 |

That is the evidence that the adapter picks the same stamps, in the same order,
with the same clip phases, opacities, bends, masks and tones: if it did not, this
is where it would show, and it is the assertion the test makes. The young-world
row is the loosest and its residual is ~120 pixels in one cluster; it was not
chased further.

**In the default pixel-art mode** the same scene differs by **mean 16.0 per
channel, 56.9 % of pixels off by more than 8**. All of it is the sampler:

* `habitat::slot_of` jitters every plant's anchor by up to ±1 px, so the CPU
  bilinearly resamples every tile at a **random sub-pixel offset**. One frame
  contains **38,524 distinct colours** on the CPU against **16,127** on the GPU —
  the CPU is manufacturing two and a half times the palette.
* 55 % of the differing pixels have a near-match within one pixel on the CPU
  side: the same texel, half a pixel away.

**Which is right: the GPU.** `design/appearance.md` asks for pixel art on an LED
cube and a 1080×1920 panel that integer-upscales; a bilinear resample of a 16×16
authored tile at a random sub-pixel offset is exactly the case where a renderer
invents colours the palette does not contain. The CPU presenter's jitter was
meant to stop a row of stalks reading as a picket fence, and snapping it to a
whole pixel keeps that (the jitter is still −1, 0 or +1 px) while dropping the
blur. The bilinear mode exists to make the comparison possible, not as a quality
setting.

### Two divergences the comparison found, rather than arguments about

1. **Column heights between ticks.** `ArtPresenter` interpolates a column's
   height across the tick but exposes only the current value. The sink now keeps
   its own snapshot, taken immediately before the presenter advances (and *after*
   it on the first observe, because the presenter snaps rather than advances
   there — getting that backwards made every column grow from nothing over one
   tick, which the comparison caught at once: 0.50 → 3.23).
2. **The sprite tile does not scale with `S`.** `art_present` passes `scale = 1.0`
   to every stamp whatever the world's `S`, so a 16 px tile is 16 raster pixels at
   `S = 1` and at `S = 2` alike: `S` scales the cell grid and leaves the art the
   size it was authored. `design/flat-world-plan-2026-09-16.md` §6 says the
   opposite — "one factor that multiplies every length in the world — sprite
   tile, field cell, body extent". **The two have not been reconciled**, and this
   is not the renderer's call. `--gpu-art-scale` shows both; the default is the
   presenter's rule, so the reference stays the reference. See
   `captures/gs1b/art-scale-1-vs-2.png`: the same world at `S = 2` with 16-px art
   (top) and 32-px art (bottom). The synthetic scene Wrysk approved was the
   bottom one.

## 4. The board

`root@192.168.68.68`, Adreno 643, `taskset -c 4-7`, release, **run as
`particle`** (uid 5005, in `video`, which is the socket's group, and in
`render`). The daemon stayed up throughout; nothing took DRM master.

| rung | fps sustained | scene build | GPU | submit+present | CPU core-s/s | peak RSS |
|---|---|---|---|---|---|---|
| 320×180 S=1, 60 s | **59.8** | 4.68 ms | 2.76 ms | 5.10 ms | 0.617 | 49 MiB |
| **640×360 S=2, 300 s** | **59.9** | 5.46 ms | 3.78 ms | 6.04 ms | **0.668** | 57 MiB |
| 960×540 S=3, 60 s | 46.4 | 7.44 ms | 5.40 ms | 8.11 ms | 0.677 | 70 MiB |

"CPU core-seconds per second" is the **whole process** — simulation, adapter and
render — from `/proc/<pid>/stat`, over the whole run including start-up. The
`--fps` cap is 60, so 59.9 is the cap, not a ceiling.

Three things had to change to get there, and the first is the interesting one.

* **The vsync wait was on the critical path.** `ShimScanout::present` waited for
  the daemon's `Presented`, which the daemon sends once the flip it queued has
  *completed* — so the host blocked for a whole refresh while the GPU and the
  panel both had nothing to do. The replies are now drained in `take_free_slot`,
  which needs one anyway (`released` is the only signal a buffer is free), so with
  three slots the host waits only once it is genuinely a frame ahead. **34 → 59.5
  fps at S = 2**, with no other change.
* A `Stamp` carries `[PoseRef; 3]` rather than a `Vec`: a ring frame builds about
  four thousand stamps and an allocation each was the adapter's largest cost.
* The plant loop was re-deriving each cell's slot every frame, because
  `ArtGeometry::species_of` calls `slot_of` — seven `SplitMix64` draws and an
  `embed_tangent`. The slots are laid out once, as the presenter does.

Scene build went 7.29 → 5.40 ms at S = 2 and the fidelity numbers did not move
to three decimals, which is the check that none of it moved a pixel.

**960×540 at S = 3 does not reach 60 fps**, and the limit is the CPU rather than
the GPU: the scene build is 7.44 ms of a 16.6 ms frame while the GPU spends 5.40.
Stage A measured the *renderer* alone at that rung at 60.3 fps; what the live
world adds is the adapter's walk over 3,600 cells and 5,652 instances. The next
lever there is the one FW-3 already built for the CPU presenter — a row-band or
pass-level split across the A78s — not more GPU.

## 5. What Wrysk should look at on the panel

Run each for a minute and compare:

```
cubarium run --fresh --topology ring:640x360 --world-scale 2 --sink gpu --art atelier
cubarium run --fresh --topology ring:640x360 --world-scale 2 --sink gpu --art atelier \
             --gpu-art-scale 2
cubarium run --fresh --topology ring:640x360 --world-scale 2 --sink gpu --art atelier \
             --gpu-bend-substep
```

1. **The art scale** (§3's second divergence) is the big one and it is a design
   decision, not a bug: 16-px plants at `S = 2` give a world of many small things;
   32-px plants give the look the synthetic scene had. Both are one flag apart.
2. **The bend.** `--gpu-bend-substep` lets the wind's displacement land between
   source texels instead of rounding to a whole one. With the shipped pack's
   measured budgets (0.3–1.3 texels, which `art_present::wind` derives against the
   *cube's* nine-pixel footprint) the default is often no visible breeze at all;
   the substep mode leans smoothly and softens the leaning rows. Stage A's report
   argued the budgets themselves should be re-measured on the ring, where
   `max_local_radius` is 90 px rather than 9; that is still open and is the better
   fix if the breeze is wanted.
3. **The horizon and the canopy line.** `CANOPY_TOP = 0.67` is FW-5's constant and
   the GPU inherits it, so both renderers put the canopy in the same place. There
   is no config key for it yet; `sink/gpu.rs` carries a TODO naming `canopy_top`
   beside `topology` and `world_scale`.

Captures (local; `/captures/` is gitignored):
`captures/gs1b/board-live-640x360-s2.png` (a live frame off the board),
`art-scale-1-vs-2.png`, `live-320x180-s1-x3-for-viewing.png`,
`cpu-over-gpu-320x180.png` and `cpu-gpu-difference-x4.png`.

## 6. What is left

* **`Mask::Radial` is implemented**, contrary to Stage A's report, which called it
  top-face-only with no ring analogue. That was wrong: `ArtGeometry::is_radial`
  calls a cell radial when its band is Canopy, and a ring *has* a canopy band. The
  reveal measures to `extent + 0.5`, so the atlas now measures each frame's extent
  from its own alpha exactly as `Sprite::from_rgba` does.
* **Hunters are drawn but not exercised.** The rig is decomposed into one instance
  per part through a per-frame RGBA16F scratch page, in the premultiplied linear
  form `Sprite::texel` already holds. No world in this package had a hunter
  profile, so the path is written and compiles and has never drawn one. The one
  known difference is that `stamp_rig_scaled` composites the whole rig through a
  single query while this composites part over part; every Lanternjaw texel is
  opaque or clear, so the art cannot express the difference, but that is an
  argument and not a measurement.
* **The cube is refused by name.** `--sink gpu` draws one raster; a cube is five
  charts with seams and `unfold_pixels` has no analogue in an instanced quad.
* **The corner-cap handoff is not ported.** It is cube-seam machinery
  (`near_corner` is `cx < 2 || cx > 13`); a ring has no near-corner.
* **`--gpu-target window`** is `minifb`, not a Vulkan swapchain, and reads the
  raster back each frame. `winit` would put 110 packages into the shared lockfile
  for a window the board never opens, and `minifb` is already a dependency of
  `PreviewSink`. The board's target reads nothing back.
* **S = 3 needs the adapter split across cores** to reach 60 fps; see §4.
