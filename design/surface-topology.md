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
