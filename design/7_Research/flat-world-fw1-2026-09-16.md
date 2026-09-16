---
design_status: exploration
last_reviewed: 2026-09-16
---

# FW-1 result: `Topology` and `Scale` in the surface crate

Written by Opus on 2026-09-16 against
[the FW-1 brief](../handoffs/flat-world-fw1-opus-surface-2026-09-16.md) and
[the ring-world plan](../flat-world-plan-2026-09-16.md) §2, §5a and §9's FW-1
row. **This is the freeze FW-2, FW-3 and FW-6 are briefed against.**

## 1. The frozen public API

`cubarium_surface`, one line each. Everything not listed is unchanged.

### New: `geometry.rs`

```rust
pub const CELL_PIXELS: f64 = 4.0;        // pixels per cell edge at S = 1 (moved from field.rs)
pub const FOOTPRINT_PIXELS: f64 = 9.0;   // the stamp budget at S = 1
pub const EMBED_PIXELS: f64 = 32.0;      // embedded units per pixel at S = 1

pub struct Scale { /* private f64 */ }   // Copy, Debug, PartialEq, PartialOrd, Default = ONE
                                         // serde(transparent): it *is* its multiplier on the wire,
                                         // so a config writes `world_scale = 2.0`
pub const Scale::ONE: Scale;             // S = 1
pub const fn Scale::new(world: f64) -> Scale;      // unchecked; validate() accepts or refuses
pub const fn Scale::world(self) -> f64;            // S
pub fn Scale::cell_pixels(self) -> f64;            // 4·S
pub fn Scale::footprint_radius(self) -> f64;       // 9·S
pub fn Scale::embed_divisor(self) -> f64;          // 32·S

pub enum Topology { Cube, Ring { w: u16, h: u16 } } // Copy, Debug, Eq, Hash, Default = Cube, serde

pub fn Topology::charts(self) -> &'static [Face];
pub fn Topology::chart_index(self, face: Face) -> usize;   // for indexing a per-chart array
pub fn Topology::has_chart(self, face: Face) -> bool;
pub fn Topology::extent(self, face: Face) -> (f64, f64);
pub fn Topology::neighbor(self, face: Face, edge: Edge) -> Option<Seam>;
pub fn Topology::seam_turns(self, face: Face, edge: Edge) -> Option<u8>;
pub fn Topology::max_local_radius(self) -> f64;
pub fn Topology::cells(self, scale: Scale, face: Face) -> (u16, u16);
pub fn Topology::cell_count(self, scale: Scale) -> usize;
pub fn Topology::embed(self, scale: Scale, p: &SurfacePoint) -> [f64; 3];
pub fn Topology::embed_tangent(self, scale: Scale, p: &SurfacePoint, t: Vec2) -> [f64; 3];
pub fn Topology::height(self, p: &SurfacePoint) -> f64;
pub fn Topology::chord_sq(self, a: &SurfacePoint, b: &SurfacePoint) -> f64;
pub fn Topology::validate(self, scale: Scale) -> Result<(), TopologyError>;

pub enum TopologyError {  // Copy, Debug, PartialEq, Display, Error
    BadScale { world: f64 },
    CubeScaleNotOne { world: f64 },
    EmptyRing { w: u16, h: u16 },
    ExtentNotCellMultiple { w: u16, h: u16, cell_pixels: f64 },
    TooManyCells { cells: usize, w: u16, h: u16, cell_pixels: f64 },
    RingTooNarrow { w: u16, needed: f64 },
    FootprintExceedsLocalRadius { footprint: f64, radius: f64 },
}
```

### Widened

```rust
pub fn SurfacePoint::pixel_center(topo: Topology, face: Face, x: u16, y: u16) -> SurfacePoint;
pub fn SurfacePoint::is_canonical(&self, topo: Topology) -> bool;
pub fn SurfacePoint::canonicalize(self, topo: Topology) -> SurfacePoint;
pub fn SurfacePoint::pixel(&self, topo: Topology) -> (u16, u16);
pub fn pixel_neighbor(topo: Topology, face: Face, x: u16, y: u16, edge: Edge) -> Option<(Face, u16, u16)>;

pub fn travel(topo: Topology, start: SurfacePoint, displacement: Vec2) -> Travel;
pub fn travel_into(topo: Topology, start: SurfacePoint, displacement: Vec2, out: &mut Travel);

pub fn chart_images(topo: Topology, observer_face: Face, max_seams: u8, out: &mut Vec<ChartImage>);
pub fn segment_is_valid(topo: Topology, observer_face: Face, observer: Vec2, image: Vec2, path: &ChartPath) -> bool;
pub fn unfold(topo: Topology, observer: SurfacePoint, target: SurfacePoint, max_distance: f64) -> Option<Unfolded>;
pub fn unfold_with(topo: Topology, images: &[ChartImage], observer: SurfacePoint, target: SurfacePoint, max_distance: f64) -> Option<Unfolded>;
pub fn surface_distance(topo: Topology, a: SurfacePoint, b: SurfacePoint, max_distance: f64) -> Option<f64>;
pub fn ChartPath::final_face(&self, topo: Topology, observer_face: Face) -> Face;

pub fn unfold_pixels(topo: Topology, anchor: SurfacePoint, radius: f64, out: &mut Vec<PixelImage>);
pub struct PixelImage { pub face: Face, pub x: u16, pub y: u16, /* unchanged */ }

pub const CUBE_CELL_COUNT: usize = 1280;           // renamed from CELL_COUNT
pub fn CellId::new(topo: Topology, scale: Scale, face: Face, cx: u16, cy: u16) -> CellId;
pub fn CellId::all(topo: Topology, scale: Scale) -> impl Iterator<Item = CellId>;
pub fn CellId::face(self, topo: Topology, scale: Scale) -> Face;
pub fn CellId::cx(self, topo: Topology, scale: Scale) -> u16;
pub fn CellId::cy(self, topo: Topology, scale: Scale) -> u16;
pub fn CellId::center(self, topo: Topology, scale: Scale) -> SurfacePoint;
pub fn cell_of(topo: Topology, scale: Scale, p: &SurfacePoint) -> CellId;

pub fn FieldGraph::new(topo: Topology, scale: Scale) -> FieldGraph;
pub fn FieldGraph::topology(&self) -> Topology;      // new
pub fn FieldGraph::scale(&self) -> Scale;            // new
pub fn FieldGraph::cell_count(&self) -> usize;       // new
pub struct ScalarField { pub values: Box<[f64]> }    // was Box<[f64; CELL_COUNT]>
pub fn ScalarField::zeros(topo: Topology, scale: Scale) -> ScalarField;
pub fn ScalarField::constant(topo: Topology, scale: Scale, x: f64) -> ScalarField;
pub fn ScalarField::len(&self) -> usize;             // new
pub fn ScalarField::is_empty(&self) -> bool;         // new
pub fn deposit(topo: Topology, scale: Scale, field: &mut ScalarField, center: SurfacePoint, radius: f64, amount: f64) -> usize;
```

### Removed

`SurfacePoint::{embed, embed_tangent, chord_sq}` — moved onto `Topology`, where
the shape lives. `CELL_COUNT` — renamed `CUBE_CELL_COUNT` and no longer the
world's cell count.

### Unchanged

`diffuse`, `FACE_EXTENT`, `GEOM_EPS`, `NUDGE`, `MAX_CROSSINGS`,
`MAX_LOCAL_RADIUS`, `MAX_SEAMS`, `CELLS_PER_FACE_EDGE`, `Vec2`, `TangentMap`,
`FaceFrame`, `face_frame`, `Travel`, `PathSegment`, `Unfolded`, `ChartImage`,
`ChartPath`, `CellId`'s representation (`pub u16`) and `index()`.

## 2. Where the scale is, and where it is not

The plan writes the topology as the first parameter of the free functions. Three
of them also need the world scale, and the split is deliberate:

- **Scale-free** (shape only): `travel`, `travel_into`, `unfold`, `unfold_with`,
  `unfold_pixels`, `chart_images`, `segment_is_valid`, `surface_distance`,
  `pixel_center`, `pixel_neighbor`, `pixel`, `canonicalize`, `is_canonical`,
  `chord_sq`, `height`, `max_local_radius`, `extent`, `charts`.
- **Scale-carrying** (cell geometry or the embedding): `cell_of`, `deposit`,
  `CellId::{new, all, face, cx, cy, center}`, `FieldGraph::new`,
  `ScalarField::{zeros, constant}`, `Topology::{cells, cell_count, embed,
  embed_tangent, validate}`.

`max_local_radius()` reads `CELL_PIXELS` (the constant 4.0), not
`scale.cell_pixels()`, which is what makes it scale-free. Both of §2's worked
numbers come out of that reading: `min(180, 320 − 8)/2 = 90` at `S = 1` and
`min(360, 640 − 8)/2 = 180` at `S = 2`, which is the plan's `18 <= 180`.

## 3. Files touched outside `crates/cubarium-surface/**`, and why

Every one is the mechanical follow-through of a widened signature: an inserted
`Topology::Cube` / `Scale::ONE` argument, a `u8 -> u16` on a pixel or cell index,
or the `CELL_COUNT -> CUBE_CELL_COUNT` rename. No behaviour changed anywhere.

| crate | files | what changed |
|---|---|---|
| `cubarium-core` | 46 | `Topology::Cube` / `Scale::ONE` threaded through `cell_of`, `CellId::*`, `travel`, `unfold*`, `chart_images`, `ScalarField`; `CELL_COUNT` renamed; `.embed()` → `Topology::Cube.embed(Scale::ONE, &p)`; `chord_sq` moved to the topology at `pairs.rs:64`; a few cell-index loop bounds `u8 -> u16` |
| `cubarium-render` | 20 | the same threading, plus **`Canvas::{get, set, add}` widened to `u16`** (plan §2 names them) and the pixel tuples and loop bounds that feed them |
| `cubarium` (host) | 55 | the same threading, plus the pixel-index widening through `present.rs`, `art_present/{environment,habitat,tall}.rs`, `raycast.rs`, `runner/mod.rs` and their tests; `GROUND_LATTICE`, `SEED_CELL`, `TallColumn.cx` and the `rain_marks`/`foliage_rows` pixel and cell types |
| `cubarium-search` | 6 | threading only; the crate stays cube-only (plan §1) |
| `cubarium-surface-oracle` | 0 | untouched; it has no surface call sites and stays cube-only |
| `vendor/cube-proto` | 0 | untouched (FW-0 owns it), and still `u8` at the discrete API, so the bridging casts live on the cubarium side |

The one judgement call is `Canvas::{get,set,add}`. §2 lists them among the forced
widenings, and the alternative — casting `PixelImage.x` back to `u8` at every
render call site — would have had to be undone by FW-3 anyway. The `Canvas`
*storage* is untouched: FW-3 still owns making it topology-shaped.

## 4. The by-value cube evidence

`crates/cubarium-surface/tests/cube_by_value.rs` and its golden
`tests/data/cube_by_value.txt` were written **from the tree as it stood before
any topology work** (commit `e088cae`) and the golden has not changed since.
Values are IEEE-754 bit patterns, so "identical" means bit-identical, not
"prints the same".

| section | what it covers | digest |
|---|---|---|
| `embed` | `embed` at all 20,480 pixel centres, `embed_tangent` at 25 named points | `e8ab6d5d65236be6` |
| `chord_sq` | 4,000 fixed random pairs | `ad708d6225a6ec43` |
| `travel` | the 10 named design fixtures, all 16 connected half-edges × 64 pixel-centre parameters, and 6,000 swept cases including vertex-directed aims and 600-pixel steps — end point, map, crossings, reflections, ties, fallback and every path segment | `544891f2bd73975b` |
| `chart_images` | all five faces at `MAX_SEAMS`, origins and maps | `ee326694ddf55401` |
| `unfold` | 20,000 pairs, near and far, plus 6 named cross-seam pairs | `b2105a56c4f6ac45` |
| `unfold_pixels` | 10 anchors × 6 radii (0 to 32), every returned pixel in full | `3d82a57cd723e00d` |
| `field_graph` | every cell's face/cx/cy/centre/`cell_of`/neighbours/downhill and all 2,528 edges | `6ae15d465a34d16a` |
| `deposit_diffuse` | 10 deposits, the field total, a `rate = 0.9` diffusion and all 1,280 resulting values | `ff2236a21d8460eb` |

All eight digests are unchanged after the topology parameter. The file's call
sites gained `Topology::Cube` / `Scale::ONE` like every other call site; the
golden did not move.

Alongside it, **every pre-existing surface test still passes with its assertions
unchanged** — including `tests/seams.rs`, which checks all 16 connected
half-edges at all 64 integer positions against `cube_proto::cross_seam`, and
`raster.rs`'s brute-force equivalence of `unfold_pixels` with `unfold`.

## 5. The four corners, as observed

At `w = 320, h = 180`, aiming exactly at each corner
(`crates/cubarium-surface/src/travel.rs`, `ring_tests`):

| corner | start, displacement | tied edges | `exit.edge` | first event | `(crossings, reflections, ties)` |
|---|---|---|---|---|---|
| top-left | `(1, 1) + (−2, −2)` | Top, Left | **Top** | reflect | `(1, 1, 1)` |
| top-right | `(319, 1) + (2, −2)` | Top, Right | **Top** | reflect | `(1, 1, 1)` |
| bottom-left | `(1, 179) + (−2, 2)` | Bottom, Left | **Bottom** | reflect | `(1, 1, 1)` |
| bottom-right | `(319, 179) + (2, 2)` | Right, Bottom | **Right** | **cross the seam** | `(1, 1, 1)` |

Exactly as the brief predicts: three reflect first, bottom-right crosses first,
under `Top = 0 < Right = 1 < Bottom = 2 < Left = 3`. In every case the swept
length is preserved to `1e-9`, the fallback never fires, the end point is
canonical, and the same fixture at `S = 2` (640×360, all lengths doubled) gives
exactly twice the end coordinates with the same counters. Near-ties skewed 2%
either way resolve to one edge alone (`ties == 0`) and still cost one crossing
and one reflection.

## 6. Test counts

| | before | after |
|---|---|---|
| `cargo test --workspace` | 1,472 passed, 0 failed, 23 ignored | 1,510 passed, 0 failed, 23 ignored |
| `cubarium-surface` unit tests | 53 | 100 |
| `cubarium-surface` integration tests | 39 | 40 (`cube_by_value.rs` is the new one) |

The 38 new tests are the by-value golden plus the ring exercises in
`geometry.rs`, `point.rs`, `travel.rs`, `unfold.rs`, `raster.rs` and `field.rs`.
FW-6's reserved paths `tests/{ring_travel,ring_field,ring_raster}.rs` were not
created.

`cargo clippy --workspace --all-targets` reports the same warnings as before the
change (all pre-existing, in files this package did not touch).

## 7. What §2 needed deciding, or was wrong as written

Nothing in §2 turned out to be impossible. Four things it under-specifies, with
the reading FW-1 froze:

1. **`Topology` alone cannot answer a cell question.** The enum carries only
   `w` and `h`, and a ring's cell size is `4·S`, so `cell_of`, `cell_count`,
   `CellId`'s decode, `FieldGraph::new`, `ScalarField` and `embed` need the scale
   too. They take it as a second parameter, listed in §2 of this document. The
   plan's `cell_of(topo, p)` and `Topology::embed(&SurfacePoint)` are therefore
   `cell_of(topo, scale, p)` and `Topology::embed(scale, &p)`. **FW-2/FW-3 must
   read `world.topology()` and `world.scale()` together.**

2. **`max_local_radius()` uses the constant `CELL_PIXELS`, not `cell_pixels()`.**
   §2 writes `min(h, w − 2·CELL_PIXELS)/2` in capitals and `4·S` elsewhere in
   lower case. Only the capital reading reproduces both of its worked numbers
   (90 at `S = 1`, 180 at `S = 2`), and it is the one that keeps the query radius
   — and therefore `unfold`'s panic bound — independent of the scale.

3. **`w >= 2·max_local_radius() + 2·CELL_PIXELS` is unfalsifiable as stated.**
   Because `max_local_radius() <= (w − 2·CELL_PIXELS)/2` by construction, the
   inequality holds for *every* `w`, including `w = 4`, where it holds only
   because the radius has gone negative. `validate` therefore also refuses a
   non-positive `max_local_radius()`, under the same `RingTooNarrow` error.

4. **One refusal was added beyond the three the brief lists**:
   `FootprintExceedsLocalRadius`, when `scale.footprint_radius() >
   topo.max_local_radius()`. Without it a validated world can still panic inside
   `unfold_pixels` the first time anything is stamped, which is the bound §2
   calls a proven-correctness property. It never fires on the §6 ladder
   (`9 <= 90`, `18 <= 180`) or on the cube (`9 <= 32`). **If FW-2 wants a world
   config that is accepted here and refused later instead, say so and it comes
   out.**

Two smaller notes for the packages downstream:

- **`chart_images` caps a ring at one crossing**, which is exactly §2's "the
  ring enumerates its own three images directly"; `MAX_SEAMS` stays 2 and is a
  cube constant. A caller passing `MAX_SEAMS` for a ring gets three images, not
  eleven.
- **The `unfold`/`unfold_pixels` panic message still contains the literal
  `MAX_LOCAL_RADIUS`**, because `cubarium-render`'s `multipart.rs:1292` and
  `multipart_scale.rs:649` assert on that substring and changing it would have
  been a behaviour change outside the crate. The message now reads
  "exceeds the local radius {r} of {topo:?} (MAX_LOCAL_RADIUS on the cube)".

## 8. A process note

Two commits by other workers on `tachyon-screen` — `8d26cf3` ("Tachyon plan:
FW-P's per-pass verdict") and `43455f7` ("Tachyon plan: GS-0 result; ...") —
swept most of the FW-1 diff into their own messages with `git commit -a` while
it was still in progress, and `8d26cf3` in particular committed a
`crates/cubarium-surface` that did not compile because the new `geometry.rs` was
still untracked. The work is all present and the branch is green at
`FW-1: Topology and Scale, and Topology::Cube threaded through the workspace`;
the history is misleading. A shared worktree needs one worker per `git add`.
