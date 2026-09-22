---
design_status: exploration
last_reviewed: 2026-09-21
decision_refs: []
---

# Terrain landforms — Sol implementation brief

Wrysk requested implementation following the terrain audit: the current tiers
look like compressed terraced stairs and miss the reference's landscape variety.
This is an authorized implementation brief, not a new accepted ledger decision.
Read WORKING_POLICY.md. Art direction:
[organic terrain, readable masses, chosen cutaway view](../art-direction/Cubarium_Art_Direction_v0.1.md).
Visual reference: `art/gen/runs/2026-09-21-scale-audit/wrysk-concept-ref.png`.
Existing pipeline context: `terrain-generation-briefs-2026-09-21.md`, T1–T4;
the staircase prescription there is the specific interpretation being corrected.

## Outcome

Generate irregular habitat spaces: coherent rock masses, selectively open
chambers, locally terminating shelves, different floor widths and elevations,
basins and occasional drops. Across seeds the large silhouettes and distribution
of open space must vary, not just the noise on continuous horizontal bands.
Keep the tilted cutaway camera and diagnostic renderer. Judge bare geometry
first; production textures, sprites, palettes and asset generation are out of scope.

### Wrysk's visual correction during implementation

Wrysk prefers revision 1 over revision 2: revision 2 has hard-to-follow shapes
and too many weird gaps. Accessibility is **not a hard requirement**. Use the
broader, coherent revision-1 forms as the working visual baseline. The
integrator's subsequent request for more vertical mass is superseded: do not
force tall masses, large chambers or a walkable loop into every seed. Retain
natural variation in height and favor readable continuity of the rock.

## Implementation — Sol, high effort

Own `cubarium-voxel` generation, recipes, hollows, associated unit tests and any
necessary snapshot schema adjustment. Prefer the smallest coherent replacement
of the tier composition over another set of noise multipliers.

- Replace ring-wide equal-depth bands as the shipped presets' organizing shape.
  Give land masses, valleys and shelves different horizontal extents and heights;
  retain continuity across the ring seam and useful space at organism scale.
- Add differently sized open chambers selectively where the supporting mass
  remains coherent. Do not perforate each landform to meet a cavity quota.
  Keep real supporting floors and stable water containment; disconnected
  animal routes are allowed.
- Relax the global all-ground-visible constraint where it erases these forms.
  Permit partial overlap; keep chamber entrances and major occupied spaces readable
  from the existing camera. Do not solve visibility by inventing rendered openings.
- Vary basin location, size and shape. Route a connected source-to-low-lake flow
  through selected basins with genuinely lower, localized spillways and visible
  drops. Preserve the closed water cycle and prevent cavities draining the lake.
- Do not pack more floors into the same tiny footprint. Use the available volume
  deliberately; change preset dimensions only where necessary and keep `small`
  within the panel raster. Preserve voxel metres / organism scale.
- Keep supported configuration and legacy non-tier behavior working where practical;
  remove or revise tests that require the rejected staircase. No pinned snapshots.
  Coordinate any host config/schema changes with the integrator before editing.

Run only the changed crate checks. Return <=40 lines: paths/commit (if any),
structural change, useful dimensions, checks, limitations, commands for captures.
Do not deploy or touch unrelated work. No concurrent edits to verifier's test file.

## Independent verification — Sol, medium effort

Own a new focused integration test file in `crates/cubarium-voxel/tests/`.
Inspect the existing APIs and propose independent checks before seeing the new
implementation. Coordinate with the implementation worker as interfaces settle.
Test useful behavior: supported solid floors, periodic seam consistency,
water containment and actual spill routing, and removal of the global staircase
constraint. Do not require a walkable ring or accessible route between every
floor. Remove walkability as a host seed rejection criterion; it may remain an
observation. The verifier also owns that bounded host change and its tests.
Keep runs to a few hundred ticks at most and avoid brittle art scores.
Do not duplicate implementation or assert exact seeded output. Run the changed
crate once after integration; report concrete failures and limitations.

## Integration and visual acceptance

The integrator inspects current bare-terrain renders of `small` seeds 1, 2, 77
and at least one `default` world. Compare masses, cavities, floor spacing and
water routes to the reference; passing counts alone is insufficient. Use scratch
captures, one normal build cache, no archive. Run the integration check once and
follow the latest-build policy when shipping. No Astra review rounds.
