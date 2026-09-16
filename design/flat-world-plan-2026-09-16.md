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
   (`= quarter_turns(2)`). Only `unfold` is identity on a plane. §5 now *decides*
   the canopy drain and specifies the planar weather model normatively (§5a).
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
`[u/(32S), v/(32S), 0]` (see §6).

**Distance is a topology method, not a scaled embedding.** `chord_sq` multiplies
the embedded chord by a hardcoded `1024 = 32²` to reach pixel units
(`point.rs:108-116`); with a flat embedding divided by `32S` that constant is
right only at `S = 1`, and patching it to `1024·S²` would leave distance
depending on a scale factor it has no business knowing. Make it
`Topology::chord_sq(a, b) -> f64` in squared **pixels**: the cube arm is today's
body verbatim; the flat arm is `(Δu)² + (Δv)²` straight from the chart
coordinates — exact, cheaper, and independent of the embedding. The embedding is
then used for exactly two things on a plane: habitat noise sampling and weather
blob state. Pair rejection stops being a conservative bound and becomes the true
distance (`pairs.rs:64`).

**Local-radius bounds are per topology; the cube's proof is not rescaled.**
`MAX_LOCAL_RADIUS = 32.0` is a *completeness* proof, not a tunable: within it
"every shortest path crosses at most `MAX_SEAMS` seams … so enumerating chart
paths of that length is complete" (`unfold.rs:5-13`). Nothing about `S` may touch
it. Two consequences, stated as rules:

- **`world_scale` is a flat-only parameter. `Topology::Cube` pins `S = 1`**, so
  the cube keeps its 32-px bound, its 9-px stamp budget and its proof untouched.
  Validation refuses a cube world with `world_scale != 1`.
- `Topology::max_local_radius()` for `Flat { w, h }` has no seam argument at all —
  one chart, direct images only — so completeness is trivial and the bound exists
  only to keep `unfold_pixels` cost finite. Set it to `min(w, h) / 2` (beyond
  that a radius reaches past the far wall and means nothing), with the stamp
  budget `9·S` required to satisfy `9·S <= max_local_radius()`; at S = 2 on
  640×360 that is `18 <= 180`, comfortably inside.

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
that tool for no gain: the schema number already refuses a mismatched build.

**The post-decode hook already exists and is named `WorldState::validate`.**
`decode_snapshot` calls it at `snapshot.rs:163` after the CRC and the exact-length
decode (`world/state.rs:108`). The flat work is to extend it, and to replace the
`CELL_COUNT` constants it reaches with the world's runtime `cell_count()`:

- `config.topology` is one this build supports and `Topology::validate()` passes:
  dimensions positive, multiples of `cell_pixels()`, `cell_count() <= u16::MAX`,
  and `world_scale == 1` for `Cube`;
- every `Fields` vector (`fields.rs:19-32`) and every ecology v1 vector has length
  exactly `cell_count()` — these are the only per-cell serialized arrays;
- **care's persisted state is not a per-cell vector.** `CareState.showers[].cells`
  is a `Vec<u16>` of raw `CellId` indices with matching weights
  (`care.rs:257-276`); `CareState::validate` already range-checks them against
  `CELL_COUNT` at `care.rs:340,345-349`, including a `vec![false; CELL_COUNT]`
  duplicate set. Each of those becomes `cell_count()`. The first draft wrongly
  said the care state has `cell_count()` length;
- every organism's `pos.face` is a chart the topology has — `Face::Front` for
  flat — with `u < w`, `v < h`.

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

1. Define a **common semantic projection** `CubeProjection` carrying only what
   schemas 16 and 17 both have and both mean identically: `tick`, every `Fields`
   and ecology vector, the organism roster (id, `pos`, heading, energy, reserve,
   genome), the tick counters, the RNG stream states and the care ledgers —
   **excluding `config`** and excluding anything the bump reshaped.
2. Each build exports it from its own binary: the pre-change build emits
   `CubeProjection` from a fixed-seed run, the post-change build emits the same.
   Neither ever decodes the other's snapshot, so the refusal rule is untouched.
3. Compare the two projections field-by-field (and hash them for a one-line CI
   signal). Equality is the cube regression evidence used throughout §9.

FW-1 authors the projection type and the exporter; the pre-change export is taken
**before** FW-1 merges, from the current `main` build, and committed as a fixture.

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
| **weather blobs** | orbital: `center` rotated about `axis` at `rate` rad/tick, plus a per-minute random-walk tilt (`habitat.rs:110-196`); sampled by `dot(b.center, dir)` against an angular cap (`habitat.rs:198-225`) | **decided — see §5a for the normative model.** The spherical metric does not transfer: `normalize([u/32S, v/32S, 0])` collapses the plane to a polar *fan* about the origin. §5a specifies a planar blob reusing the same `Blob`/`Weather` structs. Owned by **FW-2** |
| band thresholds | `Canopy` iff `h >= 1.0` — true only on the Top face (`art_present/habitat.rs:380`) | needs a `canopy_top` threshold. **No value can be validated from current code**; propose `0.67` as an explicit new default for review, not as a derived number |
| detritus fall / downhill | gravity's tangential component vanishes on the level Top face, so `downhill` is `None` there (`field.rs:135-151`, `design/stratified-world.md:47-52`): **the cube canopy deliberately never drains**, and the bottom row of the side faces has nothing below it | **decided: the flat world mirrors both exceptions.** `downhill(c) = None` when `cy == 0`, otherwise the neighbour at `(cx, cy+1)`. The top cell row is the canopy and holds its water and detritus exactly as the cube's level Top does; the bottom row has no cell below it and keeps its litter, exactly as the cube's rim row does. No toggle, no new config |
| water flow / pools | no flux across the open rim (`water.rs`) | the bottom wall becomes a moat the cube never had; expect standing water along the bottom row and re-check `evap_floor` |
| pair rejection | conservative chord bound (`pairs.rs:64`) | exact Euclidean; strictly fewer candidate pairs |
| founders | random face + `unit·FACE_EXTENT` (`lifecycle.rs:69-72`) | one chart; the face draw must be **kept and discarded** or every seed shifts |
| seam-carried sprites | `unfold_pixels` owns pixels across seams | no seams; bodies clip at four walls. Simpler, never worse |
| tall columns / rigs | per side face, `Face::ALL` order (`art_present/mod.rs:365`) | choose columns along `u`; the three-quarter rigs already read as walking along a wall |
| ray-cast preview | `raycast.rs` | not ported; cube-only |

The calls now made in this document: the **planar weather model** (§5a) and the
**canopy drain** (decided above). Still open and needing Wrysk's eye rather than a
rule: the **canopy threshold**, **water against the new bottom wall**, and
**organism density** — the same 512-organism cap over 3,600
cells instead of 1,280 thins the world by 2.81× in ecological terms, whatever the
raster. Recommend keeping the cap and raising `founders` proportionally.

## 5a. The planar weather model (normative)

Flat weather reuses the persisted structs unchanged, so the cube path and the
snapshot shape are untouched: `Blob { center: [f64;3], axis: [f64;3], rate: f64 }`
and `Weather { light, moisture, last_walk_minute }` (`habitat.rs:110-127`). Only
the *interpretation* of the three fields changes with the topology.

| field | cube meaning | flat meaning |
|---|---|---|
| `center` | unit direction of the cap centre on the sphere | the blob centre embedded on the plane: `[u/(32S), v/(32S), 0.0]` — the same embedding cells use, so `z == 0` always |
| `axis` | unit orbit axis, perpendicular to `center` | the unit heading of travel in the plane, `[cos φ, sin φ, 0.0]` |
| `rate` | angular speed, rad/tick | linear speed in embedded units per tick |

The plane is **10 × 5.625 embedding units at every S** (`w/(32S) = 320S/32S = 10`,
`h/(32S) = 5.625`), so every quantity below is S-invariant, which is what keeps §6's
constant-ecology claim true.

**Initialization** (`Weather::new`, `habitat.rs:136-156`). The cube draws four
`unit` values per blob at the reserved block `u64::MAX − 8 + k, k ∈ 0..4` (centre
`z`, centre azimuth, axis `z`, axis azimuth), light blobs keyed `0..n`, moisture
`n..2n`. Flat **consumes the same four draws in the same order** so the stream
stays aligned: draw 0 → `u = unit · w`; draw 1 → `v = unit · h`; **draw 2 is
consumed and discarded** (the cube's axis `z` has no planar counterpart); draw 3 →
`φ = TAU · unit`. `period_min` is cycled from `cfg.periods_min` exactly as today,
and

> `rate = 10.0 / (period_min · 60 · TICK_HZ)`

— one traversal of the world width per period. At the default 20-minute period and
20 Hz that is `1.0/2400` units/tick ≈ 0.43 px/s at S = 1, crossing 320 px in 20
minutes. A zero or empty period gives `rate = 0`, as today.

**Per tick** (`Weather::advance`). `cfg.moving == false` freezes centres in both
topologies, unchanged. Otherwise `center += axis · rate`, then **specular
reflection at the four walls using exactly §2's algebra**: negate `axis.x` at
`u = 0` or `u = w`, negate `axis.y` at `v = 0` or `v = h`, both at a corner, and
mirror the overshoot back inside so the centre always lands in the rectangle. A
blob therefore bounces around the world the way an organism does, which is the
consistency argument for reusing the rule.

**Per simulated minute.** The cube draws one `unit(seed, Stream::Weather, key,
minute)` per blob and tilts by `walk_deg_per_min`. Flat draws **the same single
value with the same key and counter** and applies a bounded heading walk:

> `θ = walk_deg_per_min.to_radians() · (2·unit − 1)`, then `axis ← rotate(axis, θ)`

— uniform in `[−step, +step]`, one draw per blob per minute, so the RNG stream is
consumed identically to the cube and a stream-parity test can assert it.

**Sampling** (`Weather::sample`, `habitat.rs:204-227`). Same raised-cosine
profile, planar argument: `radius = blob_radius_deg.to_radians()` read as a radius
in **embedded units** (equivalently `radius · 32 · S` pixels), and
`cap(d) = 0.5·(1 + cos(π·d/radius))` for `d = |p_cell − b.center| <= radius`, else
0. `amplitude`, the `light`/`moisture` sums, `rain_source` as the bare moisture
sum and therefore `rain_threshold` all keep their meanings, so `design/water.md`'s
tuning carries over. The `[f64; CELL_COUNT]` out-params become slices.

**One default needs changing.** `blob_radius_deg = 55°` is 0.96 embedded units,
about 31 px of radius at S = 1. The flat plane is 56.25 unit² against the cube's
20 unit² — the same 2.81× as the cell count — so `blobs_per_channel = 3` leaves
the flat world mostly dry. Set the flat default to **8** (`2.81 × 3`) to keep
shower coverage per cell at the cube's value; the cube default is unchanged.

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
proven correct for", so it must become `9.0 · world_scale`, defined in FW-1's
`Scale` and validated against the **topology's own** `max_local_radius()` (§2) —
`min(w, h)/2` on a plane, and the untouched cube proof of 32 px on a cube, where
`S` is pinned to 1 anyway. **That work lives in `cubarium-render` and therefore in FW-3**, which
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

Re-cut twice. **FW-1** owns cell geometry, `world_scale` and every derived bound
(including the *value* of the stamp budget); **FW-2** owns the ecological calls
now written down in §5/§5a; **FW-3** owns *adopting* the budget in the render
crate; **FW-4** owns the care chain end to end. "Files" are exclusive after the
FW-6 reservation below.

**FW-6 reserves these exact paths**, and every implementation package's glob
excludes `tests/flat_*.rs`:
`cubarium-surface/tests/{flat_travel,flat_field,flat_raster}.rs`;
`cubarium-core/tests/{flat_world,flat_weather,flat_schema17}.rs`;
`cubarium-render/tests/{flat_canvas,flat_stamp_scale}.rs`;
`cubarium/tests/{flat_sinks,flat_present,flat_care}.rs`.

| id | objective | owns (decision) | files (exclusive; no `tests/flat_*.rs`) | interface exposed | verification | size | effort |
|---|---|---|---|---|---|---|---|
| FW-0 | Vendor `cube-proto` with `Raster` + wire format 2. **Measure four numbers on the board, pinned:** serial cube render `R` on one A78, the same render split across four A78 cores, `tick_ms`, and end-to-end achieved fps and ticks/s for a real run | the S and `--fps` values in §6 | `vendor/cube-proto/**`, `vendor/cube-proto.rev` | `Raster { width, height, data }` | `cargo test --workspace`; all four numbers recorded with their pinning, and the S they select | small | medium |
| FW-1 | `Topology` + `Scale`: cell pixels, `world_scale` (flat-only; cube pinned to 1), `footprint_radius() = 9·S`, per-topology `max_local_radius()` and `chord_sq()`; `u16` pixel indices; runtime cell count with the `u16::MAX` check; flat travel with per-axis and corner wall reflection; flat `downhill` (`cy == 0` ⇒ `None`); the `CubeProjection` type and exporter | the whole geometry contract, incl. the corner tie deviation and **the stamp-budget value** | `crates/cubarium-surface/**` | §2's API; `CUBE_CELL_COUNT`; `CubeProjection` | cube identical by value; flat exercised at **S = 1 and S = 2 from the first commit**; pre-change projection fixture captured from `main` before merge | large | **high** |
| FW-2 | Topology and scale through the world: config, schema 17, extended `WorldState::validate` (runtime `cell_count()` in `care.rs:340,345-349` included), height/`up`, both controllers, **§5a's planar weather**, the decided canopy `downhill`, S-scaled noise, flat `blobs_per_channel` default, founders, core care state | the ecological calls as written in §5/§5a | `crates/cubarium-core/**` | `WorldConfig.{topology,world_scale}`, `World::topology()`, `RenderView.topology` | cube run equal by `CubeProjection`; RNG stream parity test for weather init and the per-minute draw; flat run reaches steady state | large | **high** |
| FW-3 | `Canvas` by topology, `Canvas::pixels()`, `encode_raster`; **adopt** `Scale::footprint_radius()` at both check sites (`sprite.rs:61-64,810-818`) and the `scale` stamp path; port `field/trail/sprite/body/multipart`; deterministic row-band parallel hook | adoption only — the value is FW-1's | `crates/cubarium-render/**` | `Canvas::new(topo)`, `pixels()`, `encode_raster` | same-seed cube canvas bit-identical; a `scale = 2` stamp draws instead of vanishing | medium | **high** |
| FW-4 | `Output` enum + sinks (shim/png/web), viewer flat mode, CLI/config, preview refusal, **the whole care chain: `CareTarget.{u,v}` widened to `u16` and validated against the topology extent (`care/mod.rs:122-140`), `PlannedCommand` journal and web-request compatibility (`care/mod.rs:146-162`), and the canvas flourish** | the care wire and journal shape | `crates/cubarium/src/{sink/**,cli.rs,net.rs,run.rs,runner/**,care/**,care_effects.rs}`, `sink/web/index.html` | `enum Output`, `topology`/`world_scale` TOML, the widened `CareTarget` | flat PNG capture; viewer screenshot; a journal written before the widening still replays; the measured split that picks `--fps` | medium | medium |
| FW-5 | Presenter for flat: `RenderView.topology` consumed, `ArtPresenter` built from the world's cell count, bands, horizon, water/rain, motifs, columns, bodies | presenter cache lifetime | `crates/cubarium/src/{present.rs,art_present/**,lanternjaw/**,scene.rs}` | — | flat capture reviewed by Wrysk; cube capture diffed to zero | large | **high** |
| FW-6 | Independent test authoring at the reserved paths: wall reflection incl. exact and near corner, corner-cell flux, capacity refusal, extended `validate`, §5a's weather incl. stream parity, care widening, sink/raster, presenter goldens | — | only the reserved `tests/flat_*.rs` paths above | — | written without reading FW-1..FW-5's own tests | medium | **high** |
| FW-7 | Pack v6 (`tile` as data), baker at `TILE = 16·S`, `art.rs`/`tall.rs`/`lanternjaw` constants made tile-relative, S-scaled default builder; re-bake at S = 2 | — | `art/**`, `assets/atelier/**`, `crates/cubarium/src/art.rs` | `pack.json` v6 | S = 1 pack still loads and renders bit-identically; 640×360 capture; reproducible Godot bake | large | **high** |
| FW-8 | Biomes: region field, four parameter sets, `mechanisms.biomes` off by default, presentation by dominant biome | biome parameter sets | `crates/cubarium-core/src/biome.rs` (new), `crates/cubarium/src/art_present/habitat.rs` | `HabitatConfig.biomes` | toggle off ⇒ `CubeProjection` unchanged; toggle on ⇒ short run showing distinct regions | medium | medium |

FW-5 and FW-7 both touch `art_present/tall.rs` and `lanternjaw/**`; FW-7 runs
after FW-5 and the table gives those paths to FW-5, with FW-7 editing them only in
its own window. FW-8's presenter file is likewise sequenced after FW-5.

**Ordering.** FW-0 any time. FW-1 first, **and its API is frozen and published
before anything else starts** — the freeze is the gate, not the merge. Then
FW-2 ∥ FW-3 (disjoint crates). Then FW-4 ∥ FW-5 (disjoint file sets), only after
FW-3 publishes `encode_raster`. FW-6 starts at the FW-1 freeze and writes only its
reserved paths. FW-7 follows FW-5; FW-8 last. W2 (the device) follows FW-7.

**The scale staging.** Scale lives in the FW-1 contract and is exercised at S = 2
there, so nothing downstream ever changes geometry. What stages is the chosen
*value*: FW-1..FW-6 can ship a flat world at S = 1 on pack v5, FW-3's Stage A0 can
show S = 2 on the same pack, and FW-7 makes S = 2 the shipped default with
re-baked art. If FW-7 slips the panel still works.

Six packages are high effort: FW-1 (a reflection, tie or bound mistake is silent
and corrupts motion), FW-2 (the weather model and the validate extension), FW-3
(the 9-px stamp budget is a proven-correctness bound shared with `unfold_pixels`),
FW-5 (new UI construction), FW-6 (test authoring is high by the working rules),
FW-7 (`tall.rs`'s named row indices).

**Standing evidence at every package:** `cargo test --workspace` green, a
fixed-seed `cubarium run --fresh --seed 1 --speed 0 --seconds 120 --sink png`
capture whose PNG bytes match the pre-change run, and **`CubeProjection` equality**
against the fixture exported from `main` before FW-1 — not `ecology_hash`, which
cannot be equal across a config change (`snapshot.rs:193-199`) and cannot even be
computed across the schema refusal.

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
four should-fix items; all six are accepted and none is rebutted. Astra's two
decisions in item 2 are adopted verbatim and written up normatively in §5a.

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
