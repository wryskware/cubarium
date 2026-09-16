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

1. Cubarium is coupled to the cube through **five hubs**: `SurfacePoint`/`Face`,
   `CellId`+`CELL_COUNT`, `Canvas`, `FrameSink::submit(&Frame)`, and the
   world/view/presenter caches that are shaped five-by-`CELL_COUNT` at birth.
2. Recommended abstraction: a `Copy` **enum** `cubarium_surface::Topology { Cube,
   Flat { w, h } }` **plus a `Scale`** (cell pixels and world scale) in the same
   surface contract — not a trait, not a generic, so `SurfacePoint`, `WorldState`
   and `Canvas` stay concrete and the 1,591 existing tests survive.
3. `Face` stays the chart id; flat has one chart, `Face::Front`. Three forced
   widenings: pixel indices `u8 → u16`, `CELL_COUNT` to a runtime count, and a
   checked `cell_count() <= u16::MAX` bound on `CellId`.
4. Flat edges are **not** identity: a wall bounce composes `REFLECT_Y` on
   horizontal walls, `REFLECT_X` on vertical ones, and both at a corner
   (`= quarter_turns(2)`). Only `unfold` is identity on a plane.
5. **Candidate first flat world: 640×360, upscaled 3×, at world scale S = 2** —
   sprite tile 32 px, field cell 8 px. Gated on FW-0's measured board numbers,
   not fixed here. 320×180 is the cube world stretched and wastes the panel.
6. `world_scale` S multiplies every length. Hold `cell = 4·S` **and scale the
   habitat noise coordinates by S** and every raster from 320×180 to 1920×1080
   has the same 3,600 cells and the same ecology.
7. Sim and render share **one host loop**, so the budget is
   `20·tick_ms + fps·render_ms <= 1000 ms`, not two independent budgets. Render
   work is `∝ S²`; the tick is S-invariant.
8. Genuine higher-resolution art is authoring, not a bake flag: the 148
   `art/parts/*.svg` are pixel art on integer grids. A runtime `scale` path
   exists but is blocked by a hard 9-pixel stamp budget that must scale with S.
9. Biome/terrain variation is real but **separable**: one low-frequency region
   field offsetting per-cell habitat parameters, behind a toggle, off by default.
10. Total: **nine packages — 4 large, 4 medium, 1 small**; six at high effort.
    Parallel pairs start only after FW-1's contract is frozen.

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
`chart_images` returns one direct image, so `unfold` collapses to segment length
(and *there* the tangent map is genuinely `IDENTITY`) and `unfold_pixels` always
takes the direct fast path clipped to `0..w`/`0..h`. `embed()` becomes
`[u/32S, v/32S, 0]` (see §6), which keeps `chord_sq`'s `×1024` factor exact — on a
plane the chord bound is the true distance, so pair rejection stops being
conservative and becomes exact (`point.rs:111`).

**Wall reflection is not identity — this is the part to get right.** The existing
rim bounce does two things at once (`travel.rs:254-258`): it negates the
component of the *remaining* displacement normal to the edge
(`d = Vec2::new(remaining.x, -remaining.y)`) and it composes
`TangentMap::REFLECT_Y` into `out.map`. A rectangle has walls on both axes, so:

| wall hit | remaining displacement | composed into `out.map` |
|---|---|---|
| `Edge::Top` (`v=0`) or `Edge::Bottom` (`v=h`) | `(x, −y)` | `REFLECT_Y` |
| `Edge::Left` (`u=0`) or `Edge::Right` (`u=w`) | `(−x, y)` | `REFLECT_X` |
| exact corner (`Exit::is_tie`, both edges within `GEOM_EPS`) | `(−x, −y)` | `REFLECT_X.then(REFLECT_Y) = TangentMap::quarter_turns(2)` |

The two reflections are diagonal matrices and commute, so the corner case is
order-independent and the composed map has `det = +1` — a sweep aimed exactly
into a corner returns along its incoming direction, the planar analogue of the
cube's documented vertex behaviour. **The cube's tie rule is inverted here and
must be stated as a deliberate deviation:** on the cube a tie picks the *lowest*
`Edge` index and crosses one seam (`design/surface-topology.md`, "exact
corners"); on a plane a tie means both walls were reached at once and **both**
reflections apply in a single step. A near-corner (two exits at distinct but
adjacent `t`) falls out of the existing loop as two successive reflections and
must give the same final direction as the exact tie; that pair is a required
test. Forward progress is unchanged: after a bounce `p` sits exactly on the wall
with the normal component pointing inward, so the same edge yields no further
exit at `t ≥ 0`, and reflections already count toward `MAX_CROSSINGS`
(`travel.rs:118,278-282`) with `nudge_inward` as the counted fallback. A flat
world bounces far more often than the cube's single rim, so any latent weakness
in reflection handling surfaces immediately rather than rarely.

**A capacity bound Astra is right to demand.** `CellId` is a `u16`
(`field.rs:18-21`), so a topology may not exceed 65,535 cells. At `cell = 4 px` a
1920×1080 flat world would want 129,600 — over the limit. `Topology::validate()`
must compute `cell_count()` with checked multiplication and refuse anything above
`u16::MAX`, with the error naming the raster and the cell size. The §6 ladder
(`cell = 4·S`) stays at 3,600 and is far inside it, but an arbitrary
`Flat { w, h }` from a config file is not.

**Two forced widenings.**

- `u8 → u16` for pixel indices. 320 > 255, so `SurfacePoint::pixel_center`,
  `pixel()`, `pixel_neighbor`, `PixelImage { x, y }`, `Canvas::{get,set,add}` and
  the 129 `0..64u8` loop bounds must widen. Purely mechanical, zero behaviour
  change on the cube, and the compiler finds every site.
- `CELL_COUNT` from a const to a runtime count. `1,280 = 2^8·5` cannot be
  factored 16:9 with square cells, so no flat raster reproduces it.
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

**Cell size for a flat world: `4·S` px.** At S = 1, 320×180 → 80×45 = 3,600
cells and **7,075** undirected edges (`79·45 + 80·44`), not a scaled 2,528. The
adjacency builder emits `None` per absent edge independently (`field.rs:98-119`),
so a rectangle has 3,354 interior cells of degree 4, **242 non-corner border
cells of degree 3, and 4 corner cells of degree 2** — the corners are a real case
the conservative outgoing-flux limiter must be tested against. `w` and `h` must be
multiples of `cell_pixels()`; every raster in §6 is.

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
that tool for no gain, because the schema number already refuses a mismatched
build. Validate **after decode** instead, in `WorldState::check`:

- `config.topology` is one the build supports, and `Topology::validate()` passes
  (dimensions positive, multiples of `cell_pixels()`, `cell_count() <= u16::MAX`);
- every `Fields` vector, every ecology vector and the care state have length
  exactly `cell_count()`;
- every organism's `pos.face` and every care target's `face`
  (`crates/cubarium/src/care/mod.rs:125-135`) is a chart the topology has —
  `face == Face::Front` for flat — and `u < w`, `v < h`.

Keep the `face` byte in `SurfacePoint`: one byte per organism against forking the
position type, `OrganismView`, `PathSegment` and every fixture.

**The cube regression check has to change too.** `ecology_hash` hashes
`postcard::to_allocvec` of the whole masked `WorldState`, config included
(`snapshot.rs:193-199`), so adding two config fields changes it for cube worlds
as well — a pre/post equality claim is impossible. Replace it with a **normalized
cube-state comparator** written once in FW-1's test pass: a digest over
everything *except* `config`, plus an explicit assertion that the two configs
differ only in the new fields at their cube defaults. That comparator, not
`ecology_hash`, is the standing evidence in §9.

## 5. Ecology and art: what is not mechanical

Everything below reads the cube's embedded height, and it is the one place a flat
port changes the world rather than the coordinates. The first draft under-counted
the consumers; this is the corrected list.

| system | cube | flat — recommended |
|---|---|---|
| light / moisture | `y = cell.center().embed()[1]` (`habitat.rs:94-98`) | **design call: `height(p) = 1 − 2v/h`.** The panel is a side view: canopy at the top row, soil at the bottom edge. Keeps `design/stratified-world.md` intact |
| **classic controller** | `height: o.pos.embed()[1]`, `up: up_direction(o.pos.face)` (`world/step.rs:449-460`) | both become `Topology` methods; flat `up` is the constant `(0, −1)` |
| **neural controller** | the same two fields in `SelfState` (`world/step.rs:3068-3079`) | identical treatment; a policy trained on cube height reads the same channel |
| **depth preference** | `obs.up * (w_depth · (h_pref − obs.height))` steers every organism (`controller.rs:228-235`) | works unchanged *given* a topology height and up; with a wrong height it silently steers the whole population into a wall |
| noise sampling | 3D noise on `[-1,1]^3` (`habitat.rs:43-75`) | same wave sum at `[u/32S, v/32S, 0]` — the `S` divisor is required, see §6 |
| **weather blobs** | `normalize(positions[i])`, `dot(b.center, dir)`, angular cap `cap(θ)` with `blob_radius_deg` (`habitat.rs:198-225`) | **BLOCKING design call: this is a spherical metric.** `normalize([u/32, v/32, 0])` collapses the plane to a polar *fan* around the origin, not a moving blob. A flat world needs a **planar weather metric**: blob centres drifting in the plane, a raised-cosine cap on planar distance in pixels (`blob_radius_px = blob_radius_deg` reinterpreted through `world_scale`), and a stated edge rule (drift reflects, matching the walls). Owned by **FW-2** |
| band thresholds | `Canopy` iff `h >= 1.0` — true only on the Top face (`art_present/habitat.rs:380`) | needs a `canopy_top` threshold. **No value can be validated from current code**; propose `0.67` as an explicit new default for review, not as a derived number |
| detritus fall / downhill | gravity's tangential component vanishes on the level Top face, so `downhill` is `None` there (`field.rs:135-151`, `design/stratified-world.md:47-52`): **the cube canopy deliberately never drains** | on a plane every cell has a lower neighbour, so a flat canopy drains completely to the bottom wall. **Explicit decision required, owned by FW-2:** either accept it (the top rows become a shedding ridge and the bottom a litter bank) or reproduce the cube's behaviour with a `downhill_floor` on the top band. Recommend **accept, and measure**, because the drain is what makes a side view read as gravity |
| water flow / pools | no flux across the open rim (`water.rs`) | the bottom wall becomes a moat the cube never had; expect standing water along the bottom row and re-check `evap_floor` |
| pair rejection | conservative chord bound (`pairs.rs:64`) | exact Euclidean; strictly fewer candidate pairs |
| founders | random face + `unit·FACE_EXTENT` (`lifecycle.rs:69-72`) | one chart; the face draw must be **kept and discarded** or every seed shifts |
| seam-carried sprites | `unfold_pixels` owns pixels across seams | no seams; bodies clip at four walls. Simpler, never worse |
| tall columns / rigs | per side face, `Face::ALL` order (`art_present/mod.rs:365`) | choose columns along `u`; the three-quarter rigs already read as walking along a wall |
| ray-cast preview | `raycast.rs` | not ported; cube-only |

The visible risks needing a call rather than a port: the **planar weather
metric**, the **canopy drain**, the **canopy threshold**, **water against the new
bottom wall**, and **organism density** — the same 512-organism cap over 3,600
cells instead of 1,280 thins the world by 2.81× in ecological terms, whatever the
raster. Recommend keeping the cap and raising `founders` proportionally.

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

1. **Noise coordinates must scale with S.** The habitat waves have wavelengths in
   embedding units and are sampled at `p = position/32`
   (`habitat.rs:43-75,83-104`). Sampling a flat world at a fixed `/32` while the
   cells grow as `4S` changes the number of light/moisture patches per cell by a
   factor of S — different patchiness, different ecology. Sample at `/(32·S)`.
2. **Radii already expressed in cells stay invariant, and that is checkable:**
   `sense_depth` is `ceil(r_sense / CELL_PIXELS)`
   (`world/lifecycle.rs:383-389`), so with `r_sense = 6S` and `cell = 4S` it is 2
   hops at every S. Any radius still written in bare pixels must be moved onto S.
3. `cell_count()` stays 3,600 and inside the `u16` `CellId` bound (§2).

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
core. The board also carries an **Adreno GPU and a Hexagon NPU; both are out of
scope for the first version** (Wrysk may want the NPU for organism networks or
voice later, which is a reason not to spend it on rendering now).

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
a flat world at 640×360. It is blocked by one hard gate: `FOOTPRINT_RADIUS = 9.0`
is checked both when a sprite is built (`sprite.rs:18-22,61-64`, which returns
`Err("sprite extent … exceeds the 9-pixel surface budget")`) and again at stamp
time as `extent * scale > FOOTPRINT_RADIUS`, which **silently draws nothing**
(`sprite.rs:810-818`). At `scale = 2` every creature disappears. The constant is
documented as "not review-tunable — it is the radius the shared unfolding is
proven correct for", so it must become `9.0 · world_scale`, derived in FW-1's
`Scale` and validated against `max_local_radius()` (which bounds `S ≤ 3.5` at
today's 32). **That work lives in `cubarium-render` and therefore in FW-3**, which
owns that crate; FW-7 consumes the interface and must not edit it. The first
draft's FW-7 omitted the render crate entirely — that was the gap.

**The honest finding about the art itself.** Raising `svg/scale` in the `.import`
files and `TILE` in the baker produces sprites that are S× *bigger blocks of the
same art* — no new detail, because the sources are already pixel art at 1 unit =
1 pixel. Genuine higher-resolution artwork means redrawing those 148 SVG paths on
an S× finer grid. Each file is small (a handful of `<path>` elements), but it is
authoring work, not a bake parameter. The staging that follows:

- **Stage A0 (no assets):** `scale = 2.0` on pack v5 with a scaled footprint
  budget. Proves the flat world at 640×360 before any Godot run.
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

Re-cut after review: **FW-1 owns cell geometry and `world_scale`** in the surface
contract, **FW-2 owns the two ecological design calls** (planar weather metric,
canopy drain), and **FW-3 owns the scale-derived stamp budget** in the render
crate. The *Owns* column names the decisions, not just the files.

| id | objective | owns (decision) | files (exclusive) | interface exposed | verification | size | effort |
|---|---|---|---|---|---|---|---|
| FW-0 | Vendor `cube-proto` with `Raster` + wire format 2; **measure `R` (single-core cube render ms) and `tick_ms` on desktop and on the board** | the S/fps value in §6 | `vendor/cube-proto/**`, `vendor/cube-proto.rev` | `Raster { width, height, data }` | `cargo test --workspace`; recorded `R` and `tick_ms`, and the S they select | small | medium |
| FW-1 | `Topology` + `Scale` (cell px, world scale, footprint and local radii); `u16` pixel indices; runtime cell count with the `u16::MAX` capacity check; flat travel with per-axis and corner wall reflection | the geometry contract, incl. the corner tie deviation | `crates/cubarium-surface/**` | §2's API; `CUBE_CELL_COUNT`; the normalized cube-state comparator | cube results identical by value; flat exercised at **S = 1 and S = 2 from the first commit** | large | **high** — reflection algebra, tie/near-tie equivalence, progress bounds |
| FW-2 | Topology and scale through the world: config, schema 17 + post-decode validation, height/`up`, both controllers, **planar weather metric**, **canopy drain decision**, S-scaled noise, founders, care | planar weather; canopy drain; `canopy_top` | `crates/cubarium-core/**` | `WorldConfig.{topology,world_scale}`, `World::topology()`, `RenderView.topology` | cube run matches by the normalized comparator (not `ecology_hash`); a flat run reaches steady state | large | **high** — the two calls, RNG stream order, controller height |
| FW-3 | `Canvas` by topology, `Canvas::pixels()`, `encode_raster`, **`footprint_radius = 9·S`** and the `scale` stamp path; port `field/trail/sprite/body/multipart`; deterministic row-band parallel hook | the stamp budget | `crates/cubarium-render/**` | `Canvas::new(topo)`, `pixels()`, `encode_raster`, `footprint_radius(scale)` | same-seed cube canvas bit-identical; a `scale = 2` stamp draws instead of vanishing | medium | **high** (raised) — the 9-px budget is a correctness bound, not a knob |
| FW-4 | `Output` enum + sinks (shim/png/web), viewer flat mode, CLI/config, preview refusal, host timing numbers | — | `crates/cubarium/src/{sink/**,cli.rs,net.rs,run.rs,runner/**}`, `sink/web/index.html` | `enum Output`, `topology`/`world_scale` TOML | flat PNG capture; viewer screenshot; the measured split that picks `--fps` | medium | medium |
| FW-5 | Presenter for flat: `RenderView.topology` consumed, `ArtPresenter` built from the world's cell count, bands, horizon, water/rain, motifs, columns, bodies, care effects | presenter cache lifetime | `crates/cubarium/src/{present.rs,art_present/**,lanternjaw/**,care_effects.rs,scene.rs}` | — | a flat capture reviewed by Wrysk; cube capture diffed to zero | large | **high** — new UI construction |
| FW-6 | Independent test authoring: wall reflection incl. exact and near corner, corner-cell flux, capacity refusal, schema-17 post-decode validation, planar weather, sink/raster, presenter goldens | — | new files only: `crates/*/tests/flat_*.rs` | — | written without reading FW-1..FW-5's own tests | medium | **high** |
| FW-7 | Pack v6 (`tile` as data), baker at `TILE = 16·S`, `art.rs`/`tall.rs`/`lanternjaw` constants made tile-relative, S-scaled default builder; re-bake at S = 2 | — | `art/**`, `assets/atelier/**`, `crates/cubarium/src/art.rs`, `art_present/tall.rs`, `lanternjaw/**` | `pack.json` v6 | the S = 1 pack still loads and renders bit-identically; a 640×360 capture; reproducible Godot bake | large | **high** — `tall.rs`'s 4/16-px lattice |
| FW-8 | Biomes: region field, four parameter sets, `mechanisms.biomes` off by default, presentation by dominant biome | biome parameter sets | `crates/cubarium-core/src/habitat.rs` (+ new `biome.rs`), `crates/cubarium/src/art_present/habitat.rs` | `HabitatConfig.biomes` | toggle off ⇒ comparator unchanged; toggle on ⇒ short run showing distinct regions | medium | medium |

**Ordering, with the review's constraint that parallel pairs start only after
shared interfaces are frozen.** FW-0 any time. FW-1 first, **and its API is
frozen and published before anything else starts** — that freeze is the gate, not
the merge. Then **FW-2 ∥ FW-3** (disjoint crates). Then **FW-4 ∥ FW-5** (disjoint
file sets in `crates/cubarium/src`), but only after FW-3 publishes
`footprint_radius` and `encode_raster`. FW-6 starts at the FW-1 freeze and only
creates new files. FW-7 follows FW-5 (both would otherwise write
`art_present/tall.rs`) and depends on FW-3's budget without editing it. FW-8 last.
W2 (the device) follows FW-7.

**The scale staging survives the review, with one correction.** Scale lives in
the FW-1 *contract* and is exercised at S = 2 there, so nothing downstream ever
changes geometry. What stages is the chosen *value*: FW-1..FW-6 can ship a flat
world at S = 1 on pack v5, FW-3's Stage A0 can show S = 2 on the same pack, and
FW-7 makes S = 2 the shipped default with re-baked art. If FW-7 slips the panel
still works. The first draft's claim of "S = 1 then S = 2 without touching
topology" was wrong because cell size is a surface constant
(`field.rs:11-16,49-68`); putting `Scale` in FW-1 is the fix.

Six packages are high effort: FW-1 (a reflection or tie mistake is silent and
corrupts motion), FW-2 (the weather metric and canopy drain decide whether the
flat world is alive), FW-3 (raised — the 9-px stamp budget is a proven-correctness
bound shared with `unfold_pixels`), FW-5 (new UI construction), FW-6 (test
authoring is high by the working rules), FW-7 (`tall.rs`'s named row indices).

**Standing evidence at every package:** `cargo test --workspace` green, plus a
fixed-seed `cubarium run --fresh --seed 1 --speed 0 --seconds 120 --sink png`
capture whose PNG bytes match the pre-change run, plus the **normalized
cube-state comparator** of §4 — *not* `ecology_hash`, which cannot be equal across
a config change (`snapshot.rs:193-199`).

**Test authoring is its own pass.** FW-6 is that pass at high effort, written
against §2's contract and §5's calls rather than against the implementation.
FW-7 and FW-8 each need their own small authoring pass.

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
