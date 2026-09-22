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

## Slice 2d — integrated 2026-09-21

Landed at e3a1b63; schema 8 → 9. **Works**: default/wide carry 25–47
three-voxel steps, 31–62 bench faces and 6–12 undercuts per ring; all hollows
visible, no sealed voids, walkable, skyline pass ≤ 3.8 %. Accepted:
`benches.strength` 1.0 on default/wide, 0.6 on small; undercut gate 0.70 /
front bias 0.5; mouth test against the neighbour's rock (a sill is still a
mouth); the invariance tests run with benches off; `seal_unreadable_shafts`
(a real camera bug: a shaft safe while a neighbouring gallery was open stops
being safe when it is filled). Open: **`small` gets ledges but no grottos** —
its 1.6 m band at 0.6 pull gives a 7.7-voxel face and a notch with 0.75 m of
clearance under a 2-voxel cap needs 9. Wrysk's call: accept ledges only on the
Tachyon ring, or raise `small`'s band / pull, or lower its clearance.
Pictures at 8 px/voxel: flat-topped benches, terraced strata, dark notches
under the ledges — the first slice that reads as the layered design.

## Wrysk, 2026-09-21: go for slice 3; redo `small`

"Raise small's pull to 1.0. Currently don't like its preset anyway, so feel
free to completely redo it."

## Package S — the `small` preset, redone

Owner: generator worker (Opus, medium). Files: `recipe.rs` (the `SMALL`
constants) and its tests only. Runs in parallel with slice 3; do not touch
files outside `recipe.rs` without saying so.

`small` is the Tachyon ring: 160 × 48 × 24 at 0.125 m, 20 m circumference,
6 m tall, 3 m deep, 4 px/voxel on a 640 × 360 raster. Redo its recipe from
the picture's needs, not from `default` scaled down: `benches.strength` 1.0;
strata spacing and pull chosen so a grotto with the shared 0.75 m clearance
fits under a cap (a face of at least 9 voxels at 0.125 m = 1.125 m, so bands
of about 1.25–1.5 m); two or three rocky regions with benches and grottos,
soil valleys between, at least one closed basin that can hold a pool
(`spill_m` above its floor); relief that uses most of the 6 m without
touching the ceiling; erosion budget that keeps bare rock around a quarter to
a third of columns and the skyline pass under 5 %. Keep it walkable.

Verification: the preset assertions (undercuts > 0 at two seeds, all hollows
visible, no sealed voids, walkable, skyline < 5 %) now include `small`. Report
the same per-seed table as 2d. Visual check: `small` at seeds 1, 7, 77 at
4 px/voxel and seed 1 at 8 px/voxel, into the scratch directory under
`terrain-small/`.

## Slice 3 — water and founders

Owner: a new habitat worker (Opus, high). Files: `cubarium-voxel/src/`
(new `hydrate.rs`, `world.rs` for a settle API), `cubarium/src/voxel/habitat.rs`
and `mod.rs`, `cubarium-voxel-fauna/src/step.rs` (headroom),
`cubarium-voxel-flora` only for a shared suitability predicate if one is
missing, `config/tachyon/voxel.toml`. Do not touch `recipe.rs`'s `SMALL`
constants (package S owns them) or `generate.rs`/`erosion.rs`/`hollows.rs`
except to read.

Plan sections 4 and 5 are the spec; the decisions below fix what they leave
open. Read the plan's "What exists now" for `habitat.rs:119–334`.

**Decisions.**

- **Inventory first.** `Recipe.water` (serde-defaulted for Ridge) states the
  total water as metres over the footprint (`inventory_m`) and its split:
  `atmosphere_fraction` reserved for the cycle, `aquifer_head_m` for the
  table, the rest available to pools and pore. Nothing is charged
  independently to a full target; the four stores sum to the inventory and
  `Ledger::initial_*` records them as today.
- **Pools from geometry.** `hydrate` fills closed basins to `min(spill_m,
  what the remaining inventory allows)` at one head per basin, lowest basins
  first, using the flood's own basin labelling; a grotto floor that is a bowl
  (`hollows::find`) is a basin like any other. Pore moisture is set to field
  capacity in soil below the water table and within one voxel of a pool, per
  the existing retention rules; elsewhere it is the current default. No
  perched tables except where the geometry holds them.
- **Settle is measured, not assumed.** `World::settle(cap_ticks) ->
  Settle { ticks, converged, pooled_m3, free_cells, pore_m3, drift_m3_per_100,
  dry_locked }` replaces the host's `SETTLE_TICKS = 400`. Convergence: over
  the last 100 ticks, change in pooled volume and in wet-cell count both
  under 1 %; cap 600 ticks. `dry_locked` is true when the atmosphere holds
  more than the shower trigger and no pool exists. The host logs the report
  once at startup; the ordinary display shows nothing.
- **Founders on support faces.** Replace `skyline_of` with an enumeration of
  support faces (any `y`, including hollow floors) and score each with the
  flora crate's own establishment gates (`establishment_gates_with_sky`) plus
  an adult-maintenance check: light response × the species' rate at that
  site covers its adult upkeep. If that check needs a predicate the flora
  crate does not expose, add it there and call it from the host; never copy
  the rule into the host. Patch centres with a minimum spacing in metres,
  colonies grown over connected suitable faces (use the traversability
  neighbourhood), gaps kept. Counts by usable area × target coverage with a
  small-world cap, recomputing light as crowns are placed. Logs and litter
  before glowcaps and shredders, booked in the ledger.
- **Fauna after food.** Browsers need: a support face with headroom for the
  body, water under wade depth, reachable foliage within mouth reach, and a
  walkable route to a second foliage patch. **No foodless fallback**: a
  species that cannot be placed is reported as a shortfall in `Seeded` and
  not placed. Headroom: `faces_in_column` rejects a face whose void above is
  shorter than the body's height, derived from the species' existing
  geometry (`mouth_reach_up_voxels` or the body scale it already carries);
  do not add a new species parameter unless nothing there fits, and say so.
  This is a model-rule change: its tests are listed below and stay in the
  fauna crate.
- **Defaults flip.** The host's generated scene (`cubarium voxel` with no
  TOML) uses the staged `default` preset; `Landform::Ridge` remains
  `Config::default()` for fixtures and is selectable with `landform = "ridge"`.
  `config/tachyon/voxel.toml` selects `preset = "small"` and its `[world]`
  drops the rain/evaporation/aquifer keys the recipe now decides.
- **Not in this slice**: UI regenerate/keep-candidate controls (slice 4;
  restart with `--seed` is the regenerate for now), any tuning of species
  rates, any run past the settle cap.

Tests to write **before** the code, each under a second unless marked:

1. Inventory: after `hydrate`, surface + pore + aquifer + atmosphere equals
   `inventory_m × footprint` within 1e-9 relative on a fixture with two
   basins; the deeper basin fills first; a fixture with inventory below the
   first spill leaves both basins partly filled at one head each.
2. Grotto pool: a bowl-floored hollow fixture receives water in `hydrate`
   and holds it after `settle`.
3. Settle: on a fixture already at rest, `settle` returns in ≤ 100 ticks
   with `converged`; on a fixture with a fresh column of free water above a
   basin it converges before the cap and pooled volume ends within 1 % of
   the poured amount; `dry_locked` is true on a fixture with no pools and a
   charged atmosphere. (≤ 600 ticks on tiny worlds; keep under a second.)
4. Support faces: on a fixture with a roofed shelf, the founder site list
   includes the shelf's floor and the ground under it, and excludes the roof
   top when it has no headroom.
5. Suitability: a bloomcrown is not placed on a shaded hollow floor; an
   umbrellafrond is; a glowcap is placed only where litter was booked first.
6. Shortfall: on a fixture with plants but no reachable second patch,
   browsers are not placed and `Seeded` reports the shortfall; on the
   authored scene they are placed.
7. Headroom (fauna crate): `faces_in_column` lists a floor with body-height
   headroom and rejects one with one voxel less; an animal on a floor keeps
   its face when a roof appears above its headroom and loses it when the roof
   drops below.
8. Host: `cubarium voxel` with no TOML generates the staged default; the
   Tachyon TOML parses to `small`; both seeded habitats step 60 ticks with
   closed ledgers (extend `the_seeded_habitat_steps_with_closed_ledgers`).

Visual check into the scratch directory under `terrain-slice3/`: `default`
seed 1 and `wide` seed 7 at 4 and 8 px/voxel after settle and founders, plus
the settle report and `Seeded` printed for each.

Return (≤40 lines): commits, red-first tests, disagreements and choices,
settle ticks and pooled m³ per preset, founder counts and shortfalls per
preset, PNG paths.

## Package S and slice 3 — integrated 2026-09-21

Package S landed at ab4a6db (small redone from its own picture: strata 1.4 m,
own bedrock threshold 0.78, regional hardness 5 m, depth scale 0.2, three
rocky regions; 4–11 undercuts per seed; seed 1's basin is a puddle).
Slice 3 landed at 6474f9e, 4497240, e260844, 3d80bb4; schema 9 → 10;
workspace 2042 tests green in 4.1 s. Accepted deviations: a 3D sheet-by-level
basin flood on the material array (sees grotto bowls, works for Ridge and
fixtures); allocation by catchment with per-basin spill caps; all porous
voxels wetted to field capacity and charged to the inventory (the dry
default planted nothing); atmosphere charged only under a closed budget;
headroom = 1 + mouth reach off the manifest; route to a second patch over
level-adjacent faces; `scene::authored` charges the default inventory.
Pictures: pools in the low catchments, glowcaps on the wet flat under the
ledge, bloomcrowns banded along the terrace, no shortfalls on any preset.

Open from the worker: the staged worlds run an **open budget with rain and
evaporation at zero** — hydrostatics only, no cycle — and the panel TOML has
always been that way. Every preset reports "still moving at the cap" with a
drift of 1e-10 m³ per 100 ticks, so the flag is the wet-cell criterion, not
the water.

## Package W — the cycle on, and a settle flag that means it

Owner: habitat worker (Opus, medium). Files: `recipe.rs` `Water` section and
the staged presets' `water` values, `world.rs` settle criterion, host startup
log; `config/tachyon/voxel.toml` only if a key must move.

Decision, applying the hydrology decision of 2026-09-20 (route B: closed
cycle with a lumped atmosphere, now; drought lock is a feature, reported not
hidden): staged recipes run `closed_water_budget = true` with
`atmosphere_fraction` charged, evaporation above zero, and the runtime's
shower rule. Take the shower trigger, shower volume, rain rate and evaporation
rate from the water-cycle handoff's chosen values where it states them and
from `Config`'s documented defaults otherwise; state the four numbers in the
return. `Ridge` and every fixture stay on the open budget as before.

Settle: `converged` means pooled volume drift under 1 % **and** wet-cell
change under 1 % of the pooled cell count with an absolute floor of 8 cells
over the last 100 ticks, so a resting world reports converged; keep the cap
at 600. `dry_locked` keeps its meaning. The startup line then names the
budget it runs (closed, with the atmosphere's share) instead of "no cycle to
report".

Tests before the change, each under a second: (1) a staged tiny world under
the closed budget passes `validate_loaded`, holds atmosphere equal to its
`atmosphere_fraction` share after `hydrate`, and its ledger balances after 60
ticks; (2) the resting fixture from slice 3 now reports `converged` before
100 ticks; (3) a fixture with a charged atmosphere and no pools is
`dry_locked`; a fixture with pools is not. Plus one `#[ignore = "study: run
by name"]` on `default` seed 1: settle, then step until the first shower or
6000 ticks, and print when it fired and what it delivered. Run it once and
put the two numbers in the return; it is not a test.

Visual: `default` seed 1 at 8 px/voxel after settle into the scratch
directory under `terrain-slice3w/`.

Return (≤30 lines): commits, the four cycle numbers, settle results per
preset, the study's two numbers, PNG path.

## Package W — integrated 2026-09-21

Landed at 704df08; schema 10 → 11. Cycle numbers from the water-cycle
handoff's bounded study: shower rate 2e-4 m/s, evaporation 1e-4 m/s, trigger
0.02, shower 5 m³. Finding: **at trigger 0.02 it rains back to back** (one
shower takes ~2600 ticks, the next begins at once) and no preset's settle can
converge because the drift is the rain; `atmosphere_fraction` 0.06 starts the
sky above the trigger so it rains from tick 0. Two slice-3 tests that an edit
slip had cut are restored.

## Package W2 — intermittent weather

Owner: habitat worker (Opus, low). Files: the staged presets' `water` values
in `recipe.rs`; the study; nothing else.

Decision, from the handoff's own range (intermittent weather at 0.10–0.24,
dry lock at 0.26 and above): `shower_trigger_fraction` **0.12** and
`atmosphere_fraction` **0.08** on all three staged presets, so the world
settles its hydrostatics first and the first shower follows once evaporation
has lifted the difference. Nothing else moves.

Check: settle on the three presets (expect `converged` before the cap now),
and the study rerun on `default` seed 1 reporting the tick of the first
shower after settle, the tick of the second, and the dry spell between them.
If the first shower does not arrive within the study's 6000 ticks, report it;
do not move the numbers again.

Return (≤15 lines): commit, settle per preset, the three study numbers.

## Package W2 — integrated 2026-09-21

Landed at b057da7: trigger 0.12, share aloft 0.08, via `Water::DEFAULT`.
Settle: `default` converges at 523 ticks; `small` and `wide` drift small and
negative (draining into soil) and miss the 100-tick window by tick 600. Study
on `default` seed 1: **no shower in 6000 ticks**; the sky rises 4.8e-5 m³ per
tick against an 11.52 m³ trigger, so the first rain is about 74 000 ticks
(62 simulated minutes) out in the bare voxel crate — an upper bound, since
the live world's transpiration feeds the same store. Not a dry lock. The
study now settles with the outlet shut and opens it afterwards, the host's
order. Weather cadence is Wrysk's call: roughly hourly as it stands, or a
lower trigger for more frequent showers (0.10 is the handoff's floor for
intermittent weather).

Branch state: 24 commits over `main` at 59a0dd6; fast-forward is clean.

## Wrysk's decision on weather cadence — 2026-09-21

"i would like to see a 5-15 minute cadence for rain in ambient worlds, with
some preference towards interval randomization if possible." Merge and panel
deploy follow once cadence lands (a separate cheap thread).

## Package W3 — scheduled showers

Owner: habitat worker (Opus, medium). Files: `config.rs`, `world.rs`,
`water.rs` (`shower`), `snapshot.rs` (SCHEMA 11 → 12), `recipe.rs`
(`Water` + `cycle_into` + `Water::DEFAULT`), the study. Host untouched
unless the startup water log wants the interval printed (one line, optional).

Why a scheduler and not a lower trigger: on `default` the sky lifts
4.8e-5 m³ per tick (≈0.058 m³ per simulated minute). A trigger-only cycle
fires whenever the store crosses the line, so its period is set by the return
flux and its intervals are regular. Wrysk wants 5–15 min with randomised
intervals: that is a timer drawn from the world's seed, with the store's
content deciding only whether the due shower can fall.

Mechanism (decided; the interface is fixed, the numbers are the study's):

- Two new `Config` fields, seconds of simulated time:
  `shower_interval_min_s`, `shower_interval_max_s`. Default `0.0 / 0.0` =
  **no scheduler**: the trigger-only behaviour every fixture and every
  existing closed-budget test has, unchanged. `validate` rejects
  `min > max`, negative or non-finite values.
- With `max > 0` the world carries `next_shower_tick: u64` (in the
  snapshot; SCHEMA 12, old snapshots refused, no migration). Drawn at
  creation and again each time a shower **ends**: `end_tick + U[min, max]`
  in ticks (`TICK_HZ` 20), from a splitmix64 stream over
  `(config.seed, "weather", ledger.showers)` — the generator's own idiom in
  `generate.rs:53` / `noise.rs:62`; same seed, same sequence; no clock.
- At the due tick a shower starts if the store holds at least
  `shower_trigger_fraction × expected_total` (the trigger is repurposed as
  the **availability floor** and lowered accordingly); otherwise nothing
  falls and the shower starts on the first later tick the floor holds.
  Drought lock stays a feature: an empty sky never rains. Delivery stays
  `min(shower_volume_m3, store)` as `water.rs:502` does today.
- `Water` gains the two interval fields; `cycle_into` copies them.
  `Water::DEFAULT` sets **300 s / 900 s** for all three staged presets.
- The **numbers** — `shower_volume_m3`, `rain_m_per_s`,
  `evaporation_m_per_s`, `atmosphere_fraction`, the floor — are yours to set
  from the study against these targets, on `default`, `small` and `wide`
  seed 1, 60 simulated minutes after settle: every due shower starts on its
  due tick (never held by the floor); dry spells spread across the 5–15 min
  range, not clustered at one end; each shower visibly rains for at least
  30 s and at most about 3 min; the store never dry-locks; the water
  residual stays under 1e-6 of total. A 5 m³ shower cannot be fed by a
  0.058 m³/min lift, so expect the shower volume to come down toward what
  one interval lifts (transpiration adds to it live) and the rain rate to
  come down so the shower lasts long enough to read. Say what you chose and
  the lift you measured.

Tests, written before the mechanism, each under a second, in `water.rs`
beside the closed-budget tests unless noted:

1. `the_first_due_tick_is_drawn_inside_the_interval`: `World::empty`,
   closed budget, interval 300/900 s; `next_shower_tick()` in
   `6000..=18000`; seeds 1–4 give at least two distinct values; seed 1
   twice gives the same value.
2. `a_due_shower_starts_on_its_tick_and_not_before`: the tiny soil slab
   from `viability.rs:313`, `initial_atmosphere_m3` well above the floor,
   floor 0, interval fixed 2/2 s (40 ticks). `showers == 0` through tick 39,
   `== 1` at the tick the due shower starts (state the off-by-one you chose
   in the assertion message), stays 1 until the next due tick, which is at
   least 40 ticks after that shower ended.
3. `a_starved_sky_holds_the_shower_until_the_floor`: as 2 with
   `initial_atmosphere_m3` 0 and a positive floor. At the due tick and 100
   ticks past it, `showers == 0`. `Command::AddAtmosphere` over the floor;
   a shower starts within one tick; `total_residual().abs() < 1e-9`.
4. `no_interval_means_the_trigger_alone`: interval 0/0 on the fixture of
   `a_store_under_the_trigger_never_rains` (`water.rs:2654`) behaves as that
   test does; plus the existing closed-budget tests pass **unmodified** —
   that is the regression check, do not edit them.
5. `validate` rejects `min > max` and a negative interval; accepts 0/0
   (in `config.rs` tests).
6. Snapshot round trip carries `next_shower_tick` (extend
   `the_store_survives_a_round_trip...` at `water.rs:2750`); SCHEMA 12 and
   the existing refuse-old-schema test still passes.
7. `cycle_into` copies both interval fields (`recipe.rs` tests).

Study: replace the W2 `#[ignore]` study with one that settles, opens the
outlet, then runs 60 simulated minutes and prints per shower: start tick,
dry spell before it in minutes, whether the floor held it, delivered m³,
rain duration in seconds; then store min/max and the lift per minute.
Run it on the three presets and put the digest in the return; it is not a
test.

Visual: none needed — no geometry moves.

Return (≤30 lines): commits, the numbers chosen and why, per-preset study
digest (shower count, dry-spell range, held count, rain seconds range,
store min), test count for `-p cubarium-voxel` and `-p cubarium`.

## Package W3 — integrated 2026-09-21

Landed at 3e2aa01; schema 11 → 12. Mechanism as briefed: seeded interval
scheduler (`shower_interval_min_s/max_s`, `next_shower_tick` in the
snapshot, drawn at each shower's end from splitmix64 over seed and shower
count), trigger repurposed as the 1 % availability floor. Numbers from the
study: rain 3.5e-5 m/s, evaporation 3.0e-5 m/s, shower volume scaled to
footprint (`Water::SMALL` 0.125, `DEFAULT` 0.4, `WIDE` 0.8 m³) so every ring
rains for one readable minute. Sixty minutes after settle, seed 1: five
showers per preset, gaps 6.1–15.3 min, none held by the floor, store never
near it; lift covers delivery on `small`, breaks even on `default`, runs
0.009 m³/min short on `wide` (about a day of headroom before transpiration,
which the live world adds). Known: all presets share one gap sequence at a
given seed (seed and shower count only). Fable ran `-p cubarium-voxel -p
cubarium`: 778 passed, 1 skipped (study).

Branch state: 31 commits over the fork point 59a0dd6. `main` has since
gained the stage-2 art-pass commits (through 262c727); no file overlap, the
merge is clean, but it is a merge commit now, not a fast-forward. Wrysk
authorised a cheap thread to merge and deploy the Tachyon panel once cadence
landed; dispatched after this note.
