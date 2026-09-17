# Voxel storyboard: pick tilt, scale and depth by looking

Disposable art study for Package C of
[the voxel first-wave briefs](../../../design/handoffs/voxel-first-wave-briefs-2026-09-16.md).
`storyboard.py` (PIL + numpy, `python3 storyboard.py`, ~4 s for all nine PNGs)
hand-authors one voxel strip in code and projects it the way the presenter is
meant to project it:

    sx = x * s
    sy = base - y * s - z * s * tan(tilt)

back to front, largest z first. There is no simulation and no `cubarium-voxel`
dependency. The voxels are the model, not the look: per column of each depth
slab only the top face of an exposed surface run and the -z-facing cut wall
below it are drawn. The crest is autotiled between integer neighbour heights in
x, and a one-voxel step in z is covered by a graded riser, so stair steps read
as slopes while real cliffs stay sharp. No per-voxel outlines, no per-voxel
noise, strata undulate per column so the beds do not read as ruled lines.

Palette from `design/appearance.md` and `crates/cubarium/src/present.rs`:
floor `#12093A`, producer ramp `#1E2798` → `#42C5F8` (only rich, wet patches go
cyan; ordinary standing crop stays a dim indigo), detritus `#510B6D`, warm
accent `#FF9B50` used for exactly one thing — the near creature's feeding core.
Vegetation is placed in a few large patches with negative space between them,
per the landscape review's visual finding that patch size, not instance count,
is what separates a real place from a scatter of sprites.

## The scene

One periodic strip, identical in every cell (only the camera and the grid
change): a broad ridge on the left, a terrace cut into its right flank with a
meandering stream on it, a spillway gorge the stream falls into, a hollow
holding a pool whose far shore rises out of the water, a glowcap patch on the
pool's right shore, an overhang with a shadowed recess on the next hill, a
second ridge that wraps through x = 0, and rock beds with a soil cap visible on
every cut face.

Sprites come from `assets/atelier/` per `pack.json`, scaled by
`px_per_voxel / 4` so a 16 px tile is always 4 voxels: one spiretree on the
terrace (`tall` base + 2 trunk + crown, 56 px tall at 4 px, 42 px at 3 px), a
glowcap patch on the pool's shore (`plants` rows 0/1/2, five depths), and two
creatures in the **same voxel column** at z = 1 and z = 0.72·depth (near
x 1520 in the 4 px cells) so the only thing separating them is depth.

## The matrix

`ground plane` is the total screen height of the visible top surface of a flat
area, `depth·px·tan(tilt)` — it is both how much of the ground plane you get and
what the depth costs in vertical screen space, since the strip already fills
1080 px and the depth stacks on top of that, eating sky.

| file | tilt | depth | px | world (voxels) | ground plane | band/slab | creature |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `tilt25_d16_px3.png` | 25° | 16 | 3 | 640 × 360 | 22 px | 1 px | 12 px |
| `tilt25_d16_px4.png` | 25° | 16 | 4 | 480 × 270 | 30 px | 2 px | 16 px |
| `tilt25_d32_px3.png` | 25° | 32 | 3 | 640 × 360 | 45 px | 1 px | 12 px |
| `tilt25_d32_px4.png` | 25° | 32 | 4 | 480 × 270 | 60 px | 2 px | 16 px |
| `tilt35_d16_px3.png` | 35° | 16 | 3 | 640 × 360 | 34 px | 2 px | 12 px |
| `tilt35_d16_px4.png` | 35° | 16 | 4 | 480 × 270 | 45 px | 3 px | 16 px |
| `tilt35_d32_px3.png` | 35° | 32 | 3 | 640 × 360 | 67 px | 2 px | 12 px |
| `tilt35_d32_px4.png` | 35° | 32 | 4 | 480 × 270 | 90 px | 3 px | 16 px |
| `zoom_pool_tilt30_d24_px4.png` | 30° | 24 | 4 | 480 × 270 | 55 px | 2 px | 16 px |

### What each cell shows

- **`tilt25_d16_px3`** — the worst corner. 22 px of ground plane across the whole
  strip, and a 1 px top-face band per slab, so flat ground is a hairline and the
  picture reads as a flat side-on cross-section, not a place. The pool is a
  ~20 px sliver with no readable surface: you cannot tell it has extent, only
  that there is a blue line. The overhang's recess collapses to a dark smudge.
  The two creatures overlap almost exactly — 22 px of total depth offset over
  15 slabs is barely more than one creature height. Cheapest on vertical space,
  but there is nothing to spend it on.
- **`tilt25_d16_px4`** — 30 px of ground plane. Sprites are full 16 px tiles and
  read, and the 2 px band means flat ground is a visible strip rather than a
  line, but the pool still reads as a wide puddle rather than a body of water
  and the two creatures separate by only ~30 px. The receding far shore of the
  pool is the only depth cue that survives.
- **`tilt25_d32_px3`** — 45 px of ground plane, from doubling the habitat rather
  than the camera. Depth now does something: the pool has a visible surface, the
  far shore stacks, the two creatures clearly separate. But the 1 px band at
  25°/3 px makes every slab a thin contour, so the surface reads as a
  topographic line drawing, and the extra 32 slabs are the most expensive way to
  buy that.
- **`tilt25_d32_px4`** — 60 px of ground plane, and the shallow tilt keeps the
  cut wall dominant, which is the most "terrarium cross-section" of the eight.
  The pool reads, the overhang reads, creature separation is unambiguous. This
  is the best of the 25° row and a reasonable fallback if the sky matters.
- **`tilt35_d16_px3`** — 34 px from tilt alone, at half the habitat depth of the
  32 cells. Flat ground reads, the pool has just enough surface to look like
  water, but with only 16 slabs and 3 px voxels the far shore is thin and the
  creatures separate by about two body lengths. Good value per voxel; visibly
  short on habitat.
- **`tilt35_d16_px4`** — 45 px, the same ground plane as `tilt25_d32_px3` for
  half the voxels, and it looks better doing it: the 3 px band per slab gives
  the surface body instead of contours. The pool reads as a pool. The overhang's
  recess and its cast shadow are legible. The cheapest cell that reads as a
  place.
- **`tilt35_d32_px3`** — 67 px of ground plane. A lot of ground and a lot of
  world (640 × 360 voxels), but the 3 px grid puts creatures at 12 px and the
  spiretree at 42 px, which is where the atlas art starts to lose its
  silhouette. The finer grid does make slopes smoother.
- **`tilt35_d32_px4`** — 90 px of ground plane, the most generous cell. The
  terrace reads as a plateau you could walk on, the pool's surface is an
  unmistakable body of water with a near edge and a far shore, the glowcap patch
  sits on a shore that recedes, the overhang casts a real shadow, and the two
  creatures separate by ~80 px on the same column. Costs 90 px of sky and the
  most simulated voxels of the eight.
- **`zoom_pool_tilt30_d24_px4`** — the middle of the matrix at 2×, over the
  gorge, the fall and the pool. This is the cell to judge pixel-level questions
  on: the water surface band, the ripple dashes, whether the falling sheet reads
  as water, whether the glowcaps separate from the substrate, and whether the
  autotiled crest reads as a slope rather than a staircase.

## Recommendation

**tilt 30°, depth 24, px_per_voxel 4** — i.e. the zoom cell, between
`tilt35_d16_px4` and `tilt35_d32_px4`.

`px_per_voxel 4` is not really a choice: at 3 px the creature tiles drop to
12 px and the atlas silhouettes stop reading, which is the one thing
`design/appearance.md` says must survive. Once px is 4, tilt is the cheap knob
and depth is the expensive one — 35°/16 buys the same ground plane as 25°/32 for
half the voxels — so spend on tilt first. But 35° at depth 32 tips the picture
toward a floor plan: the cut wall stops being the subject and 90 px of sky goes
away. 30° and depth 24 gives 55 px of ground plane, which is enough for the pool
to read as water with extent, for the far shore to stack, and for two creatures
on one column to separate by about three body lengths, while keeping the front
cross-section — the thing that makes this read as a habitat rather than a map —
as the dominant surface.

If depth 24 is awkward, `tilt35_d16_px4` is the frugal answer and
`tilt35_d32_px4` the lavish one; both read, and they bracket the recommendation.
