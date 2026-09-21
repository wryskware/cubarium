---
design_status: exploration
last_reviewed: 2026-09-21
decision_refs: []
---

# Terrain generation — worker briefs

Plan: [terrain-generation-plan-2026-09-21.md](../terrain-generation-plan-2026-09-21.md),
reviewed and accepted for implementation by Fable on 2026-09-21 with the
decisions below. Branch `terrain-generation`, worktree
`.claude/worktrees/terrain-generation`. Slices land in the plan's order; each is
one commit series on this branch, the commit message is the report.

## Decisions made at review (apply to every slice)

- **Recipe placement.** A serde `Recipe` in `cubarium-voxel` holds every
  generation-only parameter in metres (wavelengths, relief, persistence,
  lacunarity, octave count, later erosion budget) plus named per-pass seed
  streams. `Config` gains a `landform: Landform` field: `Landform::Ridge` is the
  present `generate::landform` unchanged and stays the default, so no fixture,
  harness or test moves in slice 1; `Landform::Staged(Recipe)` is the new
  pipeline. Selected from `[world.landform]` in the host's voxel TOML. Runtime
  water/ecology settings stay where they are in `Config`.
- **Physical units are the contract.** Feature size is fixed in metres by the
  recipe, not by `width` or `voxel_m`. Doubling `width` must add landforms;
  halving `voxel_m` must resolve the same landforms finer. Octaves finer than
  about two voxels are dropped, not aliased.
- **Topology.** X is periodic in every pass, including domain warping and any
  later erosion flux; front/back are walls. Periodic noise must be exactly
  periodic at the ring circumference (lattice period = circumference in metres),
  not a crossfade.
- **Diorama constraint stays.** `visibility_pass` and the front-low/back-high
  landform rule (memory: camera 30°, front never occludes back) still apply
  after the new relief, as an explicit habitat-preparation step. The three
  existing camera/overhang tests must pass for staged rings too.
- **Three named presets** ship with slice 1, as constants on `Recipe`:
  `small` (Tachyon: 160 × 48 × 24 at 0.125 m, 20 m circumference),
  `default` (128 × 48 × 24 at 0.25 m, 32 m) and `wide` (256 × 48 × 24 at
  0.25 m, 64 m). They differ in resolved octaves and landform count, never by
  squeezing the wide landscape into the small ring.
- **No erosion in slice 1.** Material layering (soil from slope, rock, strata,
  bedrock floor, pockets) is carried over from the ridge generator onto the new
  heightfield so the ring is habitable and renderable; slice 2 replaces the
  slope-derived soil with eroded/deposited sediment.
- **Fast iteration policy applies** (`WORKING_POLICY.md`, 2026-09-16 and
  2026-09-17): tests exercise one function on a small world for at most a few
  hundred ticks; run only the crate you touched; no report files, no captures
  in the repo; recipes live in source.

## Slice 1 — staged native ring generator

Owner: generator worker (Opus, high). Files: `crates/cubarium-voxel/src/`
(`generate.rs`, `config.rs`, new `recipe.rs` / `noise.rs`), the host TOML
loader in `crates/cubarium/src/voxel/mod.rs` only as far as `[world.landform]`
needs it, and `config/tachyon/voxel.toml` left on `Ridge` (the panel switches
in slice 3).

Deliverable: `Landform::Staged(Recipe)` producing periodic multi-octave relief
in metres (broad relief, ridged noise in rocky regions with modest periodic
domain warp, intermediate spurs/hollows, restrained fine detail; 4–6 resolved
octaves on `default`), voxelised through the existing layering and
`visibility_pass`; three presets; `cubarium voxel --sink png` renders a
staged ring when the TOML selects it.

Tests to write **before** the generator (they define done; keep each under a
second):

1. Seam: for every preset and three seeds, the surface height and material
   column at `x = width-1` and `x = 0` differ by no more than the largest
   neighbouring step elsewhere on that ring, and the noise field evaluated at
   `x` and `x + circumference` is bit-equal.
2. Bounds: every surface height lies in `[FLOOR_Y, height-5]`; the floor row is
   bedrock; no `Air` below a solid in any column (no overhang), as the ridge
   test already asserts.
3. Same seed, same ring: two `World::new` calls with an identical staged
   `Config` yield identical `material` arrays.
4. Physical invariance: (a) the `default` recipe at `width = 128` and
   `width = 256`, same `voxel_m`, has the same distribution of heights within
   ±10 % in the shared 32 m and roughly twice the count of local maxima over
   the ring; (b) the same recipe at `voxel_m = 0.25` and `0.125` (width
   doubled to keep the circumference) has surface heights in metres that
   agree within one coarse voxel at every coarse column.
5. Camera: the three existing tests in `generate.rs` (`no_surface_cell_is_hidden…`,
   `the_front_stands_below_the_back…`, `…no_overhang`) run against each staged
   preset as well as `Ridge`.
6. Ridge unchanged: with `Landform::Ridge` the default `Config` still produces
   the byte-identical `material` array to `main` for seed 1. (One-off check
   during development against a pre-change build; do not commit a hash pin.)

Visual check for the integrator: one PNG frame per preset for seed 1 and 7,
written to the scratch directory named in the message, produced with
`cargo run -p cubarium -- voxel --sink png --seconds 0.1 --out <dir>` and a
TOML per preset. Not committed.

Return (≤40 lines): commits, which tests were red first, any place the plan's
requirements conflicted with the code, and the PNG paths.

## Slices 2–5

Briefed after slice 1 is integrated. Slice 2 (erosion and soil) will be a
separate authoring pass for its fixtures (sediment accounting, periodic flux,
tiny slope/basin fixtures) before implementation.

## Slice 1 — integrated 2026-09-21

Landed at 2fa65e3, 633bf7c, b06babc. Accepted deviations: whole-ring pooled
statistics for the width test; height and depth doubled with the width in the
voxel test; the seam-step bound taken over the whole ring. Pictures: gentle
rolling relief, one material at the surface, no seam. Erosion supplies the
character.

## Slice 2 — erosion and soil

Owner: the same generator worker (Opus, high). Files: `cubarium-voxel/src/`
(`recipe.rs`, `generate.rs`, new `erosion.rs`), one dev example
`cubarium-voxel/examples/erosion_map.rs`. Nothing in the host.

**First commit — seams from the [caves plan](../caves-and-hollows-plan-2026-09-21.md):**

1. Strata come from a periodic hardness field `hardness(x_m, y_m, z_m) → 0..1`
   held by the recipe (its own stream), replacing the per-column sine carried
   over in slice 1. Ridge is untouched.
2. The staged pipeline is explicit stages with a value between them:
   `Heightfield { bedrock_m, sediment_m }` → `voxelise` → `Volume` (the
   material array) → `prepare` (skyline visibility pass, isolated-void
   repair). Erosion acts on the heightfield; carving will act on the volume.
3. `Recipe.hollows`, a serde-defaulted empty section (a unit struct or an
   empty struct is fine now).
4. No-overhang stays a test over the presets, not a property the voxeliser
   cannot violate.

**Erosion.** A CPU solver on the heightfield with a fixed budget
`recipe.erosion.iterations`; `0` is the identity. Per sample column: bedrock
height, sediment depth, hardness read at the bedrock surface. X periodic in
every pass; front and back are walls. Each iteration: uniform model rain →
downhill routing over the wrapped neighbourhood → stream-power entrainment
(∝ slope × discharge), sediment first, then bedrock divided by hardness →
transport → deposition where capacity falls below load → relaxation of
sediment above an angle of repose. The ring has no exterior drain: water that
reaches a closed basin stops there and its load deposits; find spill levels
with a Priority-Flood over a derived drainage surface, keep the real
depressions. Model water is discarded at the end; it is not the live world's
inventory. Geological time is `iterations`, never ticks.

Accounting each iteration: material removed from bedrock and sediment equals
material deposited plus material in transport; no layer negative; bedrock
never rises. Keep the totals on the heightfield so tests and the dev example
can read them.

Output carried into voxelisation: sediment depth becomes Soil, bedrock
becomes Rock/Bedrock via the hardness field; slice 1's slope-derived soil is
removed for staged rings. Exposed rock where sediment is under half a voxel.
The heightfield also carries a `hard_cap` flag per column: surface hardness
high and a downslope neighbour cut at least `recipe.hollows`-independent
`cap_drop_m` (constant for now, 0.75 m) below — the input for slice 2b's
undercuts. The visibility pass runs after erosion and reports how many
columns it moved; if it moves more than 5 % of a preset's columns, say so in
the return rather than tuning it away.

Tests to write **before** the solver, on tiny fields (about 16 × 4 samples),
each under a second:

1. Conservation: on a tilted plane, after 50 iterations, removed − deposited
   − in transport is within 1e-9 relative of zero; sediment ≥ 0 everywhere;
   bedrock at every sample ≤ its start.
2. Periodic flux: a plane sloping across the seam deposits on the far side of
   `x = 0`; and eroding a cyclically shifted field gives the shifted result
   within floating-point tolerance.
3. Closed basin: a bowl with no exterior drain gains sediment on its floor,
   loses it on its rim, never produces NaN, and its spill level equals the
   lowest rim sample.
4. Hardness: two identical slopes, hardness 0.2 and 0.9, the soft one loses
   more bedrock.
5. Repose: a vertical sediment step relaxes until no adjacent pair exceeds the
   angle of repose by more than one sample's worth.
6. Identity and determinism: `iterations = 0` returns the input; same input
   twice gives the same output.
7. Voxelisation: a column's soil voxel count equals its sediment depth
   rounded to voxels, and a column with sediment under half a voxel shows rock
   at the surface.
8. The slice 1 preset and camera tests stay green with the presets' erosion
   budgets on.

Presets get erosion budgets that finish in well under a second each at their
own sample counts; report the timings.

**Dev example** `erosion_map`: for a preset name and seed, write greyscale
PNGs of bedrock height, sediment depth and discharge for the eroded field to a
directory given on the command line (the crate already depends on `png`). Not
a test; committed as a tool.

Visual check for the integrator: the six PNGs as in slice 1 into the scratch
directory named in the message under `terrain-slice2/`, plus the `erosion_map`
output for `default` seed 1 and `wide` seed 7.

Return (≤40 lines): commits, which tests were red first, where the brief and
the code disagreed and what you chose, erosion timings per preset, how many
columns the visibility pass moved per preset, PNG paths.

## Slice 2 — integrated 2026-09-21

Landed at 8eaf532, a35e384, cef2209; schema 5 → 6. Accepted: basin test
against the final sill with a genuine closed basin; eroded rings agree within
three coarse voxels across a voxel halving (uneroded within one, exact);
staged sweeps at two seeds; `prepare` lowers a moved column whole. Pictures:
rocky uplands, soil and vegetation gathered on depositional flats, real
front-ward catchments. Bedrock incision is small (1–6 m summed per ring); the
sediment mantle does the work. **`hard_cap` is zero on every preset**: no
0.75 m step between neighbouring samples exists in these landscapes.

## Slice 2b — caves, grottos and shelves

Owner: the same generator worker (Opus, high). Files: `cubarium-voxel/src/`
(new `hollows.rs`, `recipe.rs`, `generate.rs`). Nothing in the host, flora or
fauna; the ecology fit (founders on hollow floors, fauna headroom) is a later
package with its own test pass. Plan: [caves-and-hollows-plan-2026-09-21.md](../caves-and-hollows-plan-2026-09-21.md).

**Decisions after slice 2's finding.**

- Undercut sites come from the **hardness field's geometry on a bank**, not
  from the erosion step flag alone: a column whose surface slope toward a
  wrapped neighbour exceeds `hollows.bank_slope` and whose hardness is high at
  the surface (≥ `bedrock_hardness`) and soft (≤ a recipe threshold) in a band
  `hollows.cap_thickness_m` below. The `hard_cap` flag stays as a second
  source and `CAP_DROP_M` becomes `hollows.cap_drop_m`. If, while there, you
  see a cheap way for incision in soft strata to produce real steps (the
  cliffs the art direction wants), propose it in the return with a number; do
  not tune it in.
- **Hollows are derived geometry, not generation state.** `hollows::find(world)
  -> Vec<Hollow>` (cells, floor support faces, mouth cells, whether it meets
  the front cut) is computed from the material array like `isolated_voids`,
  so it is valid after edits and for the founders later. The carve produces
  them; `find` reports them.
- **Camera check lives in the voxel crate**, from `allowed_drop`: a floor cell
  `(x, y, z)` is visible if every nearer column `z' < z` has its skyline no
  higher than `y + allowed_drop(z − z')`… (derive the exact inequality from
  `visibility_pass`; write it as one function and test it against a hand-built
  case). A hollow meeting `z = 0` is visible by definition. A hollow with no
  visible floor cell is filled.
- **Clearance in metres**: `hollows.clearance_m`, default 0.75 m. Voids under
  it are drainage, not hollows, and are not listed.
- The staged presets' **no-overhang test is retired for presets with
  hollows on** and replaced by "every void is sky-reachable" (isolated voids
  empty after prepare). It stays for `Ridge`.
- Carve runs on the `Volume` after `voxelise` and before `prepare`; `prepare`
  then re-runs isolated-void repair after carving and after any column it
  lowers, as the slice 2 code already notes.

**Package i — undercuts, shelves, camera check.** Notch the soft band back
into the bank by `hollows.undercut_depth_m` (recipe, ~0.5–1.0 m), leaving the
hard cap as a roof; where the notch meets a lower support this makes a shelf
under a lip. Bias toward small `z` with `hollows.front_bias`. Periodic in X.

**Package ii — galleries and skylights.** Periodic 3D coherent noise
(reuse `noise.rs`, add a periodic 3D variant if it has none) thresholded
inside soft strata bands below the surface, gated by `hollows.gallery_density`.
Each gallery gets a mouth cut to the nearest bank face within
`hollows.mouth_reach_m` or a skylight shaft where its ceiling is within
`hollows.skylight_m` of the surface; otherwise it is dropped. Bowl floors are
left as they come; the water solver will fill them from the mouth, runoff or
spring in slice 3.

Tests to write **before** the carve, each under a second, fixtures about
16 × 12 × 4 unless noted:

1. Reachability: after carve + prepare, `isolated_voids` is empty on a fixture
   with one sealed candidate gallery and one open one; the sealed one is gone.
2. Clearance: every floor face of every listed hollow has at least
   `clearance_m` of void above it; a fixture slot one voxel under clearance is
   not listed.
3. Camera: the visibility function agrees with a hand-built case (a floor
   under a lip on a front-facing bank is visible; the same floor behind a
   taller nearer column is not); a hollow touching `z = 0` is listed as
   visible; an invisible hollow is filled.
4. Seam: carving a cyclically shifted world yields the shifted hollow list.
5. Undercut fixture: a bank with a hard band over a soft band produces one
   hollow whose roof is the hard band and whose floor is a support face; the
   same bank with uniform hardness produces none.
6. Gallery fixture: a soft band inside rock at depth produces at least one
   gallery with a mouth or a skylight; every listed gallery's cells are
   connected to the sky.
7. Presets: with the presets' `hollows` sections on, every preset at two seeds
   lists at least one hollow, none of them invisible, `isolated_voids` is
   empty, and the skyline camera tests stay green.
8. Water on a bowl: pour a fixed volume into a bowl-floored fixture hollow via
   the existing free-water command and step at most 300 ticks; the bowl holds
   free water and the ledger balances. (Uses `World::step`; keep the fixture
   tiny.)

Presets get hollows sections that produce a handful of grottos per ring, not a
honeycomb; the ring must stay walkable along its length. Report counts per
preset and seed.

Visual check for the integrator, into the scratch directory under
`terrain-slice2b/`: the six preset × seed PNGs as before, plus for `default`
seed 1 a second render at `px_per_voxel = 8` (TOML) so the hollows can be
read, and the `erosion_map` output is not needed.

Return (≤40 lines): commits, red-first tests, disagreements and your choice,
hollow counts per preset and seed (undercuts / galleries / filled as
invisible), any proposal on incision, PNG paths.

## Slice 2b — integrated 2026-09-21

Landed at e219af6, 2de2bea; schema 6 → 7. Accepted: seam test as rotation of
the material array; mouths and skylights reverted when they break the
landform rule; `halving_the_voxel` with hollows off; `small` strata 1.6 m.
Finding: **undercuts carve zero on every preset** because no surface drop of
3 voxels exists between neighbouring columns; listed hollows are 1–5 shallow
galleries per ring, all at the front cut, and at 8 px/voxel they read as
small dark slots only. The machinery (carve, `hollows::find`, camera check,
galleries, skylights) is in and tested; the landscapes lack banks.

## Slice 2c — layer-aware incision

Owner: the same generator worker (Opus, high). Files: `erosion.rs`,
`recipe.rs`, `generate.rs` tests; nothing outside `cubarium-voxel`.

Adopt the worker's proposal from slice 2b: the stream-power bedrock cut reads
hardness **at the cell being cut**, and the per-iteration cap is a recipe pair
`erosion.max_cut_soft_m` / `erosion.max_cut_hard_m` applied by whether the
cell's hardness is ≤ `hollows.soft_hardness`. Start at 0.25 m / 0.05 m (the
5:1 the proposal gives). This changes every preset's terrain; the point is
banks, steps and rocky shoulders where a hard band caps a soft one, so
undercuts have something to cut into. The plan's section 2 asks for exactly
this ("banks, rocky shoulders and depositional flats"; "exposed rock on
erosional slopes").

Hard constraint: **the ring stays walkable.** Add a traversability check in
the voxel crate: over all support faces (any `z`), with a step bound of
`0.5 m` in `y` between 4-neighbouring faces (wrapping `x`), a closed route
around the ring exists. Run it as a test on every preset at two seeds. If
incision breaks it, the fix is a ramp or a lower cap, reported, not a
silently loosened bound.

Tests before the change, each under a second:

1. Differential incision: a channel fixture with a hard band over a soft band
   ends, after the preset iteration count, with a step of at least 3 voxels
   between the capped column and its cut neighbour; the same fixture with
   uniform hardness ends with no step over 1 voxel.
2. Conservation and the bedrock-never-rises rule still hold under the new cap
   (extend test 1 of slice 2 to the new fixture).
3. Traversability as above, on the three presets at two seeds, and on a
   fixture with a 3-voxel wall across the whole depth (must fail) and the same
   wall with a one-column ramp (must pass).
4. Presets: with 2c on, `default` and `wide` list at least one undercut at
   both seeds; `hollows::find` has no invisible hollows; `isolated_voids` is
   empty; the skyline pass stays under 5 %; every slice 1, 2 and 2b test is
   green.

Report per preset and seed: columns with a 3-voxel drop to a 1-cell
neighbour, undercut / gallery / dropped counts, skyline-pass percentage,
traversability result, erosion time.

Visual check into the scratch directory under `terrain-slice2c/`: the six
preset × seed PNGs, plus `default` seed 1 and `wide` seed 7 at
`px_per_voxel = 8`.

Return (≤40 lines) as before.

## Slice 2c — integrated 2026-09-21

Landed at e88080b, d5fcd82; schema 7 → 8. Result: the mechanism is sound on a
bare fixture (2.10-voxel step against 1.09) and **does nothing on the presets**.
Incision there is supply-limited: the sediment mantle satisfies the flow before
it reaches rock; the deepest cut anywhere is 0.34 m, and sweeping both caps
over 1:1 to 50:1 gains no 3-voxel step. A detachment-limited term was tried
and reverted: it buried the landscape in debris (bare rock 24–43 % → 2.5–7 %).
Walkability held everywhere. `small`'s erosion cap was lowered to keep the
skyline pass under 5 %. Kept: the traversability check, the recipe cap pair.
Conclusion: stream-power erosion at this scale and budget will not make
cliffs; the steps have to come from structure.

## Slice 2d — structural benches

Owner: the same generator worker (Opus, high). Files: `generate.rs`
(`heightfield`), `recipe.rs`, `hollows.rs` only if the undercut pass needs a
parameter; nothing outside `cubarium-voxel`.

**Decision.** Cliffs and ledges come from geology, not from the solver. Where a
hard stratum outcrops in a rocky region the bedrock surface is **benched**: it
follows the top of the hard band until the relief has fallen a band's
thickness, then steps down to the next band's top. Soft regions stay smooth.
Erosion then does what it already does well (mantle on flats, exposure on
slopes); the sediment-only repose rule lets bedrock faces stand and drops a
talus at the foot; and the undercut pass finally has hard-over-soft faces to
notch, so grottos appear under ledges. This is the "layered design" and the
art direction's exposed layers and abrupt cliffs, produced directly.

Implementation sketch, the worker's call on details:

- In `heightfield`, after relief and before erosion, compute for each sample
  the hardness column below the raw surface. With `recipe.benches.strength`
  (0..1, in the rocky mask only, weighted by the mask) pull the bedrock
  surface toward the nearest hard-band top at or below the raw surface. At
  strength 1 the rocky surface is a staircase of band tops; at 0 it is the
  slice 1 surface. The bench face height is the strata spacing (`strata_m`,
  1.6 m on `small`, whatever `default`/`wide` carry), so faces are 4–7 voxels.
- The pull is periodic in X and continuous where the rocky mask fades, so a
  bench ends in a ramp, not a wall. Do not bench within `benches.ramp_m` of a
  mask edge.
- Faces facing the camera (step down toward the front) are the visible
  cliffs and the undercut sites; faces facing away are what the skyline pass
  lowers, so bias the benching so that band tops, not band bottoms, sit at
  the front of a bench (i.e. a bench slopes gently down toward the back
  before its face). If the skyline pass climbs past 5 % on a preset, say so.
- The undercut pass keys off the bench faces: hard cap at the top, soft band
  below, notch back by `undercut_depth_m`. Expect undercuts > 0.

Tests before the change, each under a second:

1. Bench fixture: a rocky column band with two hard strata and a linear
   ramp of relief across one band thickness ends with the bedrock surface on
   the upper band top for the upper part and on the lower band top for the
   lower part, with one step of at least one band thickness between them; a
   soft-region copy of the same ramp is unchanged.
2. Ramp-out: at the rocky mask's edge the surface difference between benched
   and unbenched is continuous (no step over one voxel across the edge).
3. Repose keeps bedrock: a 6-voxel bedrock step with a one-voxel sediment
   veneer, after `relax`, keeps its bedrock step and has its sediment at the
   foot (extend the existing repose test).
4. Presets: with benches on, `default` and `wide` list at least one undercut
   at both seeds and at least one column pair with a 3-voxel drop to a 1-cell
   neighbour; all hollows visible; `isolated_voids` empty; walkable; skyline
   pass under 5 %; all earlier tests green.

Report per preset and seed as in 2c plus bench face count. Presets carry
`benches` sections that produce a few benches per rocky region, not a
staircase everywhere; `small` may need `strength` lower than the others.

Visual check into the scratch directory under `terrain-slice2d/`: the six
preset × seed PNGs plus `default` seed 1 and `wide` seed 7 at
`px_per_voxel = 8`.

Return (≤40 lines) as before.
