# Ring captures (FW-5)

Two frames of the CPU art presenter drawing a **ring** world, at the two rungs of the
16:9 ladder `design/flat-world-plan-2026-09-16.md` §6 names.

| file | world | cells | art |
|---|---|---|---|
| `ring_320x180.png` | `Ring { w: 320, h: 180 }`, `world_scale = 1` | 80 × 45 = 3,600 | pack v5 at its authored size |
| `ring_640x360.png` | `Ring { w: 640, h: 360 }`, `world_scale = 1` | 160 × 90 = 14,400 | the same, so the world is four times larger rather than twice the size |

Both are seed 1, `assets/atelier`, **tick 3,000**, drawn at `f = 0` and encoded through
`Canvas::encode_raster` and the sink's own PNG writer. A `run --sink png` capture is not
reproducible — `final.png` is whatever frame the render loop stopped on — so these are
rendered by the test that checks them:

```
cargo test --release -p cubarium --lib -- --ignored ring_goldens
CUBARIUM_WRITE_GOLDEN=1 cargo test --release -p cubarium --lib -- --ignored ring_goldens
```

the second of which re-records them.

## What is visible

Top to bottom, the three bands of `design/stratified-world.md` read as three places, now
as rows of one panel instead of the cube's five charts:

* **Canopy**, the top 7 rows of 45 (`h ≥ canopy_top = 0.67`): umbrellafrond and bloomcrown
  crowns with orange fruit, each at its own hashed angle, clipped by the top rim.
* **Foliage**, the 23 rows below it: lanternstalk and tendrilfan standing up the wall, with
  the tall columns — spiretree and glasscane, some with a vinecoil — rising out of the
  horizon and carrying their caps toward the canopy. Every column in the world is at the
  same segment count at this tick, so the crowns line up in one row; the cube has the same
  property and four faces to hide it in.
* **Soil**, the bottom 15 rows (`h < SOIL_TOP = −0.33`): the litter wash with glowcap and
  rootveil, and — at 320×180 — standing water along the bottom rows with reedspire in it.
  That is the **moat** `design/7_Research/flat-world-fw2-2026-09-16.md` §4 predicted: the
  ring's bottom row is a wall the cube's open rim never had, and `evap_floor` has not been
  re-tuned for it (a knob; `design/backlog.md` owns knobs).

The horizon between soil and foliage is the same per-pixel `w_soil` blend the cube uses,
which on a ring is one straight line across the panel.

Both images are continuous across the **wrap**: `u = 0` and `u = w` are the same place, so
rolling either file by half its width leaves no seam. The 16 columns either side of the
wrap carry no systematic brightness difference from the rest of the image.

At 640×360 the art is the same 16-pixel art on a world twice as wide, so the columns read
as thinner and the plants sparser. Drawing pack v5 at `scale = 2` (the plan's Stage A0) or
re-baking it at `TILE = 32` (FW-7) is what makes 640×360 read like 320×180 does.
