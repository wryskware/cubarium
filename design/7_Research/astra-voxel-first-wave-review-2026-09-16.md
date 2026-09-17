---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Voxel ringworld first wave: round 1 boundary review

**Latest status — after repair 2: clears the voxel ringworld first wave at HEAD 06b4b6f; round 2 repair cases are resolved, with the solver and presentation limits below.**

Reviewed the handoff, four-package brief, voxel types/contracts, Package D and
prior proposal §§2, 4 and 8 in the requested order after the working policy.
The tree advanced beyond stubs; additions below are not tested implementations.
No generator, water or snapshot bodies were reviewed. No builds, tests, captures
or long runs were performed. Only this requested note was written.

## 1. P1 — Define spill transfer separately from common-level membership — before code lands

`design/handoffs/voxel-first-wave-briefs-2026-09-16.md:44-54` leaves “water-bearing”
ambiguous: wet-only connectivity cannot recruit dry passage cells; unrestricted
air connectivity joins hollows through sky above a dry ridge. The newer contract
at `crates/cubarium-voxel/src/water.rs:25-44` improves this with a candidate-level
test, but common-level membership alone does not specify spill transfer.
For beds `[0,1,0]` and left input 1.4, including both bottom cells makes the level
0.7, below the sill. Rejecting that merged region is correct; refusing all
transfer to the far hollow is not. The target is left 1.0, right 0.4, dry sill.

**Proposed change:** define regions as face-connected void volumes connected
below their supported water level, admitting initially dry voids and ignoring
roofs as a sky requirement. Separately transfer only donor volume above a reached
spill crest into lower receiving space; split regions when the connection dries.
Recompute membership after transfer; never retain an old connection through air.
A submerged bottom passage must remain connected and give a common level in the
U-tube/roof fixture. A ridge above donor level admits no transfer. Specify whether
the expected result is one settled substep or a short sequence, with tolerance.
Keep the listed fixtures; no pressure solver is warranted by these cases alone.

## 2. P1 — The spill fixture needs walls and explicit input placement — before code lands

`design/handoffs/voxel-first-wave-briefs-2026-09-16.md:59-65` names beds and volumes
without the enclosing geometry. `crates/cubarium-voxel/src/config.rs:62-67` wraps
x: in a width-three fixture, the two hollows directly adjoin across the seam.
They can equalize below the ridge legitimately. Also, a single `AddWater(1.4)`
cannot place 1.4 voxel volumes in one cell (`crates/cubarium-voxel/src/world.rs:10-12`).

**Proposed change:** specify unit-area interior beds, enclosing solid walls above
the maximum water level, and stacked left-column injection whose accepted sum is
0.6, 1.4 or 3.2 voxel volumes. Account for `World::empty`'s foundation when naming
bed elevations. Expected depths are `[0.6,0,0]`, `[1,0,0.4]`, `[1.4,0.4,1.4]`
(`design/terrain-and-ecosystem-proposal-2026-09-16.md:359-366`). Shift the *whole*
fixture and inputs across x=0 and compare the inverse-shifted per-cell stores,
not just their sum. Include a mirrored wet source to expose index-order bias;
`crates/cubarium-voxel/src/water.rs:35` explicitly uses index order to break seed ties.

## 3. P2 — A corrected residual is not the raw conservation error — before code lands

`design/handoffs/voxel-first-wave-briefs-2026-09-16.md:32-33,62-63` promises a
residual below 1e-9. The live `crates/cubarium-voxel/src/ledger.rs:17-34` adds
`rounding_m3` to expected storage. That changes what the assertion establishes:
an arbitrary transfer imbalance booked there would also make the residual pass.
The f32 fractions are explicit at `crates/cubarium-voxel/src/world.rs:29-32`.

**Proposed change:** update the brief to distinguish raw error
`stored - initial_stored - net_in` from corrected error `raw - rounding_m3`.
Book rounding only from actual numeric casts, with a bound from the affected
stores' representable spacing; never infer it by closing the global ledger.
One small transfer should check both the corrected residual and bounded rounding.
If `free_transfer_cap` is used (`crates/cubarium-voxel/src/config.rs:31-34`), scale the balanced transfer
together; independent per-cell clamps must not be repaired through rounding.

## 4. P2 — Material edits must preserve volume, not saturation — before code lands

`crates/cubarium-voxel/src/world.rs:13-15` and the brief at lines 56-57 describe
displacement without defining retained water or “nearest.” Material capacities
differ (`crates/cubarium-voxel/src/material.rs:22-28`): a half-saturated soil voxel
holds 0.175 voxel volumes; half-saturated rock holds only 0.01.

**Proposed change:** convert old free/pore fractions to cubic metres before the
edit, retain only what fits the new material's capacity, and displace the excess.
Soil-to-air must release its pore volume as free water; soil-to-rock must not
silently retain the old fraction. Define nearest by wrapped face-adjacent void
path distance from the edited cell, with no tunnelling through unrelated solids;
share equally among equal-distance recipients before considering farther ones.
The live distance-then-index doc (`crates/cubarium-voxel/src/water.rs:681-682`)
needs that tie rule if edits are also to respect seam shifts. Check one wet-soil
replacement and one full recipient; export only volume with no reachable room.

## 5. P2 — Make command receipts and timing unambiguous — before code lands

`crates/cubarium-voxel/src/world.rs:10-11` says refused water is reported in the
return value; lines 189-190 say it returns accepted volume. `RainPulse` says
“this tick” at line 8 while `apply` says “now”; negative aquifer removal is
allowed at line 16 but its receipt/accounting sign is not specified.

**Proposed change:** document immediate application, including while paused;
return actual accepted m³, with rejected input equal to requested minus accepted.
For aquifer removal return a negative accepted amount, capped by available stock,
and make `user_in` explicitly signed. Return zero for non-water commands. Reject
nonfinite water amounts and negative additions other than `ChargeAquifer`.
One clipped addition and one overlarge aquifer withdrawal should check the
receipt, store delta and ledger together. Do not book refused rain/addition as input.

## 6. P2 — Pin down soil drainage and spring limits — can wait for round 2

The brief at `design/handoffs/voxel-first-wave-briefs-2026-09-16.md:48-52` says
drainage from saturated soil. `crates/cubarium-voxel/src/material.rs:31-48` and
`crates/cubarium-voxel/src/water.rs:15-23` instead drain porous materials above field capacity, including
roof drips. That is useful, but gives ecology a different retained-water floor.
The spring contract uses outlet elevation, without a submerged receiving head.

**Proposed change:** synchronize the brief with field-capacity drainage; name the
material controlling each interface's rate and cap by donor water and receiver
room. Apply per-second rates over DT, or DT/substeps when subdivided. Define
`h_spring = y * voxel_m` and spring volume as at most `Q*DT`, aquifer stock and
destination room; blocked/full cells retain the water in the aquifer. Distinguish
spring injection from the separate named export cell. For this wave, explicitly
declare one-way discharge without submerged backpressure as a limitation, or use
the connected receiving pool's head. Check a full receiver and zero head excess;
do not describe a room-limited source as general groundwater equilibrium.

## 7. P2 — Refusal needs a valid-world boundary as well as a schema tag — before code lands

`crates/cubarium-voxel/src/config.rs:58-79` divides/modulos by dimensions and only
debug-checks y/z. The index order and negative-x wrap are otherwise consistent
with the brief. `crates/cubarium-voxel/src/snapshot.rs:1-14` specifies tag refusal, while
`crates/cubarium-voxel/src/world.rs:194-200` exposes load without a stated validity contract.

**Proposed change:** require nonzero dimensions, checked cell-count arithmetic,
positive finite voxel size/porosity, valid rates and substeps at construction and
load. Loaded arrays must match `cells()`, fractions/stores must be finite and
within capacity, and spring/outlet coordinates must be valid. Reject malformed
or wrong-tag bytes with an error before replacing the runner's world. Validate
external y/z before calling the documented in-range indexing API. Add a tiny
zero-width/mismatched-array refusal check alongside the already planned tag test;
keep disposable snapshots and no migration. No compatibility suite is needed.

## 8. P2 — Test face ownership, not equality of adjacent seam pixels — can wait for round 2

`design/handoffs/voxel-first-wave-briefs-2026-09-16.md:79-94,105` provides a useful
projection and z-descending order, but not same-z face ordering. With this
projection, a farther voxel that overlaps a nearer one in screen space remains
behind it; tilt alone does not invalidate descending z for voxel slabs.

**Proposed change:** specify bottom-to-top y ordering within each z slab, exposed
face culling and deterministic shared-edge coverage. Draw free-water tops at
`y + free`, only at a water/air boundary; full cells beneath more water are not
extra translucent surfaces. Interleave water with terrain so a near wall/roof
occludes it. Test one such overlap and one stacked-water column. Wrap logical
sampling, draw translated images only where their bounds intersect the raster,
and blend each physical face once per pixel. Compare a cropped periodic tiling
with the wrapped render, including translucent water: first and last pixel
columns are adjacent samples, not necessarily equal. Define `base` from the
bottom of the raster and clip negative screen rows; a taller/deeper config must
preserve bottom registration rather than rescale when cropping from the top.

## 9. P2 — Ecology needs a supporting face, not only the highest solid — before code lands

`crates/cubarium-voxel/src/world.rs:52-54` correctly defines `surface_y` as the
highest solid. `design/voxel-ecology-sketch-2026-09-16.md:52-59,74` then assigns
surface-bound life, water depth and rooting depth to columns. Under the requested
overhang, that selects the roof instead of the sheltered floor; summing all free
water in a column can count a separate rooftop pool as drowning the plant below.

**Proposed change:** retain `surface_y` as a skyline helper, and document ecology
locations as a supporting voxel plus exposed face (top face suffices for the
first producers). Derive water depth from the contiguous wet void interval above
that support and roots from contiguous soil below it, using `voxel_m` for metres.
Use material capacity times pore fraction times voxel volume for available water.
One roof/floor column with separate pools should demonstrate the distinction in
round 2; no new cached field arrays are required now.

## 10. P3 — Reduce Package D's immediate fields and fix sky terminology — before code lands

The first experiment is two producers (`design/voxel-ecology-sketch-2026-09-16.md:90-98`).
It needs supporting geometry/material, root-accessible pore water, local standing
water and geometric light with plant-owned canopy attenuation. Litter/nutrients
belong to ecology. Animal reachability, line of sight, concealment, detailed slope
costs and explicit glowcap substrate attachment can wait for those consumers.

**Proposed change:** mark that split in the field table at lines 47-60; do not make
the whole list a first-wave core deliverable. The canopy ownership at lines 62-68
is right. Define geometric visibility as 1=open, 0=blocked: `sky_openness` currently
says fraction *blocked*. Keep vertical rain exposure separate from hemispheric
light visibility; a roof can block vertical rain while admitting lateral light.
A vertical-only light check would make sheltered shade habitat completely dark,
so name that limitation if used initially. Before coupling water consumption,
add a bounded core pore-withdrawal operation and an explicit transpiration loss;
`crates/cubarium-voxel/src/world.rs:7-19` has no such command. Two readers of moisture alone do not compete
for it. This is a next-experiment interface need, not a demand to implement plants now.

## What this does and does not establish

The boundary is small enough for the stated toy. These are contract ambiguities
and minimal fixtures, not runtime failures or a reason to reopen the owner's voxel, ringworld, fixed-grid,
depth, desktop, pixel-art or frontend decisions. The current additions narrow
several concerns but do not prove conservation, communicating-vessel behavior,
seam invariance, visual readability or a coupled ecology. Round 2 should inspect
the landed implementation against these small cases, not launch a study campaign.

## Round 2

Reviewed landed HEAD `7561316`, including repairs since `268cae8`. Checks passed: `cargo nextest run -p cubarium-voxel` (29 tests,
0.026 s execution) and `cargo nextest run -p cubarium voxel` (21 tests, 0.003 s;
579 unrelated tests skipped; 5.04 s compilation). No workspace suite.
New counterexamples use source-derived arithmetic checked in memory with Python; they are not new executed Rust tests.

### Round 1 disposition

1. **Specified spill case resolved; general order claim remains open as R2.1.**
   `crates/cubarium-voxel/tests/core.rs:69-76` passes the 1.0/0.4 target within 0.02.
   The single rule can produce this result; a separate spill mechanism was not
   necessary for that fixture. The test does not explicitly assert a dry sill.
2. **Fixture construction resolved.** `crates/cubarium-voxel/tests/core.rs:18-29,50-87`
   uses stacked injection and two sills on four columns, avoiding the seam bypass.
   Its 3.2-volume result is correctly 1.3/0.3/1.3/0.3 for that different geometry.
   Per-cell seam/mirror checks at lines 124-178 improve coverage, not a general proof.
3. **Resolved.** `crates/cubarium-voxel/src/ledger.rs:23-32` uses raw storage error;
   `crates/cubarium-voxel/src/water.rs:469-486` applies one relaxation factor, with no rounding account.
4. **Resolved for the stated edit contract.** `crates/cubarium-voxel/src/water.rs:734-801`
   preserves volume and uses void-only BFS/equal shell shares; the edit tests pass.
5. **Resolved.** `crates/cubarium-voxel/src/water.rs:628-684` implements immediate signed
   receipts/refusals; `crates/cubarium-voxel/tests/core.rs:336-365` checks stores and ledger together.
6. **Resolved at the declared approximation.** `crates/cubarium-voxel/src/water.rs:517-623`
   uses the named interface rates/caps and separate export; tests at
   `crates/cubarium-voxel/tests/core.rs:286-331` exercise sealed/full and low-head springs.
   A full spring may discharge into air above it; the no-flow case is sealed/full.
7. **Partly resolved.** Config, array shape, numeric range and tag validation landed
   (`crates/cubarium-voxel/src/config.rs:67-94`, `crates/cubarium-voxel/src/snapshot.rs:22-29`); see R2.3.
8. **Partly resolved.** Slab/y ordering, opaque ownership, stacked-water tops and bottom
   crop registration are present. Partial fills still fail at the chosen camera: R2.2.
9. **Resolved as a boundary clarification, not an implemented ecology API.**
   `design/voxel-ecology-sketch-2026-09-16.md:73-79` distinguishes support from skyline.
10. **Resolved as scoped planning.** The same sketch at lines 62-85 separates immediate
    producer needs, later queries, canopy ownership and future paid pore withdrawal.

### R2.1 — P1: greedy region growth chooses a spill branch

`crates/cubarium-voxel/src/water.rs:415-435,483-487,507-513` commits candidates
one at a time and excludes completed regions. Equal shares inside a region do not make its selection symmetric.
Concrete fixture: `World::empty`, width 6, height 5, depth 1, 1 m voxels, no fluxes;
use Bedrock for walls x=0,5 at y=1..4 and supports (2,1,0),(3,1,0). Add 0.7 m³
at each of (2,2,0),(3,2,0). This whole state is symmetric under x -> 5-x.
On the first equalization, both wet seeds join. The x=1 lower hollow is reached
first and admitted; admitting x=4's lower hollow next would put the level below
the crest, so it is rejected. The result is 1.0 left, 0 right, and 0.1 in each
of the four y=2 cells. At 80 substeps the arithmetic trace is left 1.0, right
0.399999999946: a persistent 0.6 difference, not float stopping noise.
**Smallest fix:** add this symmetric spill fixture; resolve competing equal-crest
exits together before committing their volume, sharing the available spill and
then rebuilding regions. Keep the single-sill/U-tube checks and add an explicit
dry-sill assertion to the 1.4 case; no pressure solver is implied. The existing
mirror tolerance 1e-6 is acceptable for its own fixture, but the universal claim
at `crates/cubarium-voxel/tests/core.rs:168-171` and `crates/cubarium-voxel/src/water.rs:80-86` must go:
neither a small mismatch nor this one passing fixture establishes order independence.

### R2.2 — P2: partial-water tops overlap or are wholly culled when only partly hidden

At `crates/cubarium/src/voxel/present.rs:470-499`, `top_hidden` is one boolean,
although overlapping projected surfaces can own only part of each other's rows.
At s=4, rise=2, let R be `front_row(y,1)`, with half-full water at (x,y,1)
and three-quarter-full water at (x,y,0), air above both. Their top bands are
[R,R+2) and [R+1,R+3). Neither is culled; row R+1 gets two translucent blends.
A second case: quarter-full water at (x,y,1), with a lone rock roof at
(x,y+1,0). Its top occupies [R+1,R+3); the roof covers only through R+2
exclusive, yet line 480 culls both rows. Visible row R+2 is lost.
**Smallest fix:** clip water tops by projected row intervals, rather than dropping
or keeping the whole face; add these two one-frame fixtures with known blend
counts/visible rows. Also treat a partial cell as having an air gap above its
water even if the next voxel is solid or wet (`crates/cubarium/src/voxel/present.rs:465-468`).
The stated steep-tilt limitation could wait outside the chosen camera, but the
claim that roof culling is exact at s=4/rise=2 does not hold for partial fills.

### R2.3 — P2: loaded fractions are not checked against their materials

`crates/cubarium-voxel/src/world.rs:246-252` accepts free=1 in Bedrock and pore=1
in Air: both are in [0,1], but neither has that storage capacity. `VoxelView`
then exposes water fractions the store accounting ignores, violating its contract.
**Smallest fix:** after the range checks, require free=0 in solids and pore=0
where `pore_capacity()==0`; add one malformed snapshot check for each. Keep the
existing refusal path, disposable snapshots and no migration.

### R2.4 — P2: the local tool still cannot edit terrain

`crates/cubarium/src/voxel/mod.rs:298-402` handles inspect, rain, pause, step,
speed, outlet, save/load and quit, but has no route to `SetMaterial`.
**Smallest fix:** expose a stdin command such as `m X Y Z air|rock|soil|bedrock`,
validate coordinates/material, call the existing command, and include it in help.
One paused edit followed by inspect is enough to check the missing interaction.

### R2.5 — P3: the brief reverses the camera restriction

`design/handoffs/voxel-first-wave-briefs-2026-09-16.md:48-50` caps *climbing*
toward the back at one voxel per two depth cells. The implementation correctly
allows climbing and limits drops. **Fix that sentence** to the inequality below.
Also change the sketch's `water_depth` table entry at line 56 to use its stated
support face, and the brief's lines 119-120 to say periodic adjacency/translation,
not equality of the two seam columns. These are doc fixes, not new owner choices.

### Water safety, camera and stopping point

On valid stores, fall is bottom-up and moves at most one cell per substep
(`crates/cubarium-voxel/src/water.rs:330-343`); transfer caps donor/room at lines
198-218. Face adjacency and solid exclusion at lines 507-513, plus the candidate
level check at 421-425, prevent joining through solids or a dry above-level path.
No negative/over-room transfer was found there; R2.1 concerns branch selection.
For the chosen projection, visibility requires `4*(y1-y2) < 2*dz`, hence integer
`y1-y2 <= (dz-1)/2`. `crates/cubarium-voxel/src/generate.rs:243-258,341-359`
implements and checks that inequality for every depth pair at three seeds.
With adjacent pairs included it enforces nondecreasing heights into depth.
The supplied 512×240 `astra-r2-generated.png` visibly has a ridge and cyan wet
hollow at the selected camera. I inspected that frame, not a new capture or live
web session. Passing fixtures establish collection/spill and the controller's
pause/step/speed/save/load functions; they do not establish interactive usability
or that the pictured generated world was observed overflowing. Terrain editing
is definitely missing, so the stated stopping point is not fully reached.

**Verdict: changes requested — R2.1 spill-branch symmetry; R2.2 partial-water ownership; R2.3 material-aware load validation; R2.4 terrain-edit command; R2.5 brief/table corrections.**

## Round 3

Reviewed landed HEAD `06b4b6f`, including `7c53dcd`, `2b02e02`, `4216e17`,
`4b92893` and `df884be`. Ran the permitted checks: 32/32 core tests passed
(0.023 s), and 24/24 voxel host tests passed (0.004 s; 579 unrelated tests
skipped). No workspace suite, new captures, production edits or commit.

### Round 2 disposition

1. **R2.1 resolved for the demonstrated defect.**
   `crates/cubarium-voxel/src/water.rs:434-478` batches candidates by y and judges
   each batch together. The symmetric fixture now checks every tick, final
   0.7/0.7 hollows and a dry shelf (`crates/cubarium-voxel/tests/core.rs:102-147`);
   the 1.4 case explicitly checks dry sills at lines 75-79. These assertions use
   tolerances, not bitwise equality; they suffice to reject the old 0.6 bias.
   The causal distinction is correct: the old rule admitted the first lower
   cell, then refused the second because the first had consumed its capacity.
   My proposed explicit spill sharing is not required to repair that case.
2. **R2.2 resolved for the reported partial-fill cases.**
   `crates/cubarium/src/voxel/present.rs:468-488,531-542` clips top rows by their
   projected ownership. Both counterexample tests at lines 971-1071 pass.
   `water_open_up` at lines 590-592 consistently uses drawn fill: at 4 px/voxel,
   a fraction rounding to four rows has no drawable gap. Accept this pixel-art
   quantization; it must remain presentation-only, never the ecology's wet/dry
   or drowning test. A 0.98 cell is not physically full merely because it draws full.
3. **R2.3 resolved.** `crates/cubarium-voxel/src/world.rs:260-271` now rejects
   free water in solids and pore water without capacity. Both malformed-snapshot
   tests pass (`crates/cubarium-voxel/src/snapshot.rs:59-77`).
4. **R2.4 resolved.** `crates/cubarium/src/voxel/mod.rs:378-400,412-430` implements
   `m`, validates y/z and material, wraps x, and reports the edited state. The
   paused edit/inspect test at lines 573-605 passes; help lists the command.
5. **R2.5 resolved.** The brief at
   `design/handoffs/voxel-first-wave-briefs-2026-09-16.md:48-52,119-122` states
   the correct drop inequality and seam adjacency. The sketch's `water_depth`
   entry (`design/voxel-ecology-sketch-2026-09-16.md:56`) uses the support face.

### Row-rule assessment and remaining scope

The batching is a sound repair of the identified admission-order failure.
`crates/cubarium-voxel/src/water.rs:440-448,547-553` admits only unique,
face-adjacent void candidates; lines 456-477 test their combined volume against
all included elevations and retain a rejected batch. Accepted paths therefore
remain below the resulting level; I found no new solid crossing or above-level
mis-join. Redistribution at lines 489-526 still shares by height with one
relaxation factor. Fall supplies the descent when the lower batch cannot join.

This is not a proof of general order independence. Batches contain the currently
reachable frontier, row priority is explicit at lines 449-451, and reconsideration
is bounded to four rounds at line 434. Different-y exits are a sensible next
counterexample *if an actual drainage case warrants it*, not a new gate now.
No concrete failing fixture emerged in this read. Keep the qualified module claim
at lines 90-96; the leftover “bias ... by a tenth” comment in
`crates/cubarium-voxel/tests/core.rs:234-237` can simply be deleted in a routine
cleanup, since error magnitude alone cannot identify its cause.

### Stopping point

**Yes, for the stated local terrain-and-water toy.** The supplied 512×240
`astra-r3-generated.png` clearly shows the ridge and cyan wet hollow at the chosen
camera. Collection/spill fixtures pass, and the runner now exposes rain, inspect,
terrain edits, pause, step and speed controls, plus save/load. I inspected the
supplied frame and tested the components; I did not independently drive a live
web session or watch that particular generated scene overflow. This clears the
first wave's functionality, not a general fluid solver or coupled ecology.

**What the next slice should watch:** debit shared root water through one bounded
core operation and an explicit loss/store; query actual supporting faces and
unrounded water depth; combine geometric sky visibility with plant-owned canopy
attenuation; keep local establishment and propagules paid; turn any observed
multi-height drainage anomaly into one tiny fixture before expanding the solver.

**Verdict: clears.**
