---
design_status: leaning
last_reviewed: 2026-09-16
decision_refs: []
---

# A ring W×H world beside the cube world

Coupling audit and phased plan for the Tachyon panel, written by Opus on
2026-09-16 under
[the tachyon-screen plan's revision](handoffs/tachyon-screen-plan-2026-09-16.md).
Read-only audit: no source file was changed to write it.

## Summary

1. Cubarium is coupled to the cube through **five hubs**: `SurfacePoint`/`Face`,
   `CellId`+`CELL_COUNT`, `Canvas`, `FrameSink::submit(&Frame)`, and the
   world/view/presenter caches that are shaped five-by-`CELL_COUNT` at birth.
2. Recommended abstraction: a `Copy` **enum** `cubarium_surface::Topology { Cube,
   Ring { w, h } }` **plus a `Scale`** (cell pixels and world scale) in the same
   surface contract — not a trait, not a generic, so `SurfacePoint`, `WorldState`
   and `Canvas` stay concrete and the 1,591 existing tests survive.
3. `Face` stays the chart id; a ring has one chart, `Face::Front`. Three forced
   widenings: pixel indices `u8 → u16`, `CELL_COUNT` to a runtime count, and a
   checked `cell_count() <= u16::MAX` bound on `CellId`.
4. **The world is a ring: left and right join, top and bottom are solid.** That
   needs no new geometry — the vertical edge is a seam of the chart to itself
   (identity map, translation by `∓w`), the rims reuse the existing `REFLECT_Y`
   bounce, and a ring corner *is* the cube's lower side corner, tie rule included.
   §5 decides the canopy drain; §5a embeds the ring as a cylinder.
5. **Candidate first ring world: 640×360, upscaled 3×, at world scale S = 2** —
   sprite tile 32 px, field cell 8 px. Gated on FW-0's measured board numbers,
   not fixed here. 320×180 is the cube world stretched and wastes the panel.
6. `world_scale` S multiplies every length. Hold `cell = 4·S` and embed on an
   isotropic cylinder (`r = w/(2π·32·S)`, `y_e = (h/2 − v)/(32·S)`), and every
   raster from 320×180 to 1920×1080 has the same 3,600 cells, the same noise scale
   and the same ecology — weather defaults included, because a blob centre is
   uniform on the whole sphere, so one blob covers 21.3% of either world.
7. Sim and render share **one host loop**, so the budget is
   `20·tick_ms + fps·render_ms <= 1000 ms`, not two independent budgets. Render
   work is `∝ S²`; the tick is S-invariant.
8. Genuine higher-resolution art is authoring, not a bake flag: the 148
   `art/parts/*.svg` are pixel art on integer grids. A runtime `scale` path
   exists but is blocked by a hard 9-pixel stamp budget that must scale with S.
9. Biome/terrain variation is real but **separable**: one low-frequency region
   field offsetting per-cell habitat parameters, behind a toggle, off by default.
10. Total: **ten packages — 5 large, 4 medium, 1 small**; seven at high effort.
    FW-1..FW-8 are CPU-only; FW-9 is the approved GPU follow-on. Parallel pairs
    start only after FW-1's contract is frozen.

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
| **world / view / presenter caches** | `crates/cubarium-core/src/world/mod.rs:44-53`, `src/view.rs:34-61`, `crates/cubarium/src/art_present/mod.rs:416-423` | `World` holds `images: [Vec<ChartImage>; 5]` and five `Box<[f64; CELL_COUNT]>` weather caches; `RenderView` carries **no** topology, so the presenter cannot learn the shape from it; `ArtPresenter::new` lays out every slot and band through the **global** `CellId::all()` at construction |

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
pub enum Topology { Cube, Ring { w: u16, h: u16 } }
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

**The `Ring { w, h }` arm.** *(Renamed from `Flat` on Wrysk's direction of
2026-09-16 — the `FW-` package ids and this file's name keep the earlier "flat"
label; the topology is `Ring` everywhere in code.)* **The left and right edges
join; the top and bottom stay solid.** The whole point of the shape is that it
needs no new machinery: a ring is one chart whose right edge is a seam **to
itself**, and the cube already transports across seams and reflects at an open
rim.

- **One chart**, `Face::Front`, `extent = (w, h)`.
- **The vertical seam is the chart to itself:** exiting at `u = w` enters at
  `u = 0` and vice versa, along-edge parameter unchanged, **zero quarter turns,
  `TangentMap::IDENTITY`** — a pure translation by `∓w`. That is exactly the
  `Front right → Right left` row of the seam table in `design/surface-topology.md`
  with the neighbour being the same chart, so `travel` takes its **existing**
  `Some(seam)` branch (`travel.rs:263-275`) with no new code path: `edge_param`,
  `edge_point` and `seam_turns` simply answer for the ring's own extent.
- **The only reflection is the existing rim bounce**, `REFLECT_Y`
  (`travel.rs:254-258`), now at both `v = 0` and `v = h` instead of the cube's
  single bottom rim. There is **no `REFLECT_X` and no corner rule** — the
  previous revision's per-axis algebra and its "inverted tie rule" are deleted.
- **Corners need nothing new either.** A ring corner is a point where the
  vertical seam meets a horizontal rim, which is precisely the cube's *lower side
  corner*: "at a lower side corner the unfolded boundary is straight; test a step
  that both crosses a vertical seam and reflects, including an exact tie"
  (`design/surface-topology.md`, "The open bottom and exact corners"). The
  existing lowest-`Edge` tie rule applies unchanged, and
  `lower_corner_crosses_a_vertical_seam_and_reflects` (`travel.rs:478-492`) is the
  precedent the ring's fixture mirrors.

**Unfolding: at most two images in range.** `chart_images` enumerates three
candidates — the direct image and the translations by `+w` and `−w` — of which at
most **two** can lie within `max_local_radius()` of any observer, since the two
shifts are `2w` apart; the shortest wins, exactly as today. To guarantee that (and
that no further translation can ever be nearest), `Topology::validate()` requires

> `w >= 2 · max_local_radius() + 2 · CELL_PIXELS`

and `Ring::max_local_radius() = min(h, w − 2·CELL_PIXELS) / 2` satisfies it by
construction. At 320×180, S = 1 that is `min(180, 312)/2 = 90`, and the stamp
budget `9·S <= max_local_radius()` holds with room to spare (`18 <= 180` at
640×360, S = 2). `MAX_SEAMS` stays 2 and is not consulted: the ring enumerates its
own three images directly.

**Distance is a topology method, not a scaled embedding.** `chord_sq` multiplies
the embedded chord by a hardcoded `1024 = 32²` to reach pixel units
(`point.rs:108-116`), which is right only for the cube's `/32` embedding. Make it
`Topology::chord_sq(a, b) -> f64` in squared **pixels**: the cube arm is today's
body verbatim; the ring arm is

> `min(|Δu|, w − |Δu|)² + Δv²`

taken straight from the chart coordinates — exact, cheaper, wrap-aware, and
independent of the embedding. Pair rejection stops being a conservative bound and
becomes the true distance (`pairs.rs:64`).

**Local-radius bounds are per topology; the cube's proof is not rescaled.**
`MAX_LOCAL_RADIUS = 32.0` is a *completeness* proof, not a tunable: within it
"every shortest path crosses at most `MAX_SEAMS` seams … so enumerating chart
paths of that length is complete" (`unfold.rs:5-13`). Nothing about `S` may touch
it. Two rules follow:

- **`world_scale` is a ring-only parameter. `Topology::Cube` pins `S = 1`**, so
  the cube keeps its 32-px bound, its 9-px stamp budget and its proof untouched.
  Validation refuses a cube world with `world_scale != 1`.
- `Ring::max_local_radius()` is a **chosen performance cap**, not a limit of
  meaning: `unfold_pixels` cost grows with the square of the radius, and the cap
  is also what keeps the image count at two. No radius is meaningless on a ring.

**A capacity bound.** `CellId` is a `u16` (`field.rs:18-21`), so a topology may
not exceed 65,535 cells. At `cell = 4 px` a 1920×1080 world would want 129,600 —
over the limit. `Topology::validate()` computes `cell_count()` with checked
multiplication and refuses anything above `u16::MAX`, naming the raster and the
cell size. The §6 ladder (`cell = 4·S`) stays at 3,600 and is far inside it, but
an arbitrary `Ring { w, h }` from a config file is not.

**Two forced widenings.**

- `u8 → u16` for pixel indices. 320 > 255, so `SurfacePoint::pixel_center`,
  `pixel()`, `pixel_neighbor`, `PixelImage { x, y }`, `Canvas::{get,set,add}` and
  the 129 `0..64u8` loop bounds must widen. Purely mechanical, zero behaviour
  change on the cube, and the compiler finds every site.
- `CELL_COUNT` from a const to a runtime count. `1,280 = 2^8·5` cannot be
  factored 16:9 with square cells, so no ring raster reproduces it.
  `ScalarField.values` becomes `Box<[f64]>` sized at construction, `CellId`
  becomes a plain index decoded through the topology, and `World`'s five
  `Box<[f64; CELL_COUNT]>` weather caches and `[Vec<ChartImage>; 5]` become
  runtime-sized (`world/mod.rs:48-53`). **A partial mitigation, stated honestly:**
  `pub const CUBE_CELL_COUNT: usize = 1280` lets cube-only tests and fixtures keep
  their literals as a symbol rename (186 lines in `cubarium/tests`, 28 in
  `cubarium-core/tests`, 26 in examples). It does *not* preserve them
  semantically — `for c in 0..CELL_COUNT { state.fields.d[c] }`
  (`cubarium-core/tests/redesign_rules.rs:116,418`) iterates a runtime-length
  vector and is correct only because the test builds a cube world. Any test that
  becomes topology-parameterised must take its count from the world, not a const.

**Cell geometry and world scale belong in this contract, not downstream.**
`CELL_PIXELS` is a surface constant read by `CellId::center` and `cell_of`
(`field.rs:11-16,49-68`), so a later package cannot change the cell size without
owning `cubarium-surface`. `Topology` therefore ships beside a `Scale { world: f64 }`
with `cell_pixels() = 4.0 · world`, `footprint_radius()` and `max_local_radius()`
derived from it, all in FW-1, and FW-1 exercises `S = 2` from its first commit
while keeping the `S = 1` cube goldens. Later packages choose a *value*; they
never change the geometry.

**Cell size for a ring world: `4·S` px.** At S = 1, 320×180 → 80×45 = 3,600
cells. **Every row is a ring**, so the horizontal wrap adds one edge per row:
`80·45 + 80·44 = 7,120` undirected edges. Degrees are simpler than a rectangle's —
**there are no corners**: 3,440 interior cells of degree 4, and the 80 + 80 cells
of the top and bottom rows at degree 3, since `field.rs:98-119` emits `None` only
for the two horizontal rims. (The previous revision's degree-2 corner case is
deleted with the walls that created it.) `downhill` is unchanged from §5's
decision: `None` on the top row, else `(cx, cy+1)`. `w` and `h` must be multiples
of `cell_pixels()`; every raster in §6 is.

## 3. Render and host

| piece | cube today | ring |
|---|---|---|
| `Canvas` | `Box<[[[f32;3]; 4096]; 5]>` | `Canvas::new(topo)`, a flat `Vec<[f32;3]>` of `w·h`, same `get/set/add` on `(Face::Front, x, y)` |
| per-pixel loops | `for face in Face::ALL { for y in 0..64 { for x in 0..64` (129 lines) | one new `Canvas::pixels() -> impl Iterator<Item=(Face,u16,u16)>`; each triple loop becomes one line and is then topology-correct for free |
| encode | `Canvas::encode(&mut Frame)` | add `Canvas::encode_raster(&mut Raster)`; sRGB encode is shared and untouched |
| `FrameSink` | `submit(&mut self, frame: &Frame)` | `submit(&mut self, out: Output<'_>)` with `enum Output<'a> { Cube(&'a Frame), Ring(&'a Raster) }` — the trait must stay object-safe for `FanOutSink(Vec<Box<dyn FrameSink>>)` (`sink/fanout.rs:19`); a generic would monomorphise the whole host per topology |
| `ShimSink` | `CubeClient` + `Frame` mailbox | the same worker and mailbox, sending a `Raster` in wire format 2 (strips); the newest-frame mailbox and backoff are unchanged |
| `PngSink` | `net_rgb8` → 256×128 PNG | a ring writes the raster directly as a `w×h` PNG; `net.rs` untouched |
| `WebSink` | `/frame` = 8-byte seq + 61,440 frame bytes | `/frame` = 8-byte seq + the raster bytes; `/status` gains `"topology"` so the page picks a mode |
| `index.html` | three.js cube + 768×384 net | a third mode: one `<canvas width=w height=h>` with `image-rendering: pixelated`; the cube/net code stays for cube worlds |
| `PreviewSink` | net + ray-cast cube (`minifb`) | **refuses** a ring world with a clear error (`--sink web` or `png` instead). The ray-cast camera has no meaning on a ring and the window is a development tool |
| care overlay | `CareTarget { face: u8, u, v }`, `face > 4` rejected (`care/mod.rs:134`) | validate `face == 0` and `u < w`, `v < h` for a ring |

CLI/config: `WorldConfig` gains `topology`, so it is chosen at `--fresh` from the
TOML and then carried by the snapshot:

```toml
topology = "cube"                        # default, unchanged
topology = { ring = { w = 320, h = 180 } }
```

`open_world` (`runner/mod.rs:128`) must refuse a resume whose `--config`
topology differs from the loaded snapshot's, by name, like every other schema
refusal. `--sink preview` with a ring topology is refused at argument validation
(`cli.rs`, next to the existing `--fresh`/`--require-resume` check at line 262).

`Raster` itself is W1b's, in the shim repo. Cubarium picks it up by running
`scripts/sync-cube-proto.sh` and bumping `vendor/cube-proto.rev` (currently
`7a21b5f7c7a22ee36b4930471cdf1afaef45abdd`, which has no `Raster`).

## 4. Persistence

**Correction to the first draft: the persisted fields are already length-prefixed.**
`Fields` holds `Vec<f64>` per channel (`crates/cubarium-core/src/fields.rs:19-32`),
and the fixed-size `Box<[f64; CELL_COUNT]>` arrays live in `Habitat` and `World`
(`habitat.rs:26-33`, `world/mod.rs:48-53`), which are rebuilt on load and never
serialized. `ScalarField` is therefore **not** a serialization cause. The real
cause is simpler: `WorldState.config` is inside the payload
(`world/state.rs:22-28`) and `WorldConfig` gains `topology` and `world_scale`, so
the postcard shape changes.

That is still a hard bump to **schema 17 / `CONFIG_VERSION` 9, refusing 7..=16 by
name**, matching `every_older_schema_is_refused_by_name` (`snapshot.rs:539`);
freeze a `v16.rs` mirror beside `v7..v14`.

**Do not widen the fixed header.** `HEADER_FIXED_BYTES` is 22 and the layout
`[magic][schema u32][id_len u16][id][payload_len u64][crc32 u32][payload]`
(`snapshot.rs:48-50,78-97`) is parsed with hardcoded offsets outside Rust, by
`scripts/reduce-quiet-compare.mjs:112-126`. Inserting a topology word there breaks
that tool for no gain: the schema number already refuses a mismatched build.

**The post-decode hook already exists and is named `WorldState::validate`.**
`decode_snapshot` calls it at `snapshot.rs:163` after the CRC and the exact-length
decode (`world/state.rs:108`). The ring work is to extend it, and to replace the
`CELL_COUNT` constants it reaches with the world's runtime `cell_count()`:

- `config.topology` is one this build supports and `Topology::validate()` passes:
  dimensions positive, multiples of `cell_pixels()`, `cell_count() <= u16::MAX`,
  `w >= 2·max_local_radius() + 2·CELL_PIXELS` for `Ring`, and `world_scale == 1`
  for `Cube`;
- every `Fields` vector (`fields.rs:19-32`) and every ecology v1 vector has length
  exactly `cell_count()` — these are the only per-cell serialized arrays;
- **care's persisted state is not a per-cell vector.** `CareState.showers[].cells`
  is a `Vec<u16>` of raw `CellId` indices with matching weights
  (`care.rs:257-276`); `CareState::validate` already range-checks them against
  `CELL_COUNT` at `care.rs:340,345-349`, including a `vec![false; CELL_COUNT]`
  duplicate set. Each of those becomes `cell_count()`. The first draft wrongly
  said the care state has `cell_count()` length;
- every organism's `pos.face` is a chart the topology has — `Face::Front` for a
  ring — with `u < w`, `v < h`. Weather keeps today's finiteness-only check
  (`world/state.rs:193-208`): the blob model is unchanged (§5a), so there is
  nothing topology-specific left to validate.

`CareTarget` (`crates/cubarium/src/care/mod.rs:122-140`) is a **host** command
type and is not in `WorldState`, so it is not validated here at all; its widening
and admission checks belong to FW-4 (§9). The first draft put it in the wrong
place.

Keep the `face` byte in `SurfacePoint`: one byte per organism against forking the
position type, `OrganismView`, `PathSegment` and every fixture.

**The cube regression check, and how it survives the refusal.** `ecology_hash`
hashes `postcard::to_allocvec` of the whole masked `WorldState`, config included
(`snapshot.rs:193-199`), so two config fields change it for cube worlds too — a
pre/post equality claim is impossible. Worse, schema 17 *refuses* a schema 16
snapshot by design, so the new build cannot even read the old run's output. The
comparator therefore needs a cross-schema procedure, not just a masked hash:

1. Define `CubeProjection` as **`WorldState` verbatim with exactly one
   substitution**: `config: ConfigProjection`, where `ConfigProjection` is
   `WorldConfig` minus **three** fields — `topology`, `world_scale`, **and
   `version`**. `WorldConfig.version` is a serialized field defaulting to
   `CONFIG_VERSION` (`config.rs:15,19,372`), and this plan bumps it 8 → 9 (§4
   above), so leaving it in would fail every comparison for a reason that has
   nothing to do with the world. The versions are instead asserted **separately
   and explicitly**: the fixture must report schema 16 / config 8, the new build
   schema 17 / config 9. A silent version change and a silent world change must
   not be able to cancel or mask each other.
   Every other field is carried whole, at its own type — `tick`, `fields`,
   `weather`, `organisms: Slots<Organism>` **including the allocator's `entries`,
   `free` and `live`** (`ids.rs:15-23`), `births_total`, `deaths_total`,
   `cap_rejections_total`, the material and energy totals, `care`,
   `energy_correction`, `hunters`, `quiet`, `apex_dormancy`, `apex_encounters`,
   `neural` and `ecology` (`world/state.rs:23-99`). An enumerated subset was the
   first draft's mistake: it silently dropped `ou`, `structure`, `born_tick`,
   `hunger_memory`, `mode`, `escrow`, `births`, `phenotype`, `parent`, `origin`
   and `turn_counter` from every organism (`organism.rs:46-67`), the free-list
   state, the weather, every extension, and all behaviour-bearing config. **Carry
   everything; subtract two fields.**
2. Precisely what reads what. The new build **never calls `decode_snapshot` on a
   v16 file** — that call refuses schema 16 and keeps refusing it, unchanged. Its
   *test* does read the v16 **payload** (the bytes after the header) through the
   frozen `v16.rs` mirror and `decode_exact::<WorldStateV16>`, which is the same
   mechanism the existing refusal tests already use for schemas 7–14. The
   distinction matters: the product refuses old worlds, and only the comparator
   looks inside one. No patch to `main` is needed — the fixture is one snapshot
   file from an unmodified `main` run.
3. Compare the two projections field-by-field and hash them for a one-line CI
   signal. Equality is the cube regression evidence used throughout §9.
4. **Negative tests are part of the definition**, in FW-6's reserved
   `ring_schema17.rs`: perturb one organism field, one allocator free-list entry,
   one weather blob, one field vector, one extension state and one non-added
   config field, and assert equality **fails** in every case. A comparator that
   cannot fail is not evidence.

FW-1 authors the projection type and the exporter; the pre-change fixture is a
snapshot taken from the current `main` build before FW-1 merges.

## 5. Ecology and art: what is not mechanical

Everything below reads the cube's embedded height, and it is the one place a ring
port changes the world rather than the coordinates. The first draft under-counted
the consumers; this is the corrected list.

| system | cube | ring — recommended |
|---|---|---|
| light / moisture | `y = cell.center().embed()[1]` (`habitat.rs:94-98`) | **design call: `height(p) = 1 − 2v/h`.** The panel is a side view: canopy at the top row, soil at the bottom edge. Keeps `design/stratified-world.md` intact |
| **classic controller** | `height: o.pos.embed()[1]`, `up: up_direction(o.pos.face)` (`world/step.rs:449-460`) | both become `Topology` methods; the ring's `up` is the constant `(0, −1)` |
| **neural controller** | the same two fields in `SelfState` (`world/step.rs:3068-3079`) | identical treatment; a policy trained on cube height reads the same channel |
| **depth preference** | `obs.up * (w_depth · (h_pref − obs.height))` steers every organism (`controller.rs:228-235`) | works unchanged *given* a topology height and up; with a wrong height it silently steers the whole population into a wall |
| noise sampling | 3D noise on `[-1,1]^3` (`habitat.rs:43-75`) | **the same wave sum on a cylinder** (below): seamless across the wrap by construction, no periodic-noise work, and one pixel is `1/(32S)` embedded units on **both** axes, so patches are round |
| **weather blobs** | orbital: `center` rotated about `axis` at `rate` rad/tick, plus a per-minute random-walk tilt (`habitat.rs:110-196`); sampled by `dot(b.center, dir)` against an angular cap (`habitat.rs:198-225`) | **unchanged — see §5a.** On the cylinder embedding `normalize()` preserves azimuth and maps height monotonically to latitude, so `Weather::new`, `advance` and `sample` all work verbatim. No new model, no new validation, no RNG change |
| band thresholds | `Canopy` iff `h >= 1.0` — true only on the Top face (`art_present/habitat.rs:380`) | needs a `canopy_top` threshold. **No value can be validated from current code**; propose `0.67` as an explicit new default for review, not as a derived number |
| detritus fall / downhill | gravity's tangential component vanishes on the level Top face, so `downhill` is `None` there (`field.rs:135-151`, `design/stratified-world.md:47-52`): **the cube canopy deliberately never drains**, and the bottom row of the side faces has nothing below it | **decided: the ring mirrors both exceptions.** `downhill(c) = None` when `cy == 0`, otherwise the neighbour at `(cx, cy+1)`. The top cell row is the canopy and holds its water and detritus exactly as the cube's level Top does; the bottom row has no cell below it and keeps its litter, exactly as the cube's rim row does. No toggle, no new config |
| water flow / pools | no flux across the open rim (`water.rs`) | the bottom wall becomes a moat the cube never had; expect standing water along the bottom row and re-check `evap_floor` |
| pair rejection | conservative chord bound (`pairs.rs:64`) | exact Euclidean; strictly fewer candidate pairs |
| founders | random face + `unit·FACE_EXTENT` (`lifecycle.rs:69-72`) | one chart, `unit·w` and `unit·h`; the face draw must be **kept and discarded** or every seed shifts |
| seam-carried sprites | `unfold_pixels` owns pixels across seams | **one seam, the wrap.** A body straddling `u = 0` is carried by the existing unfolding machinery with at most two images; bodies clip only at the top and bottom rims |
| tall columns / rigs | per side face, `Face::ALL` order (`art_present/mod.rs:365`) | choose columns along `u`; the three-quarter rigs already read as walking along a wall |
| ray-cast preview | `raycast.rs` | not ported; cube-only |

The calls now made in this document: the **canopy drain** (decided above) and
the **cylinder embedding** (§5a), which is what lets weather stay untouched. Still open and needing Wrysk's eye rather than a
rule: the **canopy threshold**, **water against the new bottom wall**, and
**organism density** — the same 512-organism cap over 3,600
cells instead of 1,280 thins the world by 2.81× in ecological terms, whatever the
raster. Recommend keeping the cap and raising `founders` proportionally.

## 5a. The cylinder embedding, and why weather needs no change

The ring embeds as an **isotropic cylinder** — one pixel is `1/(32·S)` embedded
units on *both* axes:

> `θ = 2π·u/w`, `r = w / (2π·32·S)`, `y_e = (h/2 − v) / (32·S)`,
> `embed(p) = [r·cos θ, y_e, r·sin θ]`

with the vertical in slot 1 because that is where the cube puts height
(`habitat.rs:94`, `p[1]`). The radius makes arc length per pixel `1/(32·S)`, the
cube's feature scale, and `y_e` uses the same divisor, so the habitat wave sum
(`habitat.rs:43-75`) samples at the cube's frequency in every direction and is
**seamless across the wrap by construction**: `u = 0` and `u = w` are the same
point in 3D. No periodic noise, no seeded tiling, no special case. This replaces
§6's former "condition 1"; the `S` divisor is now inside `r` and `y_e`.

**`embed()` and `height()` are different functions on a ring, and that is the
point.** `Topology::height(p) = 1 − 2v/h` stays exactly as §5 decided and drives
light, moisture, the bands, `downhill` and both controllers' `height` channel;
`embed()` drives *position* — noise and weather — and nothing else. They coincide
on the cube, where height *is* `embed()[1]`, which is why `habitat.rs:94` could
read the embedding directly; on a ring the two must be asked for separately.

**Decision, 2026-09-16: the anisotropic variant is rejected.** Reusing
`y = 1 − 2v/h` inside `embed()` would have stretched every noise patch and every
shower `90/32 = 2.81×` vertically at the 16:9 ladder. The stratified design needs
height only as a **scalar**, and there is no reason for the picture to stretch.
**The visible effect of the decision: habitat patches and showers are round, and
a shower crosses the world as a moving cell rather than reading as a horizontal
band.**

**Weather is unchanged.** `normalize()` of a cylinder point preserves azimuth and
maps `y_e` monotonically to latitude, so the existing spherical blob model —
`Weather::new`, `advance` with its orbit and per-minute walk, and `sample` with
its angular cap — runs verbatim on `habitat.positions`. **No draw changes, so RNG
stream parity is trivially exact** rather than something to engineer; a
stream-parity test is still worth writing, but it should pass on day one. The
earlier planar blob model and its extra `WorldState::validate` checks are deleted.

**The numbers, at 320×180 and S = 1** (`r = 1.59155`, `H_e = h/(32S) = 5.625`, so
`y_e ∈ [−2.8125, +2.8125]`):

| quantity | ring (isotropic) | cube | note |
|---|---|---|---|
| latitude span | `±60.50°` | ±90° less the missing bottom face | was `±32.15°` when `embed` reused `1 − 2v/h` |
| share of the sphere the world occupies | 87.0% | 83.3% (5 of 6 faces' directions) | how much of a blob's travel is on-world |
| a `blob_radius_deg = 55` cap | 21.3% of the sphere | 21.3% | the config is unchanged |
| **expected share of the world one blob covers** | **21.3%** | **21.3%** | identical, so `blobs_per_channel` stays **3** and `amplitude` is unchanged |
| lateral area | `2πr · H_e = 56.25` units² | 20 units² | intrinsic area, which turns out to set nothing here |

**Why the coverage figures are equal, and why two earlier derivations were wrong.**
Blob centres are drawn uniformly on the **whole sphere** (`sphere_direction`,
`habitat.rs:136`) and orbit over the whole sphere (`habitat.rs:166`), spending
part of their period off-world in either topology. For a centre uniform on the
sphere, the chance that any given world direction falls inside a 55° cap is just
the cap's own fraction — **21.3%, independent of the world's shape or area**. So
the cube's weather defaults transfer to the ring untouched. Repair 2's "3 → 8"
(from the plane's intrinsic area) and repair 3's "stays 3 because the area is
exactly 20" were both computed in the wrong frame: intrinsic surface area sets
nothing for a model that lives on the unit sphere after `normalize()`, and
dividing cap area by *visible-world* area — the 24.5% and 25.6% figures in the
previous revision — double-counts a restriction the blob centres do not obey.

**Noise is round; weather is not. Both are accepted for v1.**

- *Noise is isotropic.* The isotropic cylinder is a local isometry from the
  `(u, v)` plane — one pixel is `1/(32S)` units along both axes — so the habitat
  wave sum, which is a function of the 3D point, gives **round patches** in pixel
  space. That is what the embedding decision bought.
- *Weather caps are not.* `sample` compares `dot(b.center, dir)` **after**
  `normalize()` (`habitat.rs:217-219`), and projecting the cylinder onto the unit
  sphere is not an isometry: one pixel spans `cos φ / (32Sr)` of spherical
  distance horizontally but `cos²φ / (32Sr)` vertically, so a round spherical cap
  maps to a pixel-space shape stretched vertically by `1/cos φ`. A shower is
  **round at the equator and up to `sec(60.50°) = 2.03×` taller than wide at the
  top and bottom rows.** The previous revision's blanket claim that
  "showers are round" was wrong.
- *Vertical drift is uneven.* `dφ/dy_e = r/(r² + y_e²)` falls from `0.6283` at the
  equator to `0.1524` at `y_e = ±2.8125`, so a blob whose orbit carries it over
  the top or bottom rows sweeps through them about **4.1× faster in pixels** than
  it crosses the middle: showers move quickly across the canopy and the soil floor
  and linger in the foliage band.

**Decision, 2026-09-16: accept the shape distortion for v1; do not redesign
weather.** The knobs remain `weather.periods_min` and `weather.blob_radius_deg`,
and neither involves a code change. FW-6's `ring_weather.rs` **measures and
records the cap aspect at the rims rather than asserting it round** — the test
must pin the real behaviour, not the behaviour the first draft claimed.

## 6. World resolution: the trade, and the recommendation

The display daemon integer-upscales, so the sim raster is the free variable.
Introduce a second, independent knob: **`world_scale` S**, one factor that
multiplies every length in the world — sprite tile, field cell, body extent,
sense radius, speed in px/s, stamp and deposit radii. Choose `S = 6/k` for an
upscale factor `k`.

| raster | upscale `k` | `S` | sprite tile | cell px | field cells | raster px | × cube | render ∝ |
|---|---|---|---|---|---|---|---|---|
| 320×180 | 6 | 1 | 16 | 4 | 3,600 | 57,600 | 2.81× | 1.0 |
| 480×270 | 4 | 1.5 | 24 | 6 | 3,600 | 129,600 | 6.33× | 2.25 |
| **640×360** | **3** | **2** | **32** | **8** | **3,600** | **230,400** | **11.25×** | **4.0** |
| 960×540 | 2 | 3 | 48 | 12 | 3,600 | 518,400 | 25.3× | 9.0 |
| 1920×1080 | 1 | 6 | 96 | 24 | 3,600 | 2,073,600 | 101× | 36.0 |

Every row divides exactly: `w/(4S) = 80`, `h/(4S) = 45`. Resolution then buys
pixels, not ecology — **but only under three conditions**, and the first draft
stated the conclusion without them:

1. **Noise scale is handled by the cylinder radius**, not by a divisor in the
   sampler: `r = w/(2π·32·S)` makes arc length per pixel `1/(32S)` at every S
   (§5a), so the patch count per cell is S-invariant without touching
   `habitat.rs:43-75`.
2. **Radii already expressed in cells stay invariant, and that is checkable:**
   `sense_depth` is `ceil(r_sense / CELL_PIXELS)`
   (`world/lifecycle.rs:383-389`), so with `r_sense = 6S` and `cell = 4S` it is 2
   hops at every S. Any radius still written in bare pixels must be moved onto S.
3. `cell_count()` stays 3,600 and inside the `u16` `CellId` bound (§2), and the
   two-image rule `w >= 2·max_local_radius() + 2·CELL_PIXELS` holds (§2).

What scales with what: **with cells** (constant here) — diffusion, water flow,
rain, habitat, detritus fall, `RenderView` vectors, snapshot payload. **With
organisms** (constant here) — the O(n²) pair pass, controller, motor, neural.
**With raster area** — the ground/ramp/water/rain passes *and* the per-organism
stamps, since a body extent of `9·S` px covers `∝ S²` pixels. Hence:
**render ∝ S², tick ≈ constant.**

**The board, corrected (measured from sysfs, 2026-09-16).** Eight online cores,
not six A55: `cpu0-3` Cortex-A55 (MIDR `0x412fd050`) at 1.96 GHz, `cpu4-6`
Cortex-A78 (`0x411fd411`) at 2.40 GHz, `cpu7` Cortex-A78 at 2.71 GHz. The earlier
"6 usable A55" in the tachyon-screen handoff came from `nproc` under a restricted
affinity with `lscpu` naming only the first core. Everything below therefore
budgets against the **A78 cluster**, and `R` means *one A78 core at 2.40 GHz*.
**Assumption pending FW-0's measurement:** an A78 at 2.40 GHz is roughly 2.5–3×
an A55 per core on this scalar `f64` pixel-and-field code (out-of-order versus
in-order, wider issue), so an `R` measured on an A55 divides by ~2.5–3.

Recommended placement, to be confirmed by FW-0: the host loop on `cpu7` (the
2.71 GHz core), the parallel presenter across `cpu4-6` plus `cpu7` when it is in
its render phase, and the screen shim plus the OS on the four A55s — the shim's
integer block upscale to 1920×1080 XRGB is memory-bound and does not need a big
core. The board also carries an Adreno GPU and a Hexagon NPU. **Everything in
this section is CPU-only, which is the scope of FW-1..FW-8; the Adreno is the
subject of FW-9, the approved GPU follow-on, and not part of the first version**
(the Hexagon NPU stays unspent — Wrysk may want it for organism networks or voice
later).

**The budget is shared, not split.** `Step::Tick` and `Step::Render` alternate on
one thread in one loop (`crates/cubarium/src/runner/mod.rs:733-801`), so the real
constraint per wall second is

> `20 · tick_ms + fps · render_ms <= 1000 ms`

with `render_ms ≈ 2.81 · S² · R` for a measured one-A78-core cube render `R`. At
the architecture doc's `tick_ms = 20` target the tick alone takes 400 ms, leaving
600 ms, so `R_max = 600 / (fps · 2.81 · S²)`. The parallel columns assume the
presenter split over the four A78 cores, worth ~3.5× after split and join:

| S | 60 fps, one A78 | 30 fps, one A78 | 60 fps, 4×A78 (×3.5) | 30 fps, 4×A78 |
|---|---|---|---|---|
| 1 | `R ≤ 3.6 ms` | `R ≤ 7.1 ms` | `R ≤ 12.5 ms` | `R ≤ 24.9 ms` |
| 1.5 | `R ≤ 1.6 ms` | `R ≤ 3.2 ms` | `R ≤ 5.5 ms` | `R ≤ 11.1 ms` |
| **2** | `R ≤ 0.9 ms` | `R ≤ 1.8 ms` | `R ≤ 3.1 ms` | `R ≤ 6.2 ms` |
| 3 | `R ≤ 0.40 ms` | `R ≤ 0.79 ms` | `R ≤ 1.4 ms` | `R ≤ 2.8 ms` |
| 6 | `R ≤ 0.10 ms` | `R ≤ 0.20 ms` | `R ≤ 0.35 ms` | `R ≤ 0.69 ms` |

The corrected core count helps in exactly one place — the parallel columns are
now four *big* cores rather than four in-order A55s, which is what makes S = 2
plausible at all. It does not change the serial columns, because they were always
per-core, and it does not change the shape of the trade.

Two consequences the shared loop makes visible. A slower tick eats the render
budget directly: at `tick_ms = 40` only 200 ms remains and every figure above
divides by three, so **the sim rate is not free after all** — dropping the sim to
10 Hz would buy back 200 ms, at the cost of the 20 Hz contract in
`design/architecture.md`. And parallelising the presenter is worth more than any
other knob, because it is the only term that can leave the main thread; it writes
disjoint pixels off the sim's critical path, so a deterministic row-band split is
sound.

**640×360 at S = 2 is the recommended candidate, explicitly gated.** Its case:
3× is an integer upscale so the pixel-art grid stays visible; S = 2 is an integer
art scale, so every 4-px and 16-px constant doubles exactly (4→8, 16→32, tile
rows 10/15→20/30) where S = 1.5 would round; creatures read ~32 world px ≈ 96
device px. But the table above shows it needs `R ≤ 3.1 ms` even with the presenter on all
four A78 cores, and **no `R` or `tick_ms` has been measured on the board** — the
A78-versus-A55 ratio above is an assumption, not a measurement. FW-0 must produce
both, pinned, before the value is fixed; 480×270 and 30 fps are the documented
fallbacks and are config edits, not rewrites. The *contract* carries S from FW-1
either way (§2), so only the number moves.

**What FW-9 does, and does not do, to this trade.** FW-9 moves the *display
daemon's* gather to the Adreno: the shim renders surfaceless into a dma-buf
imported into KMS and does the integer upscale, the rotation and any
panel-resolution post-effects there. That removes the gather from the CPU budget
entirely — worth real A55 time, and it is what makes shaded effects affordable at
1920×1080 at all. What it does **not** do is offload cubarium's presenter — it
does extend cubarium's side of the wire, adding the auxiliary layers to the raster
encoder and a layer id to the strip format (see its row in §9). The presenter
still rasterizes the `w×h` world raster on the CPU, so `render ∝ S²` and the
shared-loop budget above are unchanged, and **S stays gated on FW-0's measured
numbers exactly as written**. A higher S becomes arguable only if the presenter's
own background passes — ground, ramps, water, rain, stamps — also move to the GPU,
which is explicitly outside FW-9's scope and is the "longer-term shaded renderer"
rather than this plan.

### FW-0 measurements (2026-09-16)

All four numbers, measured. Every figure below is a median over 600 render
samples or 200 tick samples from
`crates/cubarium/examples/render_bench.rs`, on a world built from
`WorldConfig::default()` with `seed = 1` and stepped 3,000 ticks headless
first (population 24), drawn through the shipped `assets/atelier` pack —
which is the presentation the ring world inherits (§7), not the plain M2
discs. Pinning is the bench's own `--pin`, which reports the mask it
obtained; `taskset` could not be used on the big cores, see the note below.

| quantity | pinning | median | p95 |
|---|---|---|---|
| **`R` = `presenter.draw` + `canvas.encode`, one cube frame** | **cpu5, A78 2.40 GHz** | **15.28 ms** | **15.56 ms** |
| — of which `draw` | cpu5 | 13.90 ms | 14.18 ms |
| — of which `encode` | cpu5 | 1.37 ms | 1.47 ms |
| `R`, same world at 12,000 ticks (population 65) | cpu5 | 13.52 ms | 13.80 ms |
| `R` on the prime core | cpu7, A78 2.71 GHz | 14.12 ms | 14.22 ms |
| `R` on a little core | cpu0, A55 1.96 GHz | 74.00 ms | 75.00 ms |
| `R` on the desktop, for scale | Ryzen 9 9950X3D, one core | 8.76 ms | 8.85 ms |
| `R` with the plain M2 presenter | cpu5 | 3.13 ms | 3.20 ms |
| **`tick_ms` as the loop pays it** (`step` + `render_view` + `observe`) | **cpu5** | **0.534 ms** | **0.563 ms** |
| — `World::step` alone | cpu5 | 0.316 ms | 0.348 ms |
| `tick_ms` at population 65 | cpu5 | 0.599 ms | 0.641 ms |
| **`R` across four A78 cores** | — | **pending FW-3** | — |

**The four-core entry is deliberately empty.** A row-band split needs FW-3's
deterministic hook: `Canvas` exposes no band and `ArtPresenter::draw` takes
`&mut self`, so nothing the bench could time today is work the run loop
would ever execute. The parallel columns below therefore stay an assumption,
and every selection here is provisional on the serial numbers.

**Two of the section's own assumptions were wrong.**

- *The A78/A55 ratio is 4.8×, not 2.5–3×* (74.00 / 15.28). The little cores
  are further behind on this code than the section guessed.
- *`tick_ms` is 0.53 ms, not the architecture doc's 20 ms target.* The tick
  term is 10.7 ms per wall second, not 400, so the render budget is **988 ms**,
  not 600. That is a 1.65× gift, and it is not nearly enough.

**The real runs, on the board, with the display daemon left running.**

| run | result |
|---|---|
| `--sink none --seconds 120`, `taskset -c 4-7` | 2,400 ticks in 120.03 s = **19.99 ticks/s**, 1× real time |
| `--sink none --speed 0 --seconds 600`, `taskset -c 5` | 12,000 ticks in 4.11 s = **2,920 ticks/s**, 146× real time |
| `--art assets/atelier --sink shim --addr 127.0.0.1:7392 --fps 60 --seconds 120`, `taskset -c 4-7` | 7,196 frames in 120.22 s = **59.86 fps**, 2,400 ticks = 19.96 ticks/s, 0 coalesced, 0 errors, no lag lines |
| the daemon during that run | presented 60.4 fps, received 60.0 fps, **stale 0, bad 0**, its own gather 8.1 ms avg / 11.3–16.3 ms max |
| `top` during that run | `cubarium` 85–98 % of one core; whole board 17–19 % user, 3–7 % sys, ~76 % idle |
| the same at `--fps 120 --seconds 30` | 1,934 frames in 30.22 s = **64.0 fps**, the loop's ceiling |

The last row is the check that matters: asked for 120 fps the loop saturates
at 64.0, which back-solves to `R = (1000 − 10.7)/64.0 = 15.45 ms`. That is
the bench's 15.28 ms from a completely independent measurement, so both the
figure and the shared-loop budget equation are confirmed against a real run
rather than assumed.

**The gate, recomputed against the measured numbers.** With
`20 · tick_ms = 10.7 ms` the render budget is 988 ms per wall second, so
`R_max = 988 / (fps · 2.81 · S²)`:

| S | raster | 60 fps | 30 fps | 20 fps |
|---|---|---|---|---|
| 1 | 320×180 | `R ≤ 5.9 ms` | `R ≤ 11.7 ms` | **`R ≤ 17.6 ms`** |
| 1.5 | 480×270 | `R ≤ 2.6 ms` | `R ≤ 5.2 ms` | `R ≤ 7.8 ms` |
| **2** | **640×360** | `R ≤ 1.5 ms` | `R ≤ 2.9 ms` | `R ≤ 4.4 ms` |
| 3 | 960×540 | `R ≤ 0.65 ms` | `R ≤ 1.3 ms` | `R ≤ 2.0 ms` |
| 6 | 1920×1080 | `R ≤ 0.16 ms` | `R ≤ 0.33 ms` | `R ≤ 0.49 ms` |

**Measured `R` is 15.28 ms.** Exactly one cell in that table admits it.

**Selection (provisional on the serial numbers): S = 1, 320×180, `--fps 20`.**
The arithmetic: `20 · 0.534 + 20 · 2.81 · 1² · 15.284 = 10.7 + 859.0 =
869.7 ms ≤ 1000 ms`, 87 % of the wall second. The ceiling at S = 1 is
`(1000 − 10.7) / 42.95 = 23.0 fps`, so 30 fps at 320×180 misses by 30 %
(1,299 ms). **S = 2 at 60 fps, the section's recommended candidate, is over
budget by 10.4×**; the documented 480×270-at-30-fps fallback is over by 3.0×.

**What a perfect four-core split would and would not buy.** At the section's
assumed ×3.5 the effective `R` is 4.37 ms, which admits S = 1 at 60 fps
(5.9 ms), S = 1.5 at 30 fps (5.2 ms) — and still **fails S = 2 at 30 fps**
(2.9 ms) by 1.5×. So FW-3's hook, even at its assumed efficiency, does not
reach 640×360. The number to rerun this table against is the one FW-3
measures, not this one.

**And the board is not the problem.** One A78 at 2.40 GHz is 1.74× slower
than a Zen 5 core on this code (15.28 vs 8.76 ms), so a cube frame costs
8.8 ms on the fastest desktop core available. `R` is what the presenter
costs, not what the Tachyon costs, and the plain M2 presenter draws the same
frame in 3.13 ms — 4.9× cheaper — which places the cost squarely in
`ArtPresenter::draw` (13.90 of the 15.28 ms) rather than in the sRGB encode
(1.37 ms). Three consequences worth stating plainly, none of them decided
here:

1. **FW-9 does not rescue this.** Its own scope paragraph says the presenter
   keeps rasterizing the `w×h` world raster on the CPU; it moves the
   *daemon's* gather, which these runs measure separately at 8.1 ms avg and
   which is not in cubarium's budget at all.
2. The four-core split is now the difference between 20 fps and 60 fps at
   S = 1, not a nice-to-have. FW-3's hook should be treated as required.
3. Reaching S = 2 at all needs the per-frame cost of `ArtPresenter::draw`
   to fall by roughly an order of magnitude, which is a renderer decision
   this measurement does not make.

**A device note for every later package.** `taskset -c 7` and `taskset -c 4`
fail with `EINVAL` on an idle Tachyon and this is not a permissions problem:
the board's `core_ctl` driver *isolates* idle big cores
(`/sys/devices/system/cpu/cpu7/isolate` reads `1`), and the scheduler then
refuses an affinity mask that names only isolated cores. Loading the machine
brings them back. `render_bench --pin` does exactly that and prints the mask
it got; `taskset -c 4-7` always succeeds because cpu5 and cpu6 stay
un-isolated, and the full mask is retained, so the run uses cpu4 and cpu7
once load brings them back.

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

**A cheaper stage exists before any re-bake.** `stamp_sprite`/`stamp_pose`
already take a `scale: f64` (`cubarium-render/src/sprite.rs:317-326`), so an
integer nearest-neighbour `scale = 2.0` path can draw the existing pack v5 at
S = 2 with no new assets — call it **Stage A0**, and it is the fastest way to see
a ring world at 640×360. It is blocked by one hard gate: `FOOTPRINT_RADIUS = 9.0`
is checked both when a sprite is built (`sprite.rs:18-22,61-64`, which returns
`Err("sprite extent … exceeds the 9-pixel surface budget")`) and again at stamp
time as `extent * scale > FOOTPRINT_RADIUS`, which **silently draws nothing**
(`sprite.rs:810-818`). At `scale = 2` every creature disappears. The constant is
documented as "not review-tunable — it is the radius the shared unfolding is
proven correct for", so it must become `9.0 · world_scale`, defined in FW-1's
`Scale` and validated against the **topology's own** `max_local_radius()` (§2) —
`min(h, w − 2·CELL_PIXELS)/2` on a ring, and the untouched cube proof of 32 px on
a cube, where `S` is pinned to 1 anyway. **That work lives in `cubarium-render` and therefore in FW-3**, which
owns that crate; FW-7 consumes the interface and must not edit it. The first
draft's FW-7 omitted the render crate entirely — that was the gap.

**The honest finding about the art itself.** Raising `svg/scale` in the `.import`
files and `TILE` in the baker produces sprites that are S× *bigger blocks of the
same art* — no new detail, because the sources are already pixel art at 1 unit =
1 pixel. Genuine higher-resolution artwork means redrawing those 148 SVG paths on
an S× finer grid. Each file is small (a handful of `<path>` elements), but it is
authoring work, not a bake parameter. The staging that follows:

- **Stage A0 (no assets):** `scale = 2.0` on pack v5 with a scaled footprint
  budget. Proves the ring world at 640×360 before any Godot run.
- **Stage A (code, mechanical):** bake at `TILE = 16·S`, `GROUND_TILE = 8·S`,
  `svg/scale = S`; `pack.json` v6 makes `tile`, `ground_tile` and `pivot` data
  the loader honours instead of asserting; every hard `16`/`8`/`4` in `art.rs`,
  `art_present/tall.rs` and `lanternjaw/raster.rs` becomes `tile`-relative. The
  picture is unchanged in shape, only larger. This unblocks everything else.
- **Stage B (authoring, incremental):** redraw parts at the finer grid,
  file by file. The runtime reads the same pack, so art can improve one creature
  or one plant at a time after the ring world is already on the panel.

**One factor can drive the code side.** Put `world_scale: f64` in `WorldConfig`
and have the *fresh-world default builder* multiply the length-dimensioned
defaults: `organism.speed_max 5.0` px/s, `organism.sense_radius 6.0`,
`organism.body_extent_max 9.0`, `drives.birth_offset_px 2.5`
(`crates/cubarium-core/src/config.rs:551,554,580,609`), `CELL_PIXELS 4.0`
(`cubarium-surface/src/field.rs:14`), plus the deposit and care radii. Pace stays
**1 BL/s** by construction: body length and px/s scale together, so the
calibration recorded on 2026-09-14 is preserved exactly.

**`MAX_LOCAL_RADIUS` is not on that list.** It is per topology and never scaled
(§2): the cube's 32 px is a completeness proof tied to `MAX_SEAMS`
(`unfold.rs:5-13`) and `Topology::Cube` pins `S = 1`, so nothing multiplies it.
A ring has one seam and needs no path enumeration, so no radius is *meaningless*
there; `Ring::max_local_radius() = min(h, w − 2·CELL_PIXELS)/2` is a **chosen
performance cap** — `unfold_pixels` cost grows with the square of the radius — that
also keeps the image count at two, and the validation `9·S <= max_local_radius()`
is what ties the stamp budget to it. The two call sites that today validate config radii
against the constant (`config.rs:863,917`) take the topology's value instead.

One caution remains: scale the *defaults*, never a value read from a TOML file,
or an explicit config gets scaled twice.

## 8. Biome and terrain variation (separable)

What exists already: three height bands and a light/moisture gradient
(`design/stratified-world.md`); patch noise on the embedding for light and
moisture and a basin noise for pools (`cubarium-core/src/habitat.rs:63-104`);
moving moisture blobs on 20–47 minute periods driving rain
(`design/water.md`); downhill detritus fall and gravity-driven flow; and a
stimulus envelope for external `Light | Moisture | Nutrient | Flow` events
(`design/environmental-inputs.md`). On a ring all of that survives and
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

This is FW-8. The first ring world ships without it.

## 9. The plan

Re-cut twice. **FW-1** owns cell geometry, `world_scale` and every derived bound
(including the *value* of the stamp budget); **FW-2** owns the ecological calls
now written down in §5/§5a; **FW-3** owns *adopting* the budget in the render
crate; **FW-4** owns the care chain end to end. "Files" are exclusive after the
FW-6 reservation below.

**FW-6 reserves these exact paths**, and every implementation package's glob
excludes `tests/ring_*.rs`:
`cubarium-surface/tests/{ring_travel,ring_field,ring_raster}.rs`;
`cubarium-core/tests/{ring_world,ring_weather,ring_schema17}.rs`;
`cubarium-render/tests/{ring_canvas,ring_stamp_scale}.rs`;
`cubarium/tests/{ring_sinks,ring_present,ring_care}.rs`.

| id | objective | owns (decision) | files (exclusive; no `tests/ring_*.rs`) | interface exposed | verification | size | effort |
|---|---|---|---|---|---|---|---|
| FW-0 | Vendor `cube-proto` with `Raster` + wire format 2. **Measure, pinned: serial `R` and `tick_ms` now; the four-A78 render number re-measured after FW-3** (the parallel presenter does not exist until then), plus end-to-end fps and ticks/s. **S is provisional until that rerun** | the S and `--fps` values in §6 | `vendor/cube-proto/**`, `vendor/cube-proto.rev` | `Raster { width, height, data }` | `cargo test --workspace`; each number recorded with its pinning and its date, and the S it selects | small | medium |
| FW-1 | `Topology` + `Scale`: cell pixels, `world_scale` (ring-only; cube pinned to 1), `footprint_radius() = 9·S`, per-topology `max_local_radius()` and `chord_sq()` (ring: `min(|Δu|, w−|Δu|)² + Δv²`); `u16` pixel indices; runtime cell count with the `u16::MAX` and two-image checks; **the ring's self-seam through the existing `Some(seam)` branch**, rims at `v = 0` and `v = h` through the existing `REFLECT_Y`; ring `downhill` (`cy == 0` ⇒ `None`); the isotropic cylinder `embed()` and `height()` as separate methods | the whole geometry contract and **the stamp-budget value** | `crates/cubarium-surface/**` | §2's API; `CUBE_CELL_COUNT` | cube results identical by value — **the surface crate's own by-value tests are FW-1's evidence; the projection comparison starts at FW-2**; ring exercised at **S = 1 and S = 2 from the first commit**; **exact-tie and near-tie fixtures at all four corners**, whose resolutions differ (`Edge::Top = 0 < Right = 1 < Bottom = 2 < Left = 3`, `travel.rs:50-70`): top-left, top-right and bottom-left reflect first, bottom-right crosses the seam first | large | **high** |
| FW-2 | Topology and scale through the world: config, schema 17, extended `WorldState::validate` (runtime `cell_count()` at `care.rs:340,345-349`), height/`up`, both controllers, the decided canopy `downhill`, the cylinder embedding fed to habitat and **weather unchanged**, founders, **and both cube-hardcoded core resolvers: `CareTarget::resolve` (`care.rs:148-168`, used at `world/care.rs:60`) and `HunterTarget::resolve` for `SpawnApex` (`hunter/state.rs:47-67`) — core types with `f64` `u`/`v`, distinct from FW-4's host `CareTarget`**, **and the `CubeProjection` type, the frozen `v16.rs` mirror, the exporter and the comparator** — they live here because `WorldState`, `snapshot`, `decode_exact` and the mirrors are all in `cubarium-core` (`world/state.rs:22`, `snapshot.rs:7,101`) and the dependency runs core → surface | the ecological calls as written in §5/§5a | `crates/cubarium-core/**` | `WorldConfig.{topology,world_scale}`, `World::topology()`, `RenderView.topology`, `CubeProjection` + the exporter | cube run equal by `CubeProjection` (the first package where that evidence exists); RNG stream parity (expected exact — no draw changes); **feed/rain/clean/apex targets beyond pixel 63 resolve on a ring and are refused off-world**; ring run reaches steady state | large | **high** |
| FW-3 | `Canvas` by topology, `Canvas::pixels()`, `encode_raster`; **adopt** `Scale::footprint_radius()` at both check sites (`sprite.rs:61-64,810-818`) and the `scale` stamp path; port `field/trail/sprite/body/multipart`; deterministic row-band parallel hook | adoption only — the value is FW-1's | `crates/cubarium-render/**` | `Canvas::new(topo)`, `pixels()`, `encode_raster` | same-seed cube canvas bit-identical; a `scale = 2` stamp draws instead of vanishing | medium | **high** |
| FW-4 | `Output` enum + sinks (shim/png/web), viewer ring mode, CLI/config, preview refusal, **the whole care chain: `CareTarget.{u,v}` widened to `u16` and validated against the topology extent (`care/mod.rs:122-140`), `PlannedCommand` journal and web-request compatibility (`care/mod.rs:146-162`), and the canvas flourish** | the care wire and journal shape | `crates/cubarium/src/{sink/**,cli.rs,net.rs,run.rs,runner/**,care/**,care_effects.rs}`, `sink/web/index.html` | `enum Output`, `topology`/`world_scale` TOML, the widened `CareTarget` | ring PNG capture; viewer screenshot; a journal written before the widening still replays; the measured split that picks `--fps` | medium | medium |
| FW-5 | Presenter for the ring: `RenderView.topology` consumed, `ArtPresenter` built from the world's cell count, bands, horizon, water/rain, motifs, columns, bodies | presenter cache lifetime | `crates/cubarium/src/{present.rs,art_present/**,lanternjaw/**,scene.rs}` | — | ring capture reviewed by Wrysk; cube capture diffed to zero | large | **high** |
| FW-6 | Independent test authoring at the reserved paths: the ring self-seam and both rims, **exact and near ties at all four corners** in `ring_travel.rs`, two-image unfolding, ring-row field flux, capacity and two-image refusals, extended `validate`, the cylinder embedding's wrap continuity, and in `ring_weather.rs` **the cap aspect at the rims measured and recorded, not asserted round**, **feed/rain/clean/apex targets beyond pixel 63**, care widening, sink/raster, presenter goldens, and **the `CubeProjection` negative tests of §4 in `ring_schema17.rs`** | — | only the reserved `tests/ring_*.rs` paths above | — | written without reading FW-1..FW-5's own tests; every negative test shown to fail on a perturbation | medium | **high** |
| FW-7 | Pack v6 (`tile` as data), baker at `TILE = 16·S`, `art.rs`/`tall.rs`/`lanternjaw` constants made tile-relative, S-scaled default builder; re-bake at S = 2 | — | `art/**`, `assets/atelier/**`, `crates/cubarium/src/art.rs` | `pack.json` v6 | S = 1 pack still loads and renders bit-identically; 640×360 capture; reproducible Godot bake | large | **high** |
| FW-8 | Biomes: region field, four parameter sets, `mechanisms.biomes` off by default, presentation by dominant biome | biome parameter sets | `crates/cubarium-core/src/biome.rs` (new), `crates/cubarium/src/art_present/habitat.rs` | `HabitatConfig.biomes` | toggle off ⇒ `CubeProjection` unchanged; toggle on ⇒ short run showing distinct regions | medium | medium |
| FW-9 | **GPU hybrid (approved follow-on, not the first version).** Sim and sprite stamping stay on the CPU at world resolution; the display daemon renders on the Adreno — EGL surfaceless into a dma-buf imported into KMS — and does the integer upscale, the rotation and panel-resolution post-effects there. The CPU gather remains the fallback, chosen per display | the renderer choice per display | the `led-cube-shim` repo's `cube-screen-shim` and `cube-proto` crates, plus cubarium's `sink/` raster encoder for the auxiliary layers | **auxiliary layers beside RGB in the raster strip format** — candidates: emissive, water mask, height/stratum, rain — with a layer id in the strip header; **FW-4 publishes which layers cubarium emits** | byte-identical output to the CPU gather with no effects enabled; measured ms/frame on the GPU path | large | **high** |

FW-5 and FW-7 both touch `art_present/tall.rs` and `lanternjaw/**`; FW-7 runs
after FW-5 and the table gives those paths to FW-5, with FW-7 editing them only in
its own window. FW-8's presenter file is likewise sequenced after FW-5. FW-9 touches neither
repo's presenter files and is independent of FW-7 and FW-8; its only cubarium
surface is the raster encoder, whose layer set FW-4 declares.

**Ordering.** FW-0 any time. FW-1 first, **and its API is frozen and published
before anything else starts** — the freeze is the gate, not the merge. Then
FW-2 ∥ FW-3 (disjoint crates). Then FW-4 ∥ FW-5 (disjoint file sets), only after
FW-3 publishes `encode_raster`. FW-6 starts at the FW-1 freeze and writes only its
reserved paths. FW-7 follows FW-5; FW-8 last. **FW-9 also follows FW-5 and is
independent of FW-7 and FW-8**, so it can run beside either. W2 (the device)
follows FW-7.

**The scale staging.** Scale lives in the FW-1 contract and is exercised at S = 2
there, so nothing downstream ever changes geometry. What stages is the chosen
*value*: FW-1..FW-6 can ship a ring world at S = 1 on pack v5, FW-3's Stage A0 can
show S = 2 on the same pack, and FW-7 makes S = 2 the shipped default with
re-baked art. If FW-7 slips the panel still works.

Seven packages are high effort: FW-1 (a reflection, tie or bound mistake is silent
and corrupts motion), FW-2 (the weather model and the validate extension), FW-3
(the 9-px stamp budget is a proven-correctness bound shared with `unfold_pixels`),
FW-5 (new UI construction), FW-6 (test authoring is high by the working rules),
FW-7 (`tall.rs`'s named row indices), FW-9 (EGL/dma-buf/KMS interop plus a wire
format change, with a byte-identical fallback to hold).

**Standing evidence at every package:** `cargo test --workspace` green, a
fixed-seed `cubarium run --fresh --seed 1 --speed 0 --seconds 120 --sink png`
capture whose PNG bytes match the pre-change run, and — **from FW-2 onwards, which
is where the projection can exist** — **`CubeProjection` equality** against a
fixture captured from `main` before FW-1 starts. FW-1's own evidence is the
surface crate's by-value tests: it cannot build the projection, because
`cubarium-surface` sits *below* `cubarium-core` in the dependency graph and
`WorldState`, `snapshot` and `decode_exact` all live in core
(`world/state.rs:22`, `snapshot.rs:7,101`). Not `ecology_hash`, which cannot be
equal across a config change (`snapshot.rs:193-199`) and cannot even be computed
across the schema refusal.

**Test authoring is its own pass.** FW-6 is that pass at high effort, written
against §2's contract and §5/§5a's decisions rather than against the
implementation. FW-7 and FW-8 each need their own small authoring pass.

## What this audit could not determine

- **Device performance.** No build was run (the shared `target/` belongs to the
  main checkout, and a worktree build would cost tens of GiB) and the Tachyon is
  not reachable from this task. §6 gives the scaling law and a decision table
  keyed on a measured `R` and `tick_ms`; FW-0 must produce both, pinned to an A78
  core, before S is fixed. The 2.5–3× A78:A55 per-core ratio is an assumption.
- **The exact `Raster` API.** W1b had not landed; the vendored `cube-proto` at
  `7a21b5f` has no `Raster` and no format 2. FW-0 may need a small adaptation.
- **How much art authoring Stage B really is.** 148 SVG part files were counted
  and one was read; the effort to redraw them at 2× is a judgement for whoever
  draws them, not something this audit can size.
- **Whether 640×360 reads well.** That is Wrysk's call on the panel. The raster
  and `world_scale` are both config precisely so it can change without a code
  edit.

## Review repair 1 (Astra, 2026-09-16)

Astra returned *rework*. Every finding is accepted; none is rebutted. Three were
blocking and two of those were substantive errors of fact in the first draft.

**This section records what round one changed, not the current state.** Its
item 4 left planar weather and the canopy drain as deferred design calls and its
item 6 quoted an `S ≤ 3.5` bound; both are **superseded by Review repair 2**,
which decides the two calls (§5, §5a) and deletes the bound.

| # | finding | verdict | what changed |
|---|---|---|---|
| 1 | hub audit incomplete: `World.images` five-element cache, `RenderView` carries no topology, `ArtPresenter` initialises through global `CellId::all()` | **accepted** | §1 gains a fifth hub row citing `world/mod.rs:44-53`, `view.rs:34-61`, `art_present/mod.rs:416-423`; §2 adds `World`'s five `Box<[f64; CELL_COUNT]>` caches to the runtime-sizing work; FW-2 exposes `RenderView.topology`; FW-5 owns building `ArtPresenter` from the world's cell count |
| 2 | `CellId(u16)` caps a flat world at 65,535 cells; `CUBE_CELL_COUNT` claim overstated | **accepted** | §2 adds `Topology::validate()` with checked multiplication and a `u16::MAX` refusal (1920×1080 at 4-px cells would want 129,600), carried into FW-1 and into §4's post-decode checks. The mitigation claim is rewritten: the const preserves cube-only *literals* as a rename, not callers that iterate runtime-length vectors — `redesign_rules.rs:116,418` is cited as the counter-example |
| 3 | **BLOCKING** — flat reflection cannot be `TangentMap::IDENTITY`; also "four border rows degree 3" is wrong | **accepted; the draft was wrong** | §2 replaces the identity claim with the per-axis algebra table: `REFLECT_Y` on horizontal walls, `REFLECT_X` on vertical, both at a corner (`= quarter_turns(2)`, commuting, `det = +1`), citing `travel.rs:254-258`. The cube's lowest-`Edge` tie rule is documented as *inverted* for flat (a tie applies both walls, not one seam); exact-tie versus near-corner equivalence, on-wall progress, `MAX_CROSSINGS` and the counted `nudge_inward` fallback are named as required tests. Degrees corrected to 3,354 × degree 4, 242 × degree 3, **4 corners × degree 2** (`field.rs:98-119`), and the edge count corrected from a bogus 7,115 to **7,075** = `79·45 + 80·44` |
| 4 | **BLOCKING** — height audit missed consumers; spherical weather; canopy drain; unvalidated 0.67 | **accepted; the draft was incomplete** | §5 is rebuilt. Added: both controllers reading `o.pos.embed()[1]` (`step.rs:449-460`, `3068-3079`), `up_direction(face)` (`lifecycle.rs:376-379`), and depth-preference steering (`controller.rs:228-235`) — with the note that a wrong height steers the whole population into a wall. The weather metric is now a named **BLOCKING design call**: `normalize([u/32,v/32,0])` gives a polar fan, not moving weather (`habitat.rs:198-225`), so a planar metric with a stated edge rule is required, owned by FW-2. The canopy drain is a named decision: the cube's level Top has no downhill neighbour by construction (`field.rs:135-151`) while a flat canopy drains completely — recommend *accept and measure*, owned by FW-2. `canopy_top = 0.67` is relabelled an explicit new default for review, not a derived value |
| 5 | constant-cost argument is conditional; render and sim share one loop; 640×360 not yet defensible | **accepted** | §6 states the three conditions the invariance depends on, the first being that habitat noise must be sampled at `/(32·S)` (`habitat.rs:43-75`) or patchiness per cell changes with S; cites `sense_depth`'s `r_sense / CELL_PIXELS` (`lifecycle.rs:383-389`) as the checkable invariant. The two independent budgets are replaced by the shared-loop budget `20·tick_ms + fps·render_ms <= 1000 ms` (`runner/mod.rs:733-801`) and an `R_max` table derived from it, with the consequence that a slower tick eats the render budget. 640×360 is relabelled a **gated candidate** pending FW-0's measured `R` and `tick_ms` |
| 6 | cheapest Stage A missed; 2× assets fail the nine-pixel budget; FW-7 must include `cubarium-render` | **accepted** | §7 adds **Stage A0**: `stamp_sprite`'s existing `scale` parameter (`sprite.rs:317-326`) draws pack v5 at S = 2 with no re-bake — blocked by `FOOTPRINT_RADIUS = 9.0`, which rejects at build (`sprite.rs:18-22,61-64`) and *silently draws nothing* at stamp time (`sprite.rs:810-818`). The budget becomes `9·S`, validated against `max_local_radius()` (`S ≤ 3.5` today). Placement note: the work is in `cubarium-render`, so it is given to **FW-3**, whose effort is raised to high, rather than to FW-7 — one owner per crate keeps the parallel pairs write-disjoint, and FW-7 consumes the published interface |
| 7 | **BLOCKING** — "S=1 then S=2 without touching topology" is impossible | **accepted; the draft was wrong** | Cell size is a surface constant (`field.rs:11-16,49-68`), so FW-7 could not have delivered `4·S`. `Scale` (cell pixels, world scale, footprint and local radii) moves into **FW-1's contract**, which exercises S = 2 from its first commit while keeping S = 1 cube goldens. What stages is now the chosen *value*, not the geometry. §9 also states that parallel pairs start at the **interface freeze**, not the merge |
| 8 | `ScalarField` is not a serialization cause; don't widen the fixed header; `ecology_hash` equality impossible | **accepted; the draft was wrong** | §4 is rewritten. `Fields` is already `Vec<f64>` (`fields.rs:19-32`) and the fixed arrays live in `Habitat`/`World`, which are rebuilt on load — the bump's real cause is `WorldConfig` gaining fields inside the payload (`world/state.rs:22-28`). The header widening is dropped, because `scripts/reduce-quiet-compare.mjs:112-126` parses the 22-byte layout at hardcoded offsets; validation moves after decode (topology, dimensions, capacity, every field-vector length, every organism and care `face`). The pre/post `ecology_hash` check is replaced everywhere by a **normalized cube-state comparator** that excludes `config`, because `ecology_hash` hashes the whole masked state including it (`snapshot.rs:193-199`) |

Two further corrections not in the findings, folded into the same pass:

- **Device facts.** The Tachyon has **8 online cores**, not six A55: `cpu0-3`
  Cortex-A55 at 1.96 GHz, `cpu4-6` Cortex-A78 at 2.40 GHz, `cpu7` Cortex-A78 at
  2.71 GHz (sysfs, 2026-09-16; the earlier figure came from `nproc` under a
  restricted affinity). §6 now budgets against the A78 cluster, defines `R` as
  one A78 core at 2.40 GHz, states the 2.5–3× A78:A55 per-core ratio as an
  assumption pending FW-0, and proposes a placement (host loop on `cpu7`,
  presenter across `cpu4-7`, shim and OS on the A55s). The Adreno GPU and Hexagon
  NPU are explicitly out of scope for the first version.
- **Package count and effort.** Six packages are now high effort rather than
  four: FW-3 was raised because the 9-pixel stamp budget is a proven-correctness
  bound shared with `unfold_pixels`, not a tunable, and FW-6 was already high by
  the working rules but had not been counted.

## Review repair 2 (Astra, 2026-09-16)

Round one's findings 1–3 are confirmed closed. Round two raised two blocking and
four should-fix items; all six are accepted and none is rebutted.

**This section records what round two changed, not the current state.** Its
item 2 specified a *planar* weather model and item 4 described a rectangle with
four walls; both are **superseded by Review repair 3**, in which Wrysk made the
world a ring — the weather model reverts to the unchanged spherical one on a
cylinder embedding (§5a) and the walls on the `u` axis become a seam (§2).

| # | finding | verdict | what changed |
|---|---|---|---|
| 2 | **BLOCKING** — prior finding 4 not closed: the body still *deferred* planar weather and canopy drainage instead of deciding them; existing weather persists 3D centre, axis and angular rate with orbital plus random-walk semantics (`habitat.rs:110-196`, `config.rs:249-262`) | **accepted; both decisions adopted** | **(a) Canopy decided, no toggle:** `downhill(c) = None` when `cy == 0`, else `(cx, cy+1)`. The §5 row now says the flat world mirrors *both* cube exceptions — the level Top (`field.rs:135-151`) and the rim row with nothing below it — so canopy water and detritus hold as they do today. **(b) Weather specified normatively in the new §5a**, reusing `Blob`/`Weather` unchanged with a per-topology reading of the three fields: `center = [u/(32S), v/(32S), 0]`, `axis = [cos φ, sin φ, 0]`, `rate` = embedded units/tick. Seeded init consumes the **same four draws** in the same order (draw 2 consumed and discarded), `rate = 10.0/(period_min·60·TICK_HZ)` for one width traversal per period (≈0.43 px/s at S = 1, 20 min), a bounded heading walk `θ = walk_deg_per_min·(2·unit − 1)` from the **same single draw** per blob per minute, radius `= blob_radius_deg.to_radians()` in embedded units (`= radius·32·S` px), specular reflection of `axis` at the four walls by §2's algebra, and `z = 0` so the serialized shape and the cube path are untouched. The plane is 10 × 5.625 units at every S, so all of it is S-invariant. One derived default change: `blobs_per_channel` 3 → **8** for flat, because the plane is 56.25 unit² against the cube's 20 (the same 2.81×) |
| 3 | **BLOCKING** — flat care coordinates: `CareTarget` `u`/`v` are `u8`, refused above 63 (`care/mod.rs:122-140`), journaled in `PlannedCommand` (`care/mod.rs:146-162`); no package owned `crates/cubarium/src/care/**` | **accepted** | The whole care chain moves into **FW-4**, explicitly: `u`/`v` widened to `u16` and validated against the topology's extent instead of a literal 64, `PlannedCommand` journal and web-request compatibility, and `care_effects.rs` (moved out of FW-5 so the target type and its drawing have one owner). FW-4's verification gains "a journal written before the widening still replays", and FW-6 reserves `cubarium/tests/flat_care.rs` |
| 4 | Should-fix — `chord_sq`'s `×1024` (`point.rs:108-116`) is not exact for `S ≠ 1`; the plan both scaled `max_local_radius` and claimed a fixed 32 bounds `S ≤ 3.5`, but 32 is a cube completeness proof tied to two seams (`unfold.rs:5-13`) | **accepted** | §2 makes distance a topology method returning squared **pixels**: cube unchanged, flat `(Δu)² + (Δv)²` straight from chart coordinates — exact, cheaper and independent of the embedding, which on a plane is then used only for noise and weather. Two explicit rules replace the implicit scaling: **`world_scale` is flat-only and `Topology::Cube` pins `S = 1`**, so the 32-px proof and the 9-px budget are never rescaled; and `Flat`'s `max_local_radius()` is `min(w, h)/2`, justified by having no seams at all rather than by the cube's proof. The `S ≤ 3.5` claim is deleted from §7 |
| 5 | Should-fix — "files (exclusive)" was false: FW-6 creates `crates/*/tests/flat_*.rs` inside FW-1/2/3's globs; FW-1 defined `Scale::footprint_radius()` while FW-3 claimed the decision | **accepted** | §9 now reserves **eleven exact FW-6 paths** by name and states that every implementation package's glob excludes `tests/flat_*.rs`. Ownership split stated in the *Owns* column: **FW-1 owns the budget's value**, FW-3 owns **adopting** it at `sprite.rs:61-64,810-818`. The two remaining genuine overlaps (FW-5/FW-7 on `art_present/tall.rs` and `lanternjaw/**`, FW-5/FW-8 on `art_present/habitat.rs`) are named and resolved by sequencing rather than left implicit |
| 6 | Should-fix — the plan named a nonexistent `check`, said the variable-sized care state has `cell_count()` length, validated non-persisted care targets, and gave the comparator no cross-schema procedure although 17 refuses 16 | **accepted; three factual errors** | §4 rewritten: the hook is **`WorldState::validate`** (`world/state.rs:108`), already called after decode at `snapshot.rs:163`. Care's persisted state is **not** per-cell — `CareState.showers[].cells` is a `Vec<u16>` of raw `CellId` indices (`care.rs:257-276`) already range-checked against `CELL_COUNT` at `care.rs:340,345-349` including a `vec![false; CELL_COUNT]` set, and each becomes runtime `cell_count()`. `CareTarget` is a host type, not in `WorldState`, so it is validated at admission in FW-4, not post-decode. The comparator gains the missing procedure: a **`CubeProjection`** carrying only what v16 and v17 both mean identically (excluding `config`), exported by each build from its own binary so neither ever decodes the other's snapshot, compared field-by-field and hashed; FW-1 authors it and the pre-change fixture is taken from `main` before FW-1 merges |
| 7 | Should-fix — FW-0 measured only serial `R` and `tick_ms`, not the decisive four-core parallel render time | **accepted** | FW-0 now measures **four** pinned numbers: serial cube render on one A78, the same render split across four A78 cores, `tick_ms`, and end-to-end achieved fps and ticks/s for a real run. Its verification line requires all four with their pinning recorded |

No item was rebutted. Every citation in the findings was checked against the
tree before the change; all were accurate, including the three that identified
statements in the previous revision as simply wrong — the nonexistent `check`,
the care state's supposed per-cell length, and the implicit rescaling of the
cube's 32-pixel completeness proof.

## Review repair 3 (Astra round three, plus Wrysk's ring, 2026-09-16)

Astra returned *rework* with three blocking and three should-fix items; findings
5 and 7 were resolved by the coordinator in `9cbf508` before this pass. Every
citation was checked against the tree and all were accurate. **Wrysk then changed
the shape**: the world's left and right edges join and the top and bottom stay
solid. That supersedes finding 1 entirely and simplifies §2 rather than
complicating it, which was the stated reason for the change.

| # | finding | verdict | what changed |
|---|---|---|---|
| — | **Wrysk: the world is a ring, not a rectangle** | **adopted** | `Topology::Flat` → **`Topology::Ring { w, h }`** (renamed once in §2; the `FW-` ids and this file's name keep the "flat" label). §2 is rewritten: the vertical edge is a **seam of the chart to itself** — identity `TangentMap`, zero quarter turns, translation by `∓w`, the `Front right → Right left` row of `design/surface-topology.md` with the neighbour being the same chart — so `travel` uses its **existing** `Some(seam)` branch (`travel.rs:263-275`) and the only reflection left is the existing rim `REFLECT_Y` (`travel.rs:254-258`) at `v = 0` and `v = h`. **`REFLECT_X`, the corner algebra and the inverted tie rule are deleted**: a ring corner *is* the cube's lower side corner, so the lowest-`Edge` tie rule and `lower_corner_crosses_a_vertical_seam_and_reflects` (`travel.rs:478-492`) carry over unchanged. `unfold` gets at most two images (direct and `±w`), guaranteed by validating `w >= 2·max_local_radius() + 2·CELL_PIXELS`, with `Ring::max_local_radius() = min(h, w − 2·CELL_PIXELS)/2` satisfying it by construction. `chord_sq` becomes `min(|Δu|, w − |Δu|)² + Δv²` in pixels. The field graph's rows are rings: 3,600 cells, **7,120** edges (`80·45 + 80·44`), 3,440 interior cells at degree 4, the 160 top and bottom cells at degree 3, and **no corners** — repair 1's degree-2 case is deleted with the walls that created it. `downhill` is unchanged (top row `None`, else `(cx, cy+1)`) |
| 1 | **BLOCKING** — planar weather not dimensionally implementable: centre in embedded units but reflected at pixel-space `u = w`; `rate` hardcoded width 10 although `Flat{w,h}` is general; "zero or empty period" contradicts `config.rs:765-778` | **superseded by the ring; the underlying error is real and its cause removed** | The planar blob model is **deleted**, together with its rate formula, its fold rule and its extra `WorldState::validate` checks. §5a now specifies a **cylinder embedding** — `θ = 2π·u/w`, `r = w/(2π·32·S)`, `y = 1 − 2v/h`, `embed = [r cos θ, y, r sin θ]` — on which `normalize()` preserves azimuth and maps height monotonically to latitude, so the **existing spherical blob model runs verbatim**: no new state, no new units, no dimensional mismatch to make, and **no draw changes, so RNG stream parity is exact by construction** rather than engineered (the parity note is kept as a test worth writing). Two further results are recorded: the cylinder's lateral area is `2πr × 2 = 20` embedded units² — **exactly the cube's `5 × 2 × 2`, at every S** — so `blobs_per_channel` stays **3** and repair 2's derived "3 → 8" is retracted; and the wave sum is **seamless across the wrap by construction** (`u = 0` and `u = w` are one point in 3D), which retires §6's former "noise must be divided by S" condition into the choice of `r`. The distortion is stated and accepted for v1: the world spans only `±32.15°` of latitude against 360° of longitude, so a 55° blob covers the full height while spanning ~15% of the width (showers read as bands; in *area* the cap is well matched, 21% of the sphere against the world's 53%), and `dφ/dy` falls from `0.628` at the equator to `0.451` at `y = ±1`, so blobs drift ~28% slower in latitude near the rims. The same compression makes the embedding anisotropic (`90/32 = 2.81×` vertical stretch of noise patches); the one-line isotropic alternative is named and **flagged for Wrysk, not taken**, since `Topology::height()` is already separate from `embed()` |
| 2 | **BLOCKING** — the care chain is still partly unowned: cube-hardcoded `CareTarget::resolve` (`cubarium-core/src/care.rs:148-168`) used at `world/care.rs:60`, and `HunterTarget::resolve` for `SpawnApex` (`hunter/state.rs:47-67`) | **accepted** | Both core resolvers are named explicitly in **FW-2**'s objective, with the nuance that these are *core* types carrying `f64` `u`/`v` and are distinct from FW-4's host `CareTarget` with its `u8` fields — two types, one name, two owners. FW-4 keeps the host side unchanged. **"feed/rain/clean/apex targets beyond pixel 63 resolve on a ring and are refused off-world"** is added to both FW-2's and FW-6's verification |
| 3 | **BLOCKING** — `CubeProjection` too loose: omits `ou`, structure, birth tick, hunger memory, mode, escrow, births, phenotype, ancestry, origin, turn counter (`organism.rs:46-67`), the allocator state (`ids.rs:15-23`), weather and every extension (`world/state.rs:23-99`), and excluding all config drops behaviour-bearing configuration | **accepted; the enumeration was the mistake** | §4 redefines it as **`WorldState` verbatim with exactly one substitution**: `config: ConfigProjection`, itself `WorldConfig` minus *only* `topology` and `world_scale`. Everything else is carried whole at its own type, including `organisms: Slots<Organism>` with `entries`, `free` and `live`, plus `weather` and all seven extension states — the field list is spelled out. The cross-schema procedure is also made concrete: the post-change build reads the pre-change **payload** through the frozen `v16.rs` mirror and `decode_exact`, the mechanism the existing schema-refusal tests already use, so `decode_snapshot`'s refusal of schema 16 is untouched and no patch to `main` is needed — the fixture is one snapshot file from an unmodified `main` run. **Negative tests are made part of the definition** (FW-6's `ring_schema17.rs`): perturb one organism field, one free-list entry, one weather blob, one field vector, one extension state and one non-added config field, and equality must fail in every case — "a comparator that cannot fail is not evidence" |
| 4 | Should-fix — §7 still told implementers to scale `MAX_LOCAL_RADIUS`, contradicting §2's pinned cube proof | **accepted** | `MAX_LOCAL_RADIUS` is removed from §7's list of `world_scale`-multiplied defaults, and a new paragraph states that it is per topology and never scaled: the cube's 32 px is a completeness proof tied to `MAX_SEAMS` (`unfold.rs:5-13`) and `Cube` pins `S = 1`. `Ring::max_local_radius()` is described as a **chosen performance cap** (quadratic `unfold_pixels` cost) that also holds the image count at two — explicitly *not* a limit of meaning. The two config sites that validate radii against the constant (`config.rs:863,917`) take the topology's value instead |
| 5 | Should-fix — FW-0 measured a four-core number that cannot exist yet | **resolved by the coordinator** (`9cbf508`) | FW-0's row now reads: serial `R` and `tick_ms` now; the four-A78 render number re-measured after FW-3; **S provisional until that rerun**. Its verification requires each number with its pinning and its date |
| 6 | Should-fix — the §6 FW-9 paragraph said FW-9 does not touch cubarium's wire side, but its row extends the raster encoder and the strip protocol | **accepted** | Reworded: FW-9 does not **offload cubarium's presenter**, while it *does* extend cubarium's side of the wire with the auxiliary layers and a strip-format layer id |
| 7 | Should-fix — stale six-A55 line in the handoff | **resolved by the coordinator** (`9cbf508`) | No change here |
| 8 | Note — commit order `950f294 → b781a1c → 5a81e05 → 9cbf508` is correct | **no action** | Recorded |

Repair 1's items 4 and 6, and repair 2's items 2 and 4, are marked in place as
superseded, so the historical sections are not mistaken for current state.

**Amended 2026-09-16, after this table was written:** Wrysk chose the isotropic
embedding that item 1's entry above had flagged and left untaken. `embed()`'s
vertical is now `y_e = (h/2 − v)/(32·S)`, not `1 − 2v/h`, so this row's figures —
the `±32.15°` latitude span, the `2πr × 2 = 20` area, the 28% poleward slowing and
the "showers read as bands" conclusion — are **superseded by §5a's recomputed
numbers**. The blob defaults still transfer unchanged (`blobs_per_channel` 3), but
for the solid-angle reason §5a now gives rather than the area coincidence recorded
here. Reason for the record: the stratified design needs height only as a scalar,
and there is no reason for the picture to stretch.

## Review repair 4 (Astra round four, 2026-09-16)

Three blocking and two should-fix. Every citation was checked against the tree
and **all five were accurate**; none is rebutted. Two of them caught claims this
document had asserted rather than derived.

| # | finding | verdict | what changed |
|---|---|---|---|
| 1 | **BLOCKING** — `CubeProjection` still cannot compare v16 with v17: the plan bumps `CONFIG_VERSION` 8 → 9, but `ConfigProjection` excluded only `topology` and `world_scale`, while `WorldConfig.version` is a serialized field defaulting to `CONFIG_VERSION` (`config.rs:15,19,372`), so every comparison would fail | **accepted** | `ConfigProjection` now excludes **three** fields — `topology`, `world_scale` and `version` — and §4 states that the versions are asserted **separately and explicitly** (fixture: schema 16 / config 8; new build: schema 17 / config 9), so a silent version change and a silent world change cannot cancel or mask each other. The decode wording is also made precise: the new build **never calls `decode_snapshot` on a v16 file** — that refusal is untouched — while its *test* reads the v16 **payload** through the frozen `v16.rs` mirror and `decode_exact::<WorldStateV16>`, the mechanism the existing refusal tests already use. The product refuses old worlds; only the comparator looks inside one |
| 2 | **BLOCKING** — FW-1 cannot own `CubeProjection`: it owns `cubarium-surface/**`, but `WorldState`, the snapshot module, `decode_exact` and the mirrors are all in `cubarium-core` (`world/state.rs:22`, `snapshot.rs:7,101`) and the dependency runs core → surface | **accepted** | The projection, the frozen `v16.rs` mirror, the exporter and the comparator move to **FW-2**, with the dependency reason stated in the row. FW-1's objective drops them and its verification now reads "the surface crate's own by-value tests are FW-1's evidence; the projection comparison starts at FW-2". The standing-evidence paragraph is rewritten to say the projection applies **from FW-2 onwards**, with the fixture still captured from `main` before FW-1 starts |
| 3 | **BLOCKING** — weather is not isotropic after `normalize()` (`habitat.rs:217`): caps are ~`sec(60.5°) = 2.03×` taller than wide at the rims, so "showers are round" is wrong; and the 24.5% / 25.6% coverage figures divide cap area by *visible-world* area although blob centres are initialised and orbit over the whole sphere (`habitat.rs:136,166`), so the expected covered fraction is 21.3% for either world | **accepted; both errors were mine** | Confirmed by derivation: one pixel spans `cos φ/(32Sr)` of spherical distance horizontally but `cos²φ/(32Sr)` vertically, so a round cap maps to a pixel shape stretched by `1/cos φ` — `1.00` at the equator, `2.03` at `±60.50°`. §5a now separates the two claims that the previous revision conflated: **noise patches are round** (the isotropic cylinder is a local isometry from the `(u,v)` plane, which is what the embedding decision bought), **weather caps are round at the equator and up to ~2× taller than wide at the top and bottom rows**. The coverage row becomes **21.3% in both worlds**, with the reason spelled out — for a centre uniform on the sphere the covered fraction is the cap's own fraction, independent of the world's shape — and a paragraph records that repair 2's "3 → 8" and repair 3's "stays 3 because the area is 20" were *both* computed in the wrong frame. `blobs_per_channel` stays 3 for the third and correct reason. Per Wrysk's decision the shape distortion is **accepted for v1 and weather is not redesigned**; the knobs remain `periods_min` and `blob_radius_deg`. FW-6's `ring_weather.rs` must **measure and record the cap aspect at the rims, not assert it round** |
| 4 | Should-fix — ring transport is sound (self-seams fit `Seam` at `geometry.rs:138`, the rim bounce at `travel.rs:254`, raster dedup at `raster.rs:127`, field-edge dedup at `field.rs:123`), but edge priority differs among the four corners | **accepted** | `earliest_exit` resolves a tie by `tied.trailing_zeros()` — the lowest `Edge` index — over `Top = 0 < Right = 1 < Bottom = 2 < Left = 3` (`travel.rs:50-70`, `geometry.rs:72-77`). The four ring corners therefore behave in **two different ways**: top-left (`Top` vs `Left`), top-right (`Top` vs `Right`) and bottom-left (`Bottom` vs `Left`) **reflect first**, while bottom-right (`Right` vs `Bottom`) **crosses the seam first** — which is the only one the cube's `lower_corner_crosses_a_vertical_seam_and_reflects` precedent covers. "A ring corner fixture" is replaced by **exact-tie and near-tie tests at all four corners** in FW-1's verification and FW-6's `ring_travel.rs` |
| 5 | Should-fix — stale text: the config example still read `{ flat = … }`, and the Stage A0 paragraph still gave the deleted planar `min(w, h)/2` radius although §7's later paragraph had the `Ring` formula | **accepted** | Both corrected: the TOML example is `topology = { ring = { w = 320, h = 180 } }`, and the Stage A0 line now reads `min(h, w − 2·CELL_PIXELS)/2`. A full sweep replaced the remaining topology-denoting "flat"/"Flat" with ring/Ring across every live section, including the document title, leaving the `FW-` ids, the file name and the historical repair sections as they are; the one surviving "flat" in §5a was the adjective "flat claim" and is reworded |

The reserved FW-6 path `cubarium-core/tests/ring_embedding.rs` is renamed
`ring_weather.rs` and now carries both the embedding's wrap continuity and the
measured cap aspect; the count stays at eleven.
