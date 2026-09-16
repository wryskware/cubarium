---
design_status: exploration
last_reviewed: 2026-09-16
---

# GS-1 result: `cubarium-gpu`, the ring world on the Adreno

Stage A of the brief `design/handoffs/gs1-opus-vulkan-renderer-2026-09-16.md`:
a new crate that renders a ring-world `Scene` with the CPU presenter's look,
into a desktop readback and onto the Tachyon's panel. Stage B — the adapter
from the world's `RenderView` — is **not** here and is not blocked by anything
in it.

Branch `tachyon-screen`, commits `b507843`, `ca6d674`, `419056e`, `4c1a3a0`,
`ded6909`. The CPU presenter, the cube path and `crates/cubarium/**` are
untouched.

## 1. The headline

**640×360 at S = 2 runs at 60 fps on the board with no visual concession and
0.14 CPU core-seconds per second.** `presenter-budget-2026-09-16.md` §5 found
that configuration unreachable on the CPU — every lever stacked, including the
unmeasured W3, still missed by 1.4–1.6× unless the plant and tall sway were
quantised to the tick rate, which was "Wrysk's call, not FW-3's". The GPU does
not raise that question: the sway runs at 60 Hz, the GPU spends 3.68 ms of a
16.6 ms frame, and the CPU spends 142 ms of its wall second instead of the
767 ms §5's best CPU stack needed.

## 2. The `Scene` type — the interface Stage B adapts to

`crates/cubarium-gpu/src/scene.rs`. The split is the point: the six scalar
fields move at the **tick** rate and the instances at the **frame** rate, and
they travel by different routes, so an adapter cannot accidentally make a 20 Hz
thing cost 60 Hz.

```rust
pub struct RingLayout { pub w: u32, pub h: u32, pub scale: u32 }   // raster px, and S
    // w, h divisible by 4·S;  cells_x() = w/(4S) = 80, cells_y() = 45 at every rung
    // RING_320 = 320×180 S=1,  RING_640 = 640×360 S=2

pub struct Fields {              // one f32 per cell, row-major, cells_x·cells_y long
    pub producer: Vec<f32>,      // the ramp saturates at producer_max · 0.6
    pub water: Vec<f32>,         // depth d
    pub detritus: Vec<f32>,      // litter + remains, already × SOIL_SCALE
    pub growth: Vec<f32>,        // continuous stage, < 0 for bare
    pub tall: Vec<f32>,          // column height in trunk segments
    pub rain: Vec<f32>,
    pub producer_max: f32,
    pub revision: u64,           // re-uploaded only when this changes
}

#[repr(C)] pub struct SpriteInstance {   // 88 bytes, Pod, *is* the vertex buffer
    pub anchor: [f32; 2],        // the pivot, in raster px; snapped to a whole pixel
    pub heading: [f32; 2],       // the tile's +x in raster space; +y is (−h.y, h.x)
    pub frame0: [u16; 4],        // atlas rect x, y, w, h
    pub frame1: [u16; 2],        // the second frame's origin (same size)
    pub pivot: [u16; 2],         // source texels
    pub mix: f32,                // Pose::mix, 0 = frame0 exactly
    pub opacity: f32,
    pub bend_amplitude: f32,     // Bend, in source texels along tile +x
    pub bend_base: f32,
    pub bend_root: f32,
    pub bend_length: f32,
    pub mask_floor: f32,         // Mask::Axial / Mask::Strip, source rows
    pub mask_reveal: f32,        //   NO_MASK_FLOOR / NO_MASK_REVEAL disable it
    pub tone_colour: [f32; 3],   // Tone, linear light
    pub tone_shade_floor: f32,
    pub tone_shade_reference: f32,
    pub tone_mix: f32,           // 0 is the untoned stamp, bit for bit
}

pub enum Layer { GroundCover, Plants, Tall, Rain, Bodies }   // drawn in this order

pub struct Scene {
    pub layout: RingLayout,
    pub fields: Fields,
    pub layers: [Vec<SpriteInstance>; 5],
    pub seconds: f64,   // (tick − 1 + f) · DT, exactly art_present::present_seconds
    pub tick: u64,
    pub f: f32,
}
impl Scene {
    pub fn push(&mut self, layer: Layer, instance: SpriteInstance);  // wraps the seam
    pub fn clear_instances(&mut self);
}
```

Three things worth naming.

**The seam is `Scene::push`, and nothing else.** A stamp whose footprint reaches
past `x = 0` or `x = w` is pushed a second time displaced by one circumference.
On the cube that job needed `unfold_pixels_general` for 44 % of plant slots at
4× the cost (FW-P W2a); here it is one comparison and a few dozen extra quads a
frame. There is no wrap logic in any shader, and the ring wrap in the field
textures is one modulo in `cellValue`.

**`scale` is an integer, unlike FW-1's `Scale`.** FW-1 froze `Scale` as an
`f64`, so the contract admits `S = 1.5`. This renderer refuses it: the
pixel-art rule is that one authored source texel covers an exact `S × S` block
of raster pixels, and a half-integer factor has no such block. §6's rungs 1, 2,
3 and 6 are all integers; the 480×270 rung would have to be re-baked or drawn by
the CPU presenter. Everything else matches FW-1's freeze: cell = `4·S` source
px, and `Topology::height(p) = 1 − 2v/h` is the only vertical coordinate the
shaders read.

**One two-frame pose per instance, not a weighted stack.** `art_present` mixes
up to *three* poses in one stamp (a growth step's `[stage_lower, clip, stage_upper]`,
a body's state cross-fade) and composites `Σ wᵢ · sampleᵢ` **then** one
source-over. An adapter that emits three instances instead gets three successive
source-overs, which differs wherever a texel is partially transparent. Measured
on `assets/atelier`: **0.83 %** of atlas texels have `0 < α < 255` (82.0 % are
clear, 17.1 % fully opaque). So the difference is confined to those texels, and
only while a fade is in flight. **This is Stage B's decision to make**, and the
cheap exact fix if it matters is a third and fourth frame slot in
`SpriteInstance` — six `texelFetch`es instead of two, paid only by instances that
need them.

## 3. The pipelines, against FW-P §2

| FW-P pass | here | pipeline |
|---|---|---|
| 1–4 clear, floor, producer ramp + horizon fade, detritus flecks, soil ground | **one** full-screen shader over the cell textures | `background.frag` |
| 5 ground cover (the 8-texel lattice) | instanced quads | `sprite.vert/frag`, `Layer::GroundCover` |
| 6 water with shimmer and algae tint | full-screen, source-over | `water.frag` |
| 7, 8 plants and the soil snag | instanced quads | `Layer::Plants` |
| 9 tall columns (base, trunk strips, crown) | instanced quads | `Layer::Tall` |
| 10 rain | instanced quads over one white atlas texel | `Layer::Rain` |
| 11, 12 bodies, hunters | instanced quads | `Layer::Bodies` |
| `srgb_encode` (W7, 1.37 ms board) | the `_SRGB` attachment | — |
| — | nearest ×k upscale + quarter turn, at panel resolution | `present.frag` |

Four pipelines for the world raster plus one per target format. Shaders are GLSL
with SPIR-V checked in beside them (`shaders/compile.sh` regenerates them from
the desktop's `glslc`): `shaderc` wants cmake and a C++ toolchain the board does
not have, which the brief allowed for.

**Two deviations, both deliberate.**

1. *The water is its own full-screen pass, not a term in the background shader.*
   The brief asked for one pass including the water, but `art_present::
   draw_with_fruit` draws the ground-cover lattice **between** the soil and the
   water (lines 1069–1101 and 1103–08), so folding them would put the lattice on
   top of the pools. The extra pass covers the **world raster** — 57,600 pixels at
   S = 1 — and never runs at panel resolution. Measured cost below: the whole
   background half is under a millisecond.

2. *Sampling is nearest with the bend rounded to a whole source texel, where the
   CPU samples bilinearly.* At S = 1 with an integer anchor and an axis-aligned
   heading the two are **identical** — `Sprite::sample` lands exactly on a texel
   centre, so its four bilinear weights are 1, 0, 0, 0. At S > 1 they differ and
   the GPU is the crisper: the CPU's `local = d/scale` puts raster pixel centres
   at quarter-integers and blurs across the block, while `floor(local + pivot)`
   keeps the block whole. Rounding the bend is what keeps it whole: the
   displacement varies per *row*, so a stalk leans as a staircase of whole texels
   rather than popping as a unit, but without the rounding the two sub-columns of
   an S = 2 block would cross their thresholds at different amplitudes and split.

## 4. The measurements, on the board

`root@192.168.68.68`, Adreno 643, `taskset -c 4-7`, release, 3,600 frames with
the first five discarded, presented through GS-2's shim socket onto DP-1 at
1080×1920@60.37. `--hold` presents one built scene every frame, which separates
the renderer's own CPU cost from the Stage A scene producer's — Stage B replaces
the latter wholesale, so one number for both would mislead.

| configuration | GPU p50 | submit..fence | Present..Presented | fps over 60 s | CPU core-s/s |
|---|---|---|---|---|---|
| 320×180 S = 1, ×6, live scene | 1.65 ms | 3.85 | 6.64 | **60.2** | 0.505 |
| 320×180 S = 1, ×6, scene held | 1.62 ms | 3.75 | 12.83 | **60.3** | **0.157** |
| 640×360 S = 2, ×3, scene held | 3.68 ms | 5.66 | 10.96 | **60.3** | **0.142** |

The world raster alone, headless, no present pass and no panel — 900 frames at
every rung of `flat-world-plan` §6's ladder:

| raster | S | upscale | scene passes, GPU p50 | p95 |
|---|---|---|---|---|
| 320×180 | 1 | ×6 | **0.88 ms** | 0.88 |
| 640×360 | 2 | ×3 | **2.90 ms** | 2.90 |
| 960×540 | 3 | ×2 | **6.02 ms** | 6.03 |
| 1920×1080 | 6 | ×1 | **12.60 ms** | 15.38 (max 21.96) |

The present pass, by difference at the two rungs measured both ways:
`1.62 − 0.88 = 0.74 ms` and `3.68 − 2.90 = 0.78 ms`. It costs the same either
way, as it must — it is 2.07 Mpixel of nearest `texelFetch` whatever the source
is — so **+0.8 ms** is a safe figure to add to any row above.

The scene passes go 0.88 → 2.90 ms for 4× the raster area, and 2.90 → 6.02 for
2.25× more: sublinear, because the instance count does not change with `S` and
roughly a third of the work is per-instance rather than per-pixel.

Against the shared-loop budget `20 · tick_ms + fps · render_ms ≤ 1000` that
`flat-world-plan` §6 and FW-P §5 are written against: the GPU path's claim on
the host's wall second is **157 ms at S = 1 and 142 ms at S = 2**, against the
520 ms (S = 1) and 767 ms (S = 2) the best CPU stacks needed — and the S = 2 CPU
figure was the one that required quantising the sway to 20 Hz.

### How far it goes

Adding the present pass's 0.8 ms to the table above and comparing with the
16.6 ms a 60.37 Hz refresh allows:

| rung | scene + present | share of a frame | 60 fps? |
|---|---|---|---|
| 320×180 S = 1 | 1.62 ms (measured) | 10 % | yes, measured |
| 640×360 S = 2 | 3.68 ms (measured) | 22 % | yes, measured |
| 960×540 S = 3 | ~6.8 ms (derived) | 41 % | yes, on this evidence |
| 1920×1080 S = 6 | ~13.4 ms p50, ~16.2 p95 (derived) | 81–98 % | **no** — the p95 is at the refresh and the max is over it |

`flat-world-plan` §6 gives the CPU `R ≤ 1.4 ms` at S = 3 with the presenter on
all four A78s, which nothing in FW-P's stack comes near. **On the GPU, 960×540 at
60 fps looks reachable and is the first thing to measure once the display is
free**; the S = 3 and S = 6 rows above are the scene passes measured headless plus
a present pass measured at two other rungs, not a panel run, and they are marked
derived for that reason. S = 6 is where the renderer would have to stop waiting
on its own fence before flipping — see §7.

### The daemon's pacing

`Present..Presented` is 6.6 ms with the live scene and 12.8 ms with it held —
the same 16.6 ms frame either way. The frame arrives earlier when the CPU has
less to do, so it waits longer for the vsync; nothing is lost. Neither run
dropped below 60.2 fps over a minute.

## 5. The golden image, and board-versus-desktop agreement

`crates/cubarium-gpu/tests/golden.rs`, four tests, run on the desktop:

* the synthetic ring at a fixed instant (7.5 s) at 320×180 and again at 640×360,
  against stored PNGs;
* the seam column stepping no harder than the worst interior column;
* **the pixel-art rule, on the image rather than argued from the shader**: one
  stamp of one frame on an empty raster paints exactly the same seven colours at
  S = 1 and S = 2, and every one of them covers exactly **four times** as many
  pixels at S = 2. Nearest sampling invents no colour and every source texel is a
  whole `S × S` block.

**The tolerance is measured, not guessed.** Rendering the same scene on the
Adreno 643 and on the desktop's RTX 5090 gives mean |Δ| **0.116** per channel,
worst 23, with **0.05 %** of pixels off by more than 8 at 320×180 and 0.02 % at
640×360; 11 % of channels are off by exactly one, which is the last bit of the
sRGB encode and is all that should ever differ. The test bounds are four times
that.

**Getting there found a real bug.** The first comparison disagreed by mean |Δ|
1.10 with 3.5 % of pixels off by more than 8 and a worst of 218 — single wrong
*texels* scattered through every rotated sprite. A nearest-neighbour sampler
turns a half-ulp of interpolation error at a texel boundary into a whole wrong
texel, and the tile coordinate was an interpolated varying. It is now computed
in the fragment from `gl_FragCoord` and the instance's own snapped anchor and
heading — two dots and a divide on values both implementations hold identically
(`419056e`). That is a ten-fold improvement in agreement and it is also simply
more correct: the coordinate no longer depends on the interpolator's precision.

## 6. What was verified where

| claim | where | how |
|---|---|---|
| the pack loads with every clip the presenter draws | desktop | `atlas::tests`, 3 tests over `assets/atelier` |
| a looping clip wraps last → first; a growth clip clamps | desktop | `atlas::tests`, against `art.rs`'s `Clip::sample` rule |
| the ladder divides into 80 × 45 cells at S = 1, 2, 3, 6 | desktop | `scene::tests` |
| a stamp reaching a rim is pushed twice, one in the middle once | desktop | `scene::tests` |
| the quarter turn maps the ring's corners onto the portrait panel | desktop | `render::tests`, all four corners |
| the half-float field conversion round-trips | desktop | `render::tests` |
| the shim request bytes are the daemon's 32 | desktop | `shim::tests`, 3 tests |
| the picture at a fixed instant, at both scales | desktop | golden PNGs, bit-exact on the RTX 5090 |
| every source texel is a whole S × S block | desktop | colour histogram, exactly 4× at S = 2 |
| the seam carries as much light as any interior column | desktop | golden test |
| the same scene renders the same on the Adreno | **board** | mean \|Δ\| 0.116 against the desktop goldens |
| linear dma-buf export, `Attach`, `Present`, 60 fps | **board** | 3,600-frame runs through the shim socket |
| the sRGB encode is the attachment's, not the shader's | **board** | the blob accepts `MUTABLE_FORMAT` + a `B8G8R8A8_SRGB` view; the shader path stays as the runtime fallback |
| **what the panel is reading is the frame that was drawn** | **board** | the presented dma-buf pulled back through the GPU: 1080×1920, the ring upscaled ×6 and quarter-turned, 100 % non-black. `captures/gs1/panel-1080x1920-from-320x180-s1.png` |
| a window on the desktop shows the same passes | desktop | `crates/cubarium-gpu/window`, `winit` + `ash` swapchain, 960×540 at ×3, `R8G8B8A8_SRGB`, sRGB by the attachment |

No camera was available, so "visible on the panel" rests on the daemon's
`Presented` replies plus the verified contents of the buffer it is scanning out —
the same standard GS-0 set.

Screenshots (not in git; `/captures/` is ignored):

* `captures/gs1/synthetic-320x180-s1.png` and `-640x360-s2.png` — the world
  raster, which are also the committed golden fixtures under
  `crates/cubarium-gpu/tests/golden/`
* `captures/gs1/synthetic-320x180-s1-x3-for-viewing.png` — the same, blown up 3×
  with nearest neighbour, for looking at
* `captures/gs1/panel-1080x1920-from-320x180-s1.png` and
  `-from-640x360-s2.png` — read back out of the dma-buf the daemon presented

## 7. What is left

**For Stage B (the `RenderView` adapter).**

* *Multi-layer stamps.* §2 above: one two-frame pose per instance against
  `art_present`'s three-pose composite, and the 0.83 % of atlas texels where the
  difference can show.
* *Hunters.* `Layer::Bodies` takes whole-tile stamps. The Lanternjaw is a
  multi-part rig (`lanternjaw::draw_living`) and the adapter must decompose it
  into one `SpriteInstance` per part, each with its own anchor and heading. The
  interface supports that; nothing has exercised it.
* *`Mask::Radial` is not implemented.* It exists for top-face plants opening from
  their centre, which the ring has no analogue of. `Mask::Axial` and
  `Mask::Strip` are both there, in one formula.
* *Band thresholds on a ring.* `synthetic.rs` invents a canopy line at
  `h ≥ 0.55`, because `habitat::band_of_height` calls the canopy `h ≥ 1` — the
  cube's top face, which on a ring is one pixel row. The plan's §5 stratification
  decides this, not the renderer.

**For a viewing session.**

* *The bend budgets are a cube constraint and should be re-measured.*
  `art_present::wind` caps the shipped pack's amplitudes at 0.3–1.3 source texels
  through `Sprite::bend_headroom`, which measures against `FOOTPRINT_PIXELS = 9`
  — the radius `unfold_pixels` must fit a stamp inside when it crosses a seam. On
  the ring, FW-1's `max_local_radius` is `min(h, w − 8)/2` = **90 px** at
  320×180, and the GPU quad simply grows with the amplitude. Quantised to whole
  texels, a 0.3 px budget is *no visible breeze at all*, so the synthetic scene
  asks for 2.5 texels on plants and 5 on columns. Whether the ring wants the
  cube's restraint or the room it actually has is Wrysk's call.
* *The quarter-turn direction.* `--quarter-turns 1` puts the ring's bottom-left at
  the panel's top-left. `3` is the other way. Which one is right depends on how
  the panel is mounted and nobody has looked at it yet.

**Not attempted, and why.** The renderer holds one frame in flight: it waits on
its fence before presenting. The spike observed that render and flip serialise
to exactly one refresh and still hit 60 Hz, which both these configurations do
with 10–13 ms of slack, so overlapping them buys nothing until `S` rises. At
S = 6 it would be the first thing to do.
