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

Branch `tachyon-screen`, commits `b507843` (the crate, the `Scene` type, the
passes), `ca6d674` (the synthetic world, the scanout target, the golden image),
`419056e` (the fragment-coordinate fix §5 describes), `4c1a3a0` (GS-2's socket),
`ded6909` and `c675b5f` (two client bugs), `dcb1ea2` and `1311a8f` (this
report and the measured ladder). Everything is under `crates/cubarium-gpu/**`
plus one workspace member line and the lock entries it implies. The CPU
presenter, the cube path, `crates/cubarium/**` and `crates/cubarium-surface/**`
are untouched.

## 1. The headline

**640×360 at S = 2 runs at 60.3 fps on the panel with no visual concession, and
the renderer's own claim on the host's wall second is 142 ms.**
`presenter-budget-2026-09-16.md` §5 found that configuration unreachable on the
CPU — every lever stacked, including the unmeasured W3, still missed by 1.4–1.6×
unless the plant and tall sway were quantised to the tick rate, which was
"Wrysk's call, not FW-3's". The GPU does not raise that question: the sway runs
at 60 Hz, the GPU spends 3.68 ms of a 16.6 ms frame, and §5's best CPU stack for
the same rung needed ~756 ms of the wall second (its 767 ms load less the tick's
10.7) **with** the sway concession.

Two honest qualifications on that 142 ms. It is the renderer alone — the upload,
the instance buffer, the command buffer and the wait — measured with one scene
presented repeatedly. Building a scene on top of it costs another ~350 ms in
this package, but that is Stage A's synthetic producer walking 3,600 cells on one
thread, not the `RenderView` adapter Stage B will write, so adding the two would
be measuring the wrong thing. And the GPU time is not free to the *system* even
though it is free to the host loop: the Adreno is otherwise unused, which is the
whole point of the stack, but a future shader effect spends the same 13 ms of
headroom.

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
   S = 1 — and never runs at panel resolution, where only `present.frag` does. For
   scale: the whole world raster, both full-screen passes and every instanced
   quad, costs 0.88 ms at S = 1 against the present pass's own 0.74.

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
| 640×360 S = 2, ×3, live scene | 3.77 ms | 5.70 | 4.86 | **59.9** | 0.481 |
| 640×360 S = 2, ×3, scene held | 3.68 ms | 5.66 | 10.96 | **60.3** | **0.142** |
| 960×540 S = 3, ×2, scene held | 6.84 ms | 9.00 | 7.59 | **60.3** | **0.150** |
| 1920×1080 S = 6, ×1, scene held | 22.96 ms | 25.20 | 7.92 | **30.2** | 0.074 |

The world raster alone, headless, no present pass and no panel — 900 frames at
every rung of `flat-world-plan` §6's ladder:

| raster | S | upscale | scene passes, GPU p50 | p95 |
|---|---|---|---|---|
| 320×180 | 1 | ×6 | **0.88 ms** | 0.88 |
| 640×360 | 2 | ×3 | **2.90 ms** | 2.90 |
| 960×540 | 3 | ×2 | **6.02 ms** | 6.03 |
| 1920×1080 | 6 | ×1 | **12.60 ms** | 15.38 (max 21.96) |

The present pass, by difference between the two tables:

| rung | scene passes | scene + present | present pass |
|---|---|---|---|
| 320×180 S = 1 | 0.88 | 1.62 | **0.74 ms** |
| 640×360 S = 2 | 2.90 | 3.68 | **0.78 ms** |
| 960×540 S = 3 | 6.02 | 6.84 | **0.82 ms** |
| 1920×1080 S = 6 | 12.60 | 22.96 | **10.36 ms** |

**The present pass is memory-bound, and that is what ends the ladder.** It writes
the same 2.07 Mpixel at every rung, so its cost should be flat — and it is, at
0.74–0.82 ms, right up to S = 3. At S = 6 it jumps thirteen-fold. The difference
is what it *reads*: the world raster is 230 KB at S = 1 and 2.0 MB at S = 3,
both of which the Adreno's caches absorb while a `k × k` block of output reads
one texel; at S = 6 it is 8.3 MB read essentially once per output pixel. The
scene passes themselves scale about as the plan predicts (0.88 → 2.90 → 6.02 for
1×, 4×, 9× the area — sublinear, because the instance count does not change with
`S` and a good third of the work is per-instance). It is the *upscale* that
stops being free, and it stops being free exactly where the upscale stops being
an upscale.

Against the shared-loop budget `20 · tick_ms + fps · render_ms ≤ 1000` that
`flat-world-plan` §6 and FW-P §5 are written against: the GPU path's claim on
the host's wall second is **157 ms at S = 1, 142 ms at S = 2 and 150 ms at
S = 3** — flat, because what the host does per frame is write 4,000 instances and
a command buffer whatever the raster is. The CPU stacks §5 priced needed 520 ms
(S = 1) and 767 ms (S = 2), and the S = 2 figure was the one that required
quantising the sway to 20 Hz.

### How far the ladder goes — all four rungs, measured on the panel

| rung | GPU per frame | share of a 16.6 ms refresh | sustained |
|---|---|---|---|
| 320×180 S = 1, ×6 | 1.62 ms | 10 % | **60.3 fps** |
| 640×360 S = 2, ×3 | 3.68 ms | 22 % | **60.3 fps** |
| **960×540 S = 3, ×2** | 6.84 ms | 41 % | **60.3 fps** |
| 1920×1080 S = 6, ×1 | 22.96 ms | 138 % | 30.2 fps |

**960×540 at 60 fps works.** `flat-world-plan` §6 gives the CPU `R ≤ 1.4 ms` for
that rung with the presenter split over all four A78s, which nothing in FW-P's
stack comes near — §5's very best number, with every lever including the
unmeasured W3 and the sway concession, is 3.16 ms for a *cube* frame, and the
ring multiplies by `2.81 · S²` = 25.3. On the GPU it is 41 % of a frame and
0.150 CPU core-seconds per second, and it is not a derivation: it ran for a
minute on the panel at 60.3 fps.

**1920×1080 at S = 6 is a 30 fps configuration, not a 60 fps one**, and the
reason is the present pass rather than the world — see the table above. It is
not obviously worth rescuing: at S = 6 the panel shows one world pixel per
device pixel, which is the one rung where the pixel-art grid stops being visible
at all, and §6 lists it for completeness rather than as a candidate. If it were
wanted, the fix is not more GPU: it is to skip the present pass entirely by
rendering the world raster *into* the scanout image at panel resolution, which
`RingLayout { w: 1080, h: 1920, scale: 6 }` plus the quarter turn moved into the
scene pass would do — a different renderer, not a tuning.

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

**Corrections to make if this report is read alongside the session log.** Two
"the display is busy" failures during the measurements were **mine, not a
neighbour's**. The daemon decides its one-client rule at `accept`
(`handoff/server.rs`, `if client.is_some()` on the newly accepted connection),
not at `Attach`; the client held a second socket open purely to fill in a struct
field before the real connect, and that placeholder *was* the attached client, so
the real connection refused itself forty times and the log read exactly like
contention with GS-2. Fixed, and the shim module now says so where someone would
next be tempted to open a second socket.

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
its own fence before presenting. Every rung that reaches 60 fps does so with
7–13 ms of slack in `Present..Presented`, so overlapping the render with the
flip buys nothing at S = 1, 2 or 3. It would not rescue S = 6 either: there the
GPU alone is 23 ms against a 16.6 ms refresh, and the cure named above is to
delete the present pass rather than to hide it.
