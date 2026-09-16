---
design_status: leaning
last_reviewed: 2026-09-16
decision_refs: []
---

# A flat W×H world beside the cube world

Coupling audit and phased plan for the Tachyon panel, written by Opus on
2026-09-16 under
[the tachyon-screen plan's revision](handoffs/tachyon-screen-plan-2026-09-16.md).
Read-only audit: no source file was changed to write it.

## Summary

1. Cubarium is coupled to the cube through **four hubs**, not the whole tree:
   `SurfacePoint`/`Face` (the chart id), `CellId`+`CELL_COUNT` (the field grid),
   `Canvas` (five fixed 64×64 buffers), and `FrameSink::submit(&Frame)`.
2. Recommended abstraction: a `Copy` **enum** `cubarium_surface::Topology { Cube,
   Flat { w, h } }`, passed by value into the surface free functions and owned by
   `WorldConfig`, `FieldGraph` and `Canvas`. Not a trait, not a generic:
   `SurfacePoint`, `WorldState` and `Canvas` stay concrete, so serde, `Copy`,
   `Box<dyn FrameSink>` and all 1,591 existing tests survive.
3. `Face` stays the chart id; a flat world has exactly one chart, `Face::Front`.
   Two forced widenings: pixel indices `u8 → u16`, and `CELL_COUNT` from a const
   to a runtime count.
4. Flat geometry: one chart, four solid edges using the **same** specular
   reflection the open rim uses today, `TangentMap::IDENTITY` everywhere,
   `embed() = [u/32, v/32, 0]` so `chord_sq` becomes exact Euclidean.
5. **Recommended first flat world: 640×360, upscaled 3× by the daemon, at world
   scale S = 2** — sprite tile 32 px, field cell 8 px. Not 320×180: that is the
   cube world stretched, and it wastes the panel.
6. A second parameter does the real work: **`world_scale` S**, which multiplies
   every length (sprite tile, cell size, body extent, sense radius, px/s speed).
   Hold `cell = 4·S` and **every** raster from 320×180 to 1920×1080 has the same
   3,600 field cells and the same ecology; only the picture sharpens.
7. That gives one clean law: **render cost ∝ S², simulation cost ≈ constant.**
   S = 2 is 11.25× the cube's pixel work; S = 6 (native 1080p) is 101× and is not
   a first version.
8. Genuine higher-resolution art is **authoring, not a bake flag**: the 148
   `art/parts/*.svg` are pixel art drawn on integer grids (`bud.svg` is 4×4), so
   `svg/scale` only makes bigger blocks. Bake at S× first to unblock the code,
   redraw parts file-by-file afterwards — the runtime never notices.
9. Biome/terrain variation is real but **separable**: one low-frequency region
   field offsetting habitat parameters that are already per-cell, behind a
   toggle, off by default. The first flat world ships without it.
10. Total: **nine packages — 4 large, 4 medium, 1 small**; four at high reasoning
    effort. FW-2∥FW-3 and FW-4∥FW-5 pair off on disjoint crates; FW-7 (art) and
    FW-8 (biomes) follow and can slip without blocking the panel.

## 1. How deep the cube goes

Counts are matching *lines* from `rg -c` over `*.rs`, per crate/module.
`Face` is the pattern `\bFace` (so it includes `Face::ALL` and `FaceFrame`).

| module | `cube_proto` | `Face` | `FACE_SIZE` | `FACE_EXTENT` | `NUM_FACES` | `Canvas` | `SurfacePoint` | `CELL_COUNT` | `travel` | `unfold` |
|---|---|---|---|---|---|---|---|---|---|---|
| vendor/cube-proto/src | — | 80 | 20 | — | 5 | — | — | — | — | — |
| cubarium-surface/src | 7 | 156 | — | 36 | — | — | 98 | 14 | 39 | 63 |
| cubarium-surface/tests | 1 | 149 | — | 15 | — | — | 80 | 8 | 39 | 45 |
| cubarium-surface-oracle/src | 3 | 65 | — | — | — | — | — | — | — | 7 |
| cubarium-render/src | 12 | 114 | 17 | — | 5 | 83 | 62 | — | 18 | 27 |
| cubarium-render/tests | 6 | 197 | — | — | — | 90 | 138 | — | 2 | 21 |
| cubarium-core/src | 3 | 84 | — | 10 | — | — | 44 | 130 | 59 | 31 |
| cubarium-core/tests | — | 200 | — | 2 | — | — | 104 | 28 | 41 | 7 |
| cubarium/src | 21 | 223 | 38 | 6 | 1 | 97 | 71 | 38 | 47 | 19 |
| cubarium/tests | 34 | 377 | 59 | — | — | 261 | 164 | 186 | 35 | 7 |
| cubarium/examples | 8 | 57 | 10 | — | — | 19 | 36 | 26 | 14 | — |
| cubarium-search/src | — | 7 | — | — | — | — | — | — | 36 | — |

Other exhaustive counts: `FrameSink` 27 (host src) / 7 (tests) / 5 (examples);
`CellId::new` 245, `CellId::all` 124, `cell_of` 133, `ScalarField` 102 across the
workspace; 129 source lines containing a `0..64` or `0..FACE_SIZE` loop bound in
26 files; `Face::ALL` 54 in `cubarium/src`, 24 in `cubarium-render/src`.
`SurfacePoint::pixel()` is used on only **9** lines — the `u8` pixel index leaks
through `PixelImage`, not through points.

**The hubs.** Generalising these five covers nearly every call site:

| hub | file:line | why |
|---|---|---|
| `SurfacePoint` + `FACE_EXTENT` | `crates/cubarium-surface/src/point.rs:11`, `src/lib.rs:56` | every position in core, render and host |
| `CellId` / `CELL_COUNT` / `ScalarField` | `crates/cubarium-surface/src/field.rs:12-21,83,228` | every field, every habitat/water/ecology array (430 lines) |
| `Canvas` | `crates/cubarium-render/src/canvas.rs:9` | `Box<[[[f32;3]; 4096]; 5]>`; every presenter pass writes through `get/set/add` |
| `FrameSink::submit(&Frame)` | `crates/cubarium/src/sink/mod.rs:22` | all five sinks, one `Box<dyn FrameSink>` fan-out |
| `unfold_pixels` / `PixelImage` | `crates/cubarium-surface/src/raster.rs:10,36` | all sprite, body, care and motif stamping |

Two whole subsystems are cube-only and should stay so: `cubarium-surface-oracle`
(the rigid-rotation 3D reference; a plane needs no oracle) and `cubarium-search`
(training fixtures place cells by `Face`, `crates/cubarium-search/src/es/fixture.rs:153,205`).

The web viewer (`crates/cubarium/src/sink/web/index.html`, 890 lines) embeds
`FACE_SIZE = 64`, `NUM_FACES = 5` and a copy of `face_frame` at lines 173-184 and
draws a three.js cube plus the 768×384 net canvas (line 103). The PNG sink and
the preview window both go through `crates/cubarium/src/net.rs`, the 4×2 net
layout. The ray-cast preview (`crates/cubarium/src/raycast.rs`, 299 lines) is
cube-only by construction.

## 2. The surface contract, and the smallest generalization

Today (`crates/cubarium-surface/src/`): `SurfacePoint { face, u, v }` with
`u,v ∈ [0,64)`; `FaceFrame`/`face_frame` giving the `[-1,1]^3` embedding;
`travel/travel_into` sweeping a displacement across seams with specular
reflection at the open bottom rim and a lowest-`Edge` vertex tie rule
(`travel.rs:201-283`); `unfold`/`chart_images`/`segment_is_valid` for the
shortest valid unfolding within `MAX_LOCAL_RADIUS = 32`, `MAX_SEAMS = 2`;
`unfold_pixels` returning every pixel within a radius exactly once
(`raster.rs:36`); `FieldGraph` at 16×16 cells per face, 4 px per cell, 1,280
cells, 2,528 undirected edges (`field.rs:83-158`); `Vec2`/`TangentMap` for
transported directions.

**The generalization: an enum, by value.**

```rust
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Topology { Cube, Flat { w: u16, h: u16 } }
```

with `charts()`, `extent(face) -> (f64, f64)`, `cells(face) -> (u16, u16)`,
`cell_count()`, `embed(&SurfacePoint)`, `height(&SurfacePoint)`. Free functions
gain it as a first parameter: `travel(topo, ..)`, `unfold(topo, ..)`,
`unfold_pixels(topo, ..)`, `cell_of(topo, p)`, `pixel_center(topo, ..)`,
`pixel_neighbor(topo, ..)`. Every `Topology::Cube` arm is today's body verbatim.

Why an enum and not the alternatives:

- **Generic parameter** (`SurfacePoint<T>`, `World<T>`, `Canvas<T>`): the largest
  possible diff. It reaches `WorldState`'s serde derives, `RenderView`, every
  presenter, and the signature of roughly 1,591 tests. Rejected.
- **Trait object** (`&dyn Topology`): virtual calls inside per-pixel loops
  (`draw_field` calls `pixel_neighbor` four times per pixel,
  `cubarium-render/src/field.rs:28`), and `ScalarField`/`FieldGraph` would have to
  carry a `dyn` reference through serde and `Clone`. Rejected.
- **Enum, by value**: 2 bytes + tag, `Copy`, `PartialEq`, serde-derivable, static
  dispatch, matched once per call and hoisted out of inner loops. The set of
  topologies is closed and lives in one crate — open extension is not wanted.

**The `Flat { w, h }` arm.** One chart (`Face::Front`); `extent = (w, h)`;
`travel` keeps its sweep loop but its `earliest_exit` sees four *closed* edges and
applies the rim's existing specular reflection to all four, so `MAX_CROSSINGS`,
the fallback counters and the "never tunnels" property carry over unchanged;
`TangentMap` is always `IDENTITY`, so nothing rotates; `chart_images` returns one
direct image, so `unfold` collapses to `segment length` and `unfold_pixels`
always takes the direct fast path clipped to `0..w`/`0..h`. `embed()` becomes
`[u/32, v/32, 0]`, which keeps `chord_sq`'s `×1024` factor exact — on a plane the
chord bound is the true distance, so pair rejection stops being conservative and
becomes exact (`point.rs:111`). `MAX_LOCAL_RADIUS = 32` still bounds sensing (12 px).

**Two forced widenings.**

- `u8 → u16` for pixel indices. 320 > 255, so `SurfacePoint::pixel_center`,
  `pixel()`, `pixel_neighbor`, `PixelImage { x, y }`, `Canvas::{get,set,add}` and
  the 129 `0..64u8` loop bounds must widen. Purely mechanical, zero behaviour
  change on the cube, and the compiler finds every site.
- `CELL_COUNT` from a const to a runtime count. `1,280 = 2^8·5` cannot be
  factored 16:9 with square cells, so no flat raster reproduces it.
  `ScalarField.values` becomes `Box<[f64]>` sized at construction and `CellId`
  becomes a plain index decoded through the topology. **Mitigation that saves
  ~240 of the 430 sites:** keep `pub const CUBE_CELL_COUNT: usize = 1280` for the
  cube-only tests and fixtures (186 lines in `cubarium/tests`, 28 in
  `cubarium-core/tests`, 26 in examples) and convert only production code.

**Cell size for a flat world: stays 4 px.** 320×180 → 80×45 = 3,600 cells, all
interior cells degree 4, the four border rows degree 3, no seam edges, `downhill`
unchanged. `w` and `h` must be multiples of `CELL_PIXELS`; 320×180 and the
fallback 192×108 both are, and both divide 1920×1080 by an integer (6× and 10×).

## 3. Render and host

| piece | cube today | flat |
|---|---|---|
| `Canvas` | `Box<[[[f32;3]; 4096]; 5]>` | `Canvas::new(topo)`, flat `Vec<[f32;3]>` of `w·h`, same `get/set/add` on `(Face::Front, x, y)` |
| per-pixel loops | `for face in Face::ALL { for y in 0..64 { for x in 0..64` (129 lines) | one new `Canvas::pixels() -> impl Iterator<Item=(Face,u16,u16)>`; each triple loop becomes one line and is then topology-correct for free |
| encode | `Canvas::encode(&mut Frame)` | add `Canvas::encode_raster(&mut Raster)`; sRGB encode is shared and untouched |
| `FrameSink` | `submit(&mut self, frame: &Frame)` | `submit(&mut self, out: Output<'_>)` with `enum Output<'a> { Cube(&'a Frame), Flat(&'a Raster) }` — the trait must stay object-safe for `FanOutSink(Vec<Box<dyn FrameSink>>)` (`sink/fanout.rs:19`); a generic would monomorphise the whole host per topology |
| `ShimSink` | `CubeClient` + `Frame` mailbox | the same worker and mailbox, sending a `Raster` in wire format 2 (strips); the newest-frame mailbox and backoff are unchanged |
| `PngSink` | `net_rgb8` → 256×128 PNG | flat writes the raster directly as a `w×h` PNG; `net.rs` untouched |
| `WebSink` | `/frame` = 8-byte seq + 61,440 frame bytes | `/frame` = 8-byte seq + the raster bytes; `/status` gains `"topology"` so the page picks a mode |
| `index.html` | three.js cube + 768×384 net | a third mode: one `<canvas width=w height=h>` with `image-rendering: pixelated`; the cube/net code stays for cube worlds |
| `PreviewSink` | net + ray-cast cube (`minifb`) | **refuses** a flat world with a clear error (`--sink web` or `png` instead). The ray-cast camera has no meaning on a plane and the window is a development tool |
| care overlay | `CareTarget { face: u8, u, v }`, `face > 4` rejected (`care/mod.rs:134`) | validate `face == 0` and `u < w`, `v < h` for flat |

CLI/config: `WorldConfig` gains `topology`, so it is chosen at `--fresh` from the
TOML and then carried by the snapshot:

```toml
topology = "cube"                        # default, unchanged
topology = { flat = { w = 320, h = 180 } }
```

`open_world` (`runner/mod.rs:128`) must refuse a resume whose `--config`
topology differs from the loaded snapshot's, by name, like every other schema
refusal. `--sink preview` with a flat topology is refused at argument validation
(`cli.rs`, next to the existing `--fresh`/`--require-resume` check at line 262).

`Raster` itself is W1b's, in the shim repo. Cubarium picks it up by running
`scripts/sync-cube-proto.sh` and bumping `vendor/cube-proto.rev` (currently
`7a21b5f7c7a22ee36b4930471cdf1afaef45abdd`, which has no `Raster`).

## 4. Persistence

`WorldState.config` is inside the postcard payload
(`crates/cubarium-core/src/world/state.rs:23-24`), so putting `topology` in
`WorldConfig` puts it in the snapshot header's payload automatically and no
header change is strictly required. Recommended anyway: also widen the
**fixed header** with a topology word next to the schema, so a loader can refuse a
flat snapshot without decoding the payload
(`snapshot.rs:46-48`, `HEADER_FIXED_BYTES`).

This is a **schema bump to 17 and `CONFIG_VERSION` 9, refusing 7..=16 by name** —
not a migration. It is forced twice over: `ScalarField` changes from
`[f64; 1280]` (postcard writes fixed arrays with no length prefix) to a
length-prefixed slice, and `WorldConfig` gains a field. That matches the standing
rule of 2026-09-15 and the existing test
`every_older_schema_is_refused_by_name` (`snapshot.rs:539`); add 16 to its list
and freeze a `v16.rs` mirror beside `v7..v14`.

On positions: keep `SurfacePoint`'s `face` byte in the record rather than
introducing a second position type. It costs one byte per organism, keeps the
struct `Copy` and the serde derives as they are, and the loader validates
`face == Face::Front` for a flat world. A face-less flat position type would
fork `SurfacePoint`, `OrganismView`, `PathSegment` and every test fixture for one
byte.

## 5. Ecology and art: what is not mechanical

Everything below reads `p[1]` — the cube's embedded height — and it is the one
place a flat port changes the world rather than the coordinates.

| system | cube | flat — recommended |
|---|---|---|
| light / moisture | `L₀ = light_base + light_height_gain·y + noise`, `y = cell.center().embed()[1]` (`habitat.rs:94-98`) | **design call: `height(p) = 1 − 2v/h`.** The panel is a side view: canopy at the top row, foliage in the middle, soil at the bottom edge. Keeps the whole stratified design (`design/stratified-world.md`) with no other change |
| noise sampling | 3D noise on `[-1,1]^3` so seams are invisible (`habitat.rs:63-70`) | sample the same wave sum at `[u/32, v/32, 0]`: identical feature size in pixels, no seams to hide |
| band thresholds | `Canopy` iff `h >= 1.0` — true only on the Top face (`art_present/habitat.rs:380`) | **design call:** `h >= canopy_top` with a configured default (`0.67`), else a flat world has a one-row canopy |
| detritus fall | to the graph neighbour with the lowest `h` (`design/stratified-world.md`) | works unchanged: downhill is "one cell toward larger `v`" |
| water flow / pools | `z = h + basin_gain·n_b`, no flux across the open rim (`water.rs`) | works unchanged; the bottom edge becomes the moat the cube's rim never had, so **expect standing water along the bottom row** and re-check `evap_floor`. Flag for a short run |
| pair rejection | conservative chord bound (`pairs.rs:64`) | exact Euclidean; strictly fewer candidate pairs, same results |
| founders | random face + `unit·FACE_EXTENT` (`lifecycle.rs:69-72`) | one chart, `unit·w`, `unit·v`; the random stream draw for the face must be **kept and discarded** or the seed stream shifts — state which, in the brief |
| seam-carried sprites | `unfold_pixels` owns pixels across seams; bodies clip at the rim | no seams; bodies clip at all four edges. Visibly *simpler*, never worse |
| tall columns / rigs | columns chosen per side face, `Face::ALL` order (`art_present/mod.rs:365`) | choose columns along `u`; the three-quarter rigs already read as "walking along a wall", which is exactly the flat view |
| ray-cast preview | `raycast.rs` | not ported; cube-only |

The visible risks that need a design call rather than a port: the **canopy
threshold**, **water pooling against the new bottom wall**, and **organism
density** — the same 512-organism cap over 3,600 cells instead of 1,280 thins the
world by 2.81× in ecological terms, whatever the raster (§6). Recommend keeping
the cap and raising `founders` proportionally on a fresh flat world, then judging
by eye.

## 6. World resolution: the trade, and the recommendation

The display daemon integer-upscales, so the sim raster is the free variable.
Introduce a second, independent knob: **`world_scale` S**, one factor that
multiplies every length in the world — sprite tile, field cell, body extent,
sense radius, speed in px/s, stamp and deposit radii. Choose `S = 6/k` for an
upscale factor `k` and the world is the *same* world at a different sharpness.

| raster | upscale `k` | `S` | sprite tile | cell px | field cells | raster px | × cube | render ∝ |
|---|---|---|---|---|---|---|---|---|
| 320×180 | 6 | 1 | 16 | 4 | 3,600 | 57,600 | 2.81× | 1.0 |
| 480×270 | 4 | 1.5 | 24 | 6 | 3,600 | 129,600 | 6.33× | 2.25 |
| **640×360** | **3** | **2** | **32** | **8** | **3,600** | **230,400** | **11.25×** | **4.0** |
| 960×540 | 2 | 3 | 48 | 12 | 3,600 | 518,400 | 25.3× | 9.0 |
| 1920×1080 | 1 | 6 | 96 | 24 | 3,600 | 2,073,600 | 101× | 36.0 |

Every row divides exactly: `w/(4S) = 80`, `h/(4S) = 45` in all five. That is the
key result — **holding `cell = 4·S` keeps the field grid, the ecology, the
diffusion substeps, the water flow, the `RenderView` vectors and the snapshot
size constant across the whole range.** Resolution buys pixels, not ecology.

What scales with what:

- **With cells (constant here):** diffusion, water flow, rain, habitat
  evaluation, detritus fall, `RenderView` field vectors, snapshot payload.
- **With organisms (constant here):** the O(n²) pair pass (512 → 130,816),
  controller, motor, neural. Note the *stamp* per organism is not constant: a
  body extent of `9·S` px covers `∝ S²` pixels, so per-organism raster work rises
  with S at the same rate as the background passes.
- **With raster area:** the ground/ramp/water/rain passes and the per-organism
  stamps — both `∝ S²`. Hence one law: **render ∝ S², sim ≈ constant.**

**Device estimate.** Let `R` be the measured single-core cost of one *cube*
render on the board (20,480 px, art mode). Flat render ≈ `2.81 · S² · R`, and the
budget is 16.6 ms at 60 fps or 33.3 ms at 30 fps. The presenter is off the sim's
critical path and writes disjoint pixels, so splitting it over four A55 cores by
row bands is deterministic and worth roughly 3.5×. The decision rule:

| S | 1 core, 60 fps needs | 4 cores, 60 fps needs | 4 cores, 30 fps needs |
|---|---|---|---|
| 1 | `R ≤ 5.9 ms` | `R ≤ 20 ms` | `R ≤ 41 ms` |
| 2 | `R ≤ 1.5 ms` | `R ≤ 5.2 ms` | `R ≤ 10 ms` |
| 3 | `R ≤ 0.66 ms` | `R ≤ 2.3 ms` | `R ≤ 4.6 ms` |
| 6 | `R ≤ 0.16 ms` | `R ≤ 0.58 ms` | `R ≤ 1.2 ms` |

The 20 Hz simulation has a 50 ms budget and does not move with S at all; on a
6-core A55 board a tick that meets the 20 ms desktop p99 target should land
around 15–25 ms, so the sim is not the constraint and lowering the *sim* rate
buys nothing here. Lowering the *render* rate to 30 fps doubles the affordable S²
— it is the first knob, and it costs only motion smoothness, because the shim
holds the last frame and keeps presenting at panel rate.

**Recommendation: 640×360 at S = 2 for the first flat world**, because

- 3× is an integer upscale, so the pixel-art grid stays visible (each world pixel
  is a 3×3 block); 1920×1080 at S = 6 has no pixel-art read at all;
- S = 2 is an *integer* art scale: every 4-px and 16-px constant in the tall-plant
  and rig code doubles exactly (4→8, 16→32, tile rows 10/15→20/30) with no
  rounding, which S = 1.5 does not give;
- creatures read at ~32 world px ≈ 96 device px — four times today's linear
  detail, which is the "more real estate" Wrysk asked for;
- 11.25× the cube's pixel work is absorbable by row parallelism plus 30 fps even
  on a pessimistic `R`, and if it is not, 480×270 is a config edit, not a rewrite.

Both `topology` and `world_scale` are world config, fixed at `--fresh`, so the
trade can be re-run on the board with a number instead of a redesign.

## 7. The artwork pipeline, and what a rescale costs

Today (`art/README.md`, `art/bake.gd`, `art/PLANTS.md`):

| stage | where | ties to 64 px |
|---|---|---|
| source art | `art/parts/*.svg` (148 files) | **pixel art drawn on integer grids** — `bud.svg` is `width="4" height="4"` with `shape-rendering="crispEdges"` |
| rigs / timelines | `art/creatures/*.tscn`, `art/plants/*`, `AnimationPlayer` tracks | "one scene unit is one cube pixel", body faces `+x` |
| bake | `art/bake.gd` (Godot 4.7.2, headless `SceneTree`, CPU raster) | `TILE = 16`, `GROUND_TILE = 8`, `FRAMES = 16`, `PLANT_FRAMES = 24`, trunk "a 4-px-periodic segment stacked every cell" |
| pack | `assets/atelier/{creatures,plants,tall,ground,habitat}.png` + `pack.json` v5 | `tile: 16`, `pivot: [8,8]`, `ground_tile: 8`; creatures 256×256, plants 384×608, tall 384×112 |
| load | `crates/cubarium/src/art.rs` | asserts `meta["tile"] == 16` and `pivot == [8,8]` (`art.rs:288`), atlas dims `16·frames × 16·rows` (`art.rs:329,372,470,565`), vine strips fixed at 16×16 (`art.rs:202-224`) |
| stamp | `cubarium-render/src/sprite.rs` (`stamp_layers_bent`), `crates/cubarium/src/lanternjaw/raster.rs` | `unfold_pixels` radius; pivot in chart px |
| tall plants | `crates/cubarium/src/art_present/tall.rs` | the worst offender: `4·i` px per tile, `tall_grown_px(n) = 4n+8`, "eleven cells tall", tile rows 10 and 15 named explicitly (`tall.rs:190-296`) |

**The honest finding.** Raising `svg/scale` in the `.import` files and `TILE` in
the baker produces sprites that are S× *bigger blocks of the same art* — no new
detail, because the sources are already pixel art at 1 unit = 1 pixel. Genuine
higher-resolution artwork means redrawing those 148 SVG paths on an S× finer
grid. Each file is small (a handful of `<path>` elements), but it is authoring
work, not a bake parameter. The staging that follows from that:

- **Stage A (code, mechanical):** bake at `TILE = 16·S`, `GROUND_TILE = 8·S`,
  `svg/scale = S`; `pack.json` v6 makes `tile`, `ground_tile` and `pivot` data
  the loader honours instead of asserting; every hard `16`/`8`/`4` in `art.rs`,
  `art_present/tall.rs` and `lanternjaw/raster.rs` becomes `tile`-relative. The
  picture is unchanged in shape, only larger. This unblocks everything else.
- **Stage B (authoring, incremental):** redraw parts at the finer grid,
  file by file. The runtime reads the same pack, so art can improve one creature
  or one plant at a time after the flat world is already on the panel.

**One factor can drive the code side.** Put `world_scale: f64` in `WorldConfig`
and have the *fresh-world default builder* multiply the length-dimensioned
defaults: `organism.speed_max 5.0` px/s, `organism.sense_radius 6.0`,
`organism.body_extent_max 9.0`, `drives.birth_offset_px 2.5`
(`crates/cubarium-core/src/config.rs:551,554,580,609`), `CELL_PIXELS 4.0`
(`cubarium-surface/src/field.rs:14`), `MAX_LOCAL_RADIUS 32.0`
(`unfold.rs:10`, which `config.rs:863,917` validates against), plus the deposit
and care radii. Pace stays **1 BL/s** by construction: body length and px/s scale
together, so the calibration recorded on 2026-09-14 is preserved exactly. Two
cautions: scale the *defaults*, never a value read from a TOML file, or an
explicit config gets scaled twice; and `MAX_LOCAL_RADIUS` bounds `unfold_pixels`
cost quadratically, so raising it is a performance decision as well as a
geometric one.

## 8. Biome and terrain variation (separable)

What exists already: three height bands and a light/moisture gradient
(`design/stratified-world.md`); patch noise on the embedding for light and
moisture and a basin noise for pools (`cubarium-core/src/habitat.rs:63-104`);
moving moisture blobs on 20–47 minute periods driving rain
(`design/water.md`); downhill detritus fall and gravity-driven flow; and a
stimulus envelope for external `Light | Moisture | Nutrient | Flow` events
(`design/environmental-inputs.md`). On a flat world all of that survives and
varies only *vertically* — the world is banded but horizontally uniform, which is
exactly the monotony Wrysk flagged about the top-down cube.

**Bounded proposal, first version.** One extra low-frequency wave sum `R(p)`,
same construction as the existing patch noise but with a wavelength near half the
world width, evaluated once per cell at world creation beside `positions` and
`terrain`. `R` selects one of **four** biomes with a blend weight, and a biome is
nothing but a set of *offsets to parameters that are already per-cell inputs*:

| biome | offsets |
|---|---|
| wetland | `+basin_gain`, `+moisture_base`, `−light_base` |
| meadow | baseline (the current defaults) |
| scrub | `−moisture_base`, `+light_base` |
| barrens | `−moisture_base`, `−nutrient` initial charge, `+evap` |

Three properties make this safe and separable: no new field and no new material
flow, so the closed-box invariant and the energy audit are untouched; it is
static (computed at creation, checkpointed with the habitat), so it costs nothing
per tick; and it sits behind `mechanisms.biomes`, **off by default**. Presentation
picks the ground tile and palette by dominant biome and blends with the existing
seam-aware filter. An authored `biome_map` PNG read nearest-cell is a drop-in
alternative source for `R` later.

This is FW-8. The first flat world ships without it.

## 9. The plan

| id | objective | files (exclusive) | interface exposed | must stay green | verification | size | effort |
|---|---|---|---|---|---|---|---|
| FW-0 | Vendor `cube-proto` with `Raster` + wire format 2 once W1b lands; measure `R`, the single-core cube render cost, on desktop and on the board | `vendor/cube-proto/**`, `vendor/cube-proto.rev` | `Raster { width, height, data }` | whole workspace | `cargo test --workspace`; a recorded `R` in ms | small | medium |
| FW-1 | `Topology` enum; widen pixel indices to `u16`; runtime cell count; the `Flat` arms of travel/unfold/raster/field | `crates/cubarium-surface/**` | §2's API; `CUBE_CELL_COUNT` for cube-only callers | surface's 99 tests | cube results identical *by value*, not only by test | large | **high** — reflection on four edges, tie rules, exactness |
| FW-2 | Thread topology and `world_scale` through the world: config, schema 17, habitat height/noise, water, fields, pairs, founders, care | `crates/cubarium-core/**` | `WorldConfig.{topology,world_scale}`, `World::topology()` | 479 core tests | fixed-seed cube run: identical `ecology_hash`; a flat run reaches steady state | large | **high** — the height/canopy/water calls, RNG stream order |
| FW-3 | `Canvas` by topology, `Canvas::pixels()`, `encode_raster`; port `field/trail/sprite/body/multipart`; row-parallel presenter hook | `crates/cubarium-render/**` | `Canvas::new(topo)`, `pixels()`, `encode_raster` | 102 render tests | same-seed cube canvas bit-identical | medium | medium |
| FW-4 | `Output` enum + sinks (shim/png/web), viewer flat mode, CLI/config, preview refusal, measured ms/frame both topologies | `crates/cubarium/src/{sink/**,cli.rs,net.rs,run.rs,runner/**}`, `sink/web/index.html` | `enum Output`, `topology`/`world_scale` TOML | host sink + CLI tests | a flat PNG capture; viewer screenshot; the numbers that pick `--fps` | medium | medium |
| FW-5 | Presenter and art for flat at S = 1: bands, horizon, water/rain, motifs, columns, bodies, care effects | `crates/cubarium/src/{present.rs,art_present/**,lanternjaw/**,care_effects.rs,scene.rs}` | — | 344 host tests; cube PNG byte-identical | a 320×180 capture reviewed by Wrysk | large | **high** — new UI construction |
| FW-6 | Independent test authoring: flat travel/reflection/unfold, field conservation, schema-17 refusal, sink/raster, presenter goldens | new files only: `crates/*/tests/flat_*.rs` | — | — | written without reading FW-1..FW-5's own tests | medium | **high** |
| FW-7 | World scale: pack v6 (`tile` as data), baker at `TILE = 16·S`, `art.rs`/`tall.rs`/`lanternjaw` constants made tile-relative, the S-scaled default builder; re-bake at S = 2 and ship 640×360 | `art/**`, `assets/atelier/**`, `crates/cubarium/src/art.rs`, `art_present/tall.rs`, `lanternjaw/**` | `pack.json` v6, `world_scale` defaults | the S = 1 pack must still load and render bit-identically | a 640×360 capture beside the 320×180 one; reproducible Godot bake | large | **high** — `tall.rs`'s 4/16-px lattice is the densest coupling in the repo |
| FW-8 | Biomes: the region field, four parameter sets, `mechanisms.biomes` off by default, presentation by dominant biome | `crates/cubarium-core/src/habitat.rs` (+ a new `biome.rs`), `crates/cubarium/src/art_present/habitat.rs` | `HabitatConfig.biomes` | everything, with the toggle off | toggle off ⇒ `ecology_hash` unchanged; toggle on ⇒ a short run showing distinct regions | medium | medium |

The staging is deliberate: **FW-1..FW-6 ship a correct flat world at S = 1
(320×180)**, which is the smallest sound diff and proves the topology on the
panel; **FW-7 then turns the scale up to the recommended 640×360 at S = 2**
without touching topology again. If FW-7 slips, the panel still works.

Ordering: FW-0 any time. FW-1 first. Then **FW-2 ∥ FW-3** (disjoint crates), then
**FW-4 ∥ FW-5** (disjoint file sets inside `crates/cubarium/src`). FW-6 starts as
soon as FW-1's signatures are frozen and only creates new files, so it never
collides. FW-7 follows FW-5 and touches `art_present/tall.rs`, which FW-5 owns —
they must not run together. FW-8 is last and can slip indefinitely. W2 (the
device) follows FW-7.

Four packages are high effort, for different reasons: FW-1 because a reflection
or tie-rule mistake is silent and corrupts motion; FW-2 because the
height/canopy/water calls decide whether the flat world is alive or a flat lawn,
and touching the founder RNG order changes every seed; FW-5 because it is new UI
construction, which the working rules put at high effort by default; FW-7 because
`tall.rs` encodes the 4-px cell and 16-px tile lattice in dozens of named row
indices, and an off-by-one there is a visible seam in every plant.

**The standing evidence at every package**, since the cube must keep working:
`cargo test --workspace` green, plus a fixed-seed
`cubarium run --fresh --seed 1 --speed 0 --seconds 120 --sink png` capture whose
PNG bytes and `ecology_hash` match the pre-change run. That is what
"byte-for-byte identical" means operationally, and it is cheap enough to run on
every package.

**Test authoring is its own pass.** FW-6 is that pass, at high effort, written
against §2's contract and §5's design calls rather than against the
implementation. FW-7 and FW-8 each need their own small authoring pass for the
same reason — a worker's green tests are not evidence about its own art or its
own biome field.

## What this audit could not determine

- **Device performance.** No build was run (the shared `target/` belongs to the
  main checkout, and a worktree build would cost tens of GiB) and the Tachyon is
  not reachable from this task. §6 gives the scaling law and a decision table
  keyed on a single measured number `R`; FW-0 must produce it before S is fixed.
- **The exact `Raster` API.** W1b had not landed; the vendored `cube-proto` at
  `7a21b5f` has no `Raster` and no format 2. FW-0 may need a small adaptation.
- **How much art authoring Stage B really is.** 148 SVG part files were counted
  and one was read; the effort to redraw them at 2× is a judgement for whoever
  draws them, not something this audit can size.
- **Whether 640×360 reads well.** That is Wrysk's call on the panel. The raster
  and `world_scale` are both config precisely so it can change without a code
  edit.
