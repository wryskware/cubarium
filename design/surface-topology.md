---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# One connected cube surface

The proposed spatial foundation is a piecewise flat surface atlas: five square
coordinate charts joined by the shim's existing seam contract. A face ID is a
coordinate chart, not a simulation partition. All systems use the same surface
operations. See [checked local APIs](7_Research/local-contracts.md).

**Since FW-1 (2026-09-16) the cube is one of two shapes.** Everything below is
the `Topology::Cube` arm of `cubarium_surface::Topology`, unchanged by value;
[the ring](#the-ring-w-h-2026-09-16) at the end of this document is the other,
and it is written in the same contract rather than beside it.

## Coordinates and existing ownership

Use `cube_proto::Face` with indices **Front=0, Right=1, Back=2, Left=3, Top=4**.
Local continuous coordinates `(u,v)` span `[0,64)` in pixel units; positive u is
image-right and positive v is image-down. Pixel `(x,y)` has center
`(x+0.5,y+0.5)`. Chart boundaries are at 0 and 64, not at the outer pixel centers.
Transient seam intersections may equal 64; canonical stored positions do not.

Store `SurfacePoint { face, u, v }`, local tangent velocity, and local orientation.
Rotate every tangent quantity when crossing: heading, remaining displacement,
steering vectors, and any local directional memory. Scalar energy, genotype,
oscillator phase, and body-relative controller values do not rotate.

For sampling spatially coherent environmental functions, embed on `[-1,1]^3`.
With `a=u/32-1`, `b=v/32-1`:

| Face | Position | Tangent +u | Tangent +v | Outward normal |
| --- | --- | --- | --- | --- |
| Front | `(a,-b,1)` | `+X` | `-Y` | `+Z` |
| Right | `(1,-b,-a)` | `-Z` | `-Y` | `+X` |
| Back | `(-a,-b,-1)` | `-X` | `-Y` | `-Z` |
| Left | `(-1,-b,a)` | `+Z` | `-Y` | `-X` |
| Top | `(a,1,b)` | `+X` | `+Z` | `+Y` |

The embedding matches `geometry::pixel_direction` at pixel centers. Sample
continuous environmental functions by position so neighboring faces share the
same limit at a seam. Do not use separate per-face noise seeds or normals that
make a nominally smooth weather field jump at edges. The embedding is not a
distance metric for interactions: straight chords can pass through the cube.
It is a valid lower bound on surface length. Convert embedding distances to
pixel units (multiply by 32); reject a candidate when its squared chord exceeds
the squared interaction radius, with a numerical margin at the threshold.

## Seam transport

Reuse `Face::neighbor`, `Edge`, seam reversal, and the rotation convention from
`cube-proto`. Extend them for continuous coordinates and overshoot locally.
Do not duplicate a competing adjacency table in production. This table records
the expected contract for review and independent tests:

| Exit | Entry | Along-edge parameter | Heading quarter-turns |
| --- | --- | --- | --- |
| Front right | Right left | same | 0 |
| Right right | Back left | same | 0 |
| Back right | Left left | same | 0 |
| Left right | Front left | same | 0 |
| Front top | Top bottom | same | 0 |
| Right top | Top right | reversed | 1 |
| Back top | Top top | reversed | 2 |
| Left top | Top left | same | 3 |

Inverse transitions follow the same adjacency, with inverse rotations. One
quarter-turn means `(du,dv) -> (dv,-du)` in image coordinates. For continuous
edge distance use `t -> 64-t`; the integer API uses `t -> 63-t`. Confusing these
creates a one-pixel seam offset.

For each displacement, find the earliest boundary intersection along the
segment. Advance to it, transfer its edge parameter, rotate the remaining
segment and all tangent state, then continue until consumed. A long displacement
may cross several faces; a single final-coordinate clamp is insufficient.

Examples in continuous coordinates:

- Front `(63.75,20)` moving `(0.5,0)` finishes at Right `(0.25,20)`.
- Right `(10,0.25)` moving `(0,-0.5)` finishes at Top `(63.75,54)`;
  its upward direction becomes Top-leftward.
- Back `(10,0.25)` moving `(0,-0.5)` finishes at Top `(54,0.25)`;
  its upward direction becomes Top-downward.

Return the crossed segments as a short path. Movement, trails, render
interpolation, and replay can consume that path instead of interpolating face
IDs or lerping unrelated chart coordinates.

## The open bottom and exact corners

Preferred policy: no sixth face. The four lower edges form one surface boundary.
Fields have no flux through it. Start with swept specular reflection of the
outward velocity/displacement component, without a rim sensor or avoidance drive.
Body pixels outside the surface are clipped, not mirrored into a second body.
E8 observes this policy on the cube. Add a local, heritable avoidance gain only
if reflection looks wrong or produces persistent piling; zero must remain allowed.
There is no default exclusion strip, kill zone, or teleport to Top. At a lower
side corner the unfolded boundary is straight; test a step that both crosses
a vertical seam and reflects, including an exact tie.

Top vertices are curvature singularities: an exactly vertex-directed straight
path has no uniquely defined continuation. Use a documented deterministic
tie rule (lowest `Edge` index within a geometric epsilon), transition, then
continue the remaining sweep. Guarantee forward progress with a bounded
crossing count and an inward numerical nudge smaller than visible resolution;
record any fallback. Tests must exercise ties and near-ties to expose directional
bias and repeated zero-distance crossings. A fallback may not silently tunnel.

Observed in the M1 implementation (2026-09-11): a sweep aimed exactly at a top
vertex, resolved by the lowest-edge rule, circulates through the incident charts
with zero-length crossings and leaves along its incoming direction with a
rotated tangent map. It is deterministic, counted in `ties`, never triggers
the fallback, and has measure zero; sweeps skewed by 0.02 pixels do not tie.
This outcome is accepted for now rather than replaced by a smoother convention.

Parallel transport across a seam preserves speed and angles. Returning across
that seam is identity. Transport around a loop enclosing a cube vertex can
rotate a heading; demanding identity for every closed loop would incorrectly
erase the cube's curvature.

## Fields and local neighborhoods

Start with a uniform **16×16 cells per face**, 1,280 cells total, each covering
4×4 pixels. Use a single indexed graph with reciprocal cardinal edges and equal
cell areas. Derive seam neighbors from the same adjacency contract at this
resolution. Exchange each shared-edge flux once with equal and opposite changes.
No-flux bottom edges add no exchange. Three faces meet at a top vertex; this
does not create an extra zero-length diagonal diffusion edge.

Keep nonnegative scalar concentrations with a conservative outgoing-flux limiter
or proven stable substeps. Start rendering with nearest-cell samples followed
by an optional normalized, seam-aware one-pixel filter; it changes presentation
only. Upgrade interpolation if M1/E6 reveals distracting block boundaries.
Deposition kernels and localized input kernels use surface distance,
normalize over cells actually present, and account for cell area so the same
event deposits the same total near a seam or rim. Scalar diffusion requires no
rotation; vector transport does. Initial weather flow can steer agents without
adding a full fluid solver.

Start with all unordered agent pairs and the squared 3D chord rejection above:
512 organisms yield 130,816 pairs per pass. Measure cost; this is a work count,
not a performance result. Use the sum of body extents plus reach for contact
broad-phase bounds, not the sensing radius or a centroid-only bite radius.
Introduce bins only when profiling justifies them and the reference verifies
no cross-seam candidate is lost. For sensing, contact, predation, and social steering, unfold candidate
faces into the observer's tangent chart, then choose the shortest valid surface
path. Reject straight segments whose actual edge sequence does not match the
unfolding. Deduplicate by organism ID; a creature near a corner can appear in
more than one candidate image. Transport neighbors' headings into the observer's
chart before computing alignment.

Initially cap sensing radius at 12 pixels and body extent at 9 pixels. Enumerate
all reachable chart paths for that radius, including the two-seam routes around
top corners; these radii are well below one face width. Verify against a slow
reference rather than assuming only direct neighbor faces matter. Use graph
distance for large environmental footprints where cell-scale approximation is
acceptable; do not substitute it for precise bite/contact distance.

The independent slow reference embeds faces in 3D and unfolds each candidate
face path by rigid rotations about its actual shared cube edges. Measure the
straight segment in the resulting common plane, validate its edge intersections
and chart interiors in sequence, and minimize over valid paths. Derive those
rotations from normals/edge endpoints, not the production 2D transition table.
Enumerate all simple face paths for the bounded local query fixtures and include
near-vertex ties. This is a local-query oracle, not an unproved general geodesic
solver for arbitrary paths. Use it for motion, distance, and broad-phase checks.

## Rendering uses the same surface

Build each body in its own local frame. Rasterize destination pixels through
valid unfolded chart images, clipping to the actual surface and choosing one
contribution per primitive/pixel. Rotate directional features at folds. A tail
can remain on one face while the head is on another; a centroid changing face
must not make the whole sprite pop across. At a top vertex, a flat rigid stamp
cannot preserve its shape everywhere: the surface has a 90-degree angular deficit.
For each destination pixel, choose the valid unfolding with shortest surface
path from the anchor; break equal-length ties by lexicographic face/edge path.
Evaluate the body mask in that chosen chart, with one contribution per primitive.
Accept a localized shape discontinuity where chart ownership changes rather than
double brightness. Put this exact fixture in M1 for visual review; do not
promise a perfectly rigid, seamless body around a curvature singularity.

History trails retain actual transported surface segments. Renderer-side pulses
and rings propagate using the same surface graph or valid local unfoldings.
Neither the preview nor the physical output gets its own geometry implementation.

## Evidence required before ecology depends on it

- Exhaust all 16 connected half-edges and all 64 integer edge positions against
  `cube-proto`; verify inverses, reversals, heading rotation, and the four open edges.
- Property-check continuous crossings, reverse paths away from corner ties,
  norm preservation, multi-edge overshoot, rim reflection, and large-dt splitting.
- Cover exact vertices and nearby perturbations without hangs, nonfinite values,
  duplicate neighbors, or unbounded geometric drift.
- Constant fields stay constant; pure diffusion preserves total mass and
  nonnegativity, including at seams, top corners, and the rim.
- The same patch, sensor disk, body, or contact pair moved over a seam does not
  gain energy, brightness, neighbors, or interaction range.
- Compare unfolded distances against an independent slow reference on random
  local pairs. Check heading alignment across the twisted Top seams.
- E1 checks area-weighted long-run occupancy with isotropic, noninteracting
  walkers and pure reflection, using sampling tolerances and correlated-sample
  estimates. The field random walk chooses one of four directions and stays
  put at an open edge; choosing only existing neighbors would bias rim occupancy.
- Check diffusion equivariance under cube rotations that preserve the open
  bottom. A single vertex-adjacent source is not itself three-way symmetric;
  compare rotated source runs, or equal deposits on all three incident cells
  within a local symmetric neighborhood before the lower boundary influences it.
- Review a seam-spanning asymmetric organism, a thick trail, and a diffusing
  patch on both a cube preview and the physical cube. Diagnostic patterns are
  explicit development modes and never part of ambient presentation.

## The ring `w × h` (2026-09-16)

`Topology::Ring { w, h }` is **one chart**, `Face::Front`, `w` by `h` pixels.
**The left and right edges join; the top and bottom are solid.** It adds no new
geometry: the vertical edge is a seam of the chart *to itself* and the two
horizontal edges are the open rim this document already describes. Decided in
[the ring-world plan](flat-world-plan-2026-09-16.md) §2 and §5a; implemented in
FW-1.

### Seam transport

The seam table above gains exactly one row, with the neighbour being the same
chart:

| Exit | Entry | Along-edge parameter | Heading quarter-turns |
| --- | --- | --- | --- |
| Front right | Front left | same | 0 |

and its inverse, Front left → Front right. Nothing is reversed and nothing
rotates: the transport is `TangentMap::IDENTITY` and the crossing is a pure
translation by `∓w`, so a heading, a steering vector and a trail segment all
cross the wrap unchanged. Continuous along-edge reversal, where it applied,
would be `t -> edge_len - t`; on a ring it never applies.

`Face::Top` and `Face::Bottom` of the chart have **no neighbour**. They are the
same open boundary as the cube's four lower edges: no flux, no neighbour, swept
specular reflection (`REFLECT_Y`) for a moving point, and body pixels outside the
surface clipped rather than mirrored. A ring therefore has two rims where the
cube has one, and both behave identically.

Examples in continuous coordinates, at `w = 320, h = 180`:

- Front `(319.5, 20)` moving `(1, 0)` finishes at Front `(0.5, 20)`, one
  crossing, no rotation.
- Front `(10, 179)` moving `(0, 2)` finishes at Front `(10, 179)`, one
  reflection, `REFLECT_Y`.
- Front `(160, 90)` moving `(900, 0)` finishes at Front `(100, 90)`, three
  crossings, four path segments.

### Corners

A ring corner is where the wrap meets a rim, which is precisely the cube's
*lower side corner*: the unfolded boundary is straight, and the existing
lowest-`Edge` tie rule resolves an exactly corner-directed sweep with no new
case. Under `Edge::Top = 0 < Right = 1 < Bottom = 2 < Left = 3` the four corners
do not resolve alike, and **that asymmetry is the contract, not an accident**:

| Corner | Tied edges | Winner | What happens first |
| --- | --- | --- | --- |
| top-left `(0, 0)` | Top, Left | **Top** | reflects, then crosses the seam |
| top-right `(w, 0)` | Top, Right | **Top** | reflects, then crosses the seam |
| bottom-left `(0, h)` | Bottom, Left | **Bottom** | reflects, then crosses the seam |
| bottom-right `(w, h)` | Right, Bottom | **Right** | **crosses the seam, then reflects** |

Whichever order a corner takes, the sweep costs exactly one crossing and one
reflection, the tie is counted in `ties`, the fallback never fires, and the swept
length is preserved. Sweeps skewed by a couple of percent do not tie and resolve
to one edge alone. Exercised at both `S = 1` and `S = 2` in
`crates/cubarium-surface/src/travel.rs`'s ring tests.

### Unfolding and distance

An observer sees **three images** of the one chart — the direct image and the
translations by `+w` (exiting `Edge::Right`) and `−w` (exiting `Edge::Left`) — of
which at most two can be within `Topology::max_local_radius()`, because the two
shifts are `2w` apart and `Topology::validate()` requires

> `w >= 2 · max_local_radius() + 2 · CELL_PIXELS`,

which `Ring::max_local_radius() = min(h, w − 2·CELL_PIXELS) / 2` satisfies by
construction. `MAX_SEAMS` is a cube constant and is not consulted: one crossing
is the whole wrap, and a second could only name `±2w`.

Distance is `Topology::chord_sq` in squared pixels. On a ring it is

> `min(|Δu|, w − |Δu|)² + Δv²`

taken straight from the chart coordinates, so pair rejection is the **true**
distance rather than the cube's conservative 3D chord, and strictly fewer
candidate pairs survive it.

`unfold_pixels` keeps its exactly-once guarantee across the wrap: a body
straddling `u = 0` is carried by the existing two-image machinery and a pixel
visible through two images keeps its shortest one. Bodies clip only at the two
rims.

### Fields

Cells are `4·S` pixels, so 320×180 at `S = 1` and 640×360 at `S = 2` are both
**80 × 45 = 3,600 cells**. **Every row is a ring**, so the horizontal wrap adds
one edge per row: `80·45 + 80·44 = 7,120` undirected edges. Degrees are simpler
than a rectangle's — **there are no corners**: 3,440 interior cells of degree 4
and the 80 + 80 cells of the top and bottom rows at degree 3, since only the two
horizontal rims are open. `w` and `h` must be whole numbers of cells, which
`Topology::validate()` checks along with `cell_count() <= u16::MAX`.

`downhill` mirrors both of the cube's exceptions: `None` on the top row — the
canopy holds its water and detritus exactly as the cube's level Top face does —
and otherwise the neighbour at `(cx, cy + 1)`, which the bottom row does not
have, so it keeps its litter exactly as the cube's rim row does.

### Embedding, and height beside it

`embed()` and `height()` are **different functions on a ring**, and that is the
point. `Topology::height(p) = 1 − 2v/h` is the scalar light, moisture, the bands,
`downhill` and both controllers read: the panel is a side view, canopy at the top
row, soil at the bottom edge. `embed()` drives *position* — noise and weather —
and is the isotropic cylinder

> `θ = 2π·u/w`, `r = w / (2π·32·S)`, `y_e = (h/2 − v) / (32·S)`,
> `embed(p) = [r·cos θ, y_e, r·sin θ]`

with the vertical in slot 1, where the cube puts height. Arc length per pixel is
`1/(32·S)` on **both** axes, the cube's feature scale, so the habitat wave sum
samples at the cube's frequency in every direction, patches are round, and the
field is seamless across the wrap by construction: `u = 0` and `u = w` are the
same point in 3D. No periodic noise, no seeded tiling, no special case. The two
functions coincide on the cube, where height *is* `embed()[1]`; on a ring they
must be asked for separately.

### World scale

`Scale` carries `S`. Cells are `4·S` pixels, the stamp budget is `9·S`, and the
cylinder divides by `32·S`, so **a ring at 640×360 with `S = 2` is the same world
as one at 320×180 with `S = 1`** — the same cells, the same noise scale, the same
ecology — drawn twice as large. `Topology::Cube` is pinned to `S = 1`:
`MAX_LOCAL_RADIUS = 32` and the 9-pixel stamp budget are completeness proofs
about a 64-pixel chart, not tunables, and `validate()` refuses a rescaled cube.
