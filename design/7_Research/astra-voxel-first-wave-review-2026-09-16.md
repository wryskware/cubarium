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

## Round 4

Reviewed HEAD `332d3fc` on 2026-09-17: the producer brief, round-3 brief,
model, three test files, harness, package I/J results, backlog and biosphere
sections, in that order; also the intervening commit messages and relevant
water boundary. This is a source-and-arithmetic review. I did not rerun the
long experiments, run cargo, change code or make a capture. Earlier clearance
above remains clearance of the terrain-and-water first wave.

The organic/mineral accounting is a sound repair of the old currency error.
`crates/cubarium-voxel-flora/src/step.rs:509-570,572-631,697-722,997-1023`
retains mineral through respiration, draws it for newly assimilated tissue,
and transfers it proportionally through shedding, dieback, death, seeds and
decomposition. Reserve-to-foliage reflush correctly avoids a second mineral
charge. A starving stand's higher mineral/organic ratio is concentration of
an existing stock, not fertilizer creation or better health. The package's
`1 + build` enrichment is likewise accounted for. I found no missing outflow
in those paths. The qualifications are about what these stocks regulate and
what the experiment establishes, not a discovered conservation leak.

### Findings, in priority order

1. **R4.1 — P1: fresh arrivals rejuvenate old seeds; the cohort cap does not fix that. Timing: before round 4 presets.**
   `crates/cubarium-voxel-flora/src/step.rs:824-840,881-894` merges age 1 with
   age 0 and keeps age 0, every tick. Thus `seed_max_age_s = 600` means time
   since the bank's last continuous delivery, not a seed's lifetime. Attrition
   still acts, but arbitrarily small fresh deliveries can retain the surviving
   portion of old material indefinitely. The test at
   `crates/cubarium-voxel-flora/tests/round3.rs:726-758` explicitly blesses this.
   **Change:** keep landing age/expiry independent of subsequent arrivals;
   merge only equal-age cohorts, or use fixed arrival-time bins. Retain the
   bounded stock representation, but make its age approximation explicit:
   oldest-age merging at `crates/cubarium-voxel-flora/src/step.rs:864-876` conserves amounts at the merge and
   can kill much younger material at the next expiry. Four fixed time bins
   would avoid continually sweeping almost every old deposit into one bucket.
   Add a short fixture with a two-tick lifetime, germination disabled and tiny
   continuing arrivals: old material must reach litter on schedule, with
   organic, mineral and energy accounted for. Replace the age-zero expectation.
   Landing without the predicate, banks under occupants and paid attrition
   themselves are right; keep them. J's 1,700-second accumulation is conditional
   on the rejuvenation rule, so re-establishment evidence must be qualified
   until this is corrected (`design/7_Research/voxel-round3-experiment-2026-09-16.md:253-256`).

2. **R4.2 — P1: the harness supplies more water than the stated exits can remove. Timing: now.**
   `crates/cubarium-voxel-flora/examples/two_producers.rs:10-13,131-139,768-777`
   sets rain to 0.0005 m/s over 192 m²: nominal input **0.096 m³/s**.
   `crates/cubarium-voxel/src/config.rs:62-74` leaves evaporation at zero and
   outlet capacity at **0.05 m³/s**; `crates/cubarium-voxel/src/water.rs:797-808` can export less when
   its cell is undersupplied. J's 0.302307 m³ transpired over 2,000 s is only
   0.000151 m³/s on average (`design/7_Research/voxel-round3-experiment-2026-09-16.md:375-377`).
   With rain accepted, storage therefore increases by at least about
   **0.04585 m³/s**, even with the outlet continuously supplied. Spring and
   water-table seepage are internal transfers, not additional losses. This
   explains an unbalanced *forcing budget*, without evidence of a core leak.
   **Change:** remove the harness's claim that its tap stays below export;
   print accepted rain, outlet, evaporation, transpiration and storage change
   over the same interval. An outlet-only ceiling on nominal rain is
   `0.05 / 192 = 0.0002604 m/s`, not a proposed tuned setting or a guarantee
   of stationary head. Separate this rising-water disturbance from the habitat
   baseline. For comparisons, hold terrain/initial water and mineral stocks,
   external forcing and observation phase fixed; establish a bounded water
   regime first. A small explicitly controlled water fixture is enough for
   mechanism tests. Do not hold plant-driven water changes fixed while claiming
   to test root competition, or repair this experiment by tuning plant tolerance.

3. **R4.3 — P2: a mineral inventory is now honest, but the income law still treats external mineral as fuel. Timing: clarify now; physiological change later, before nutrient-specific gameplay.**
   `crates/cubarium-voxel-flora/src/step.rs:490-504` gates *all* assimilation,
   including maintenance income, by the ground pool. Consequently a stand
   already rich in mineral fixes zero on a zero-mineral pool despite ample
   light and water; `crates/cubarium-voxel-flora/tests/round3.rs:426-476` pins this. This follows the brief,
   but is stronger than “mineral caps new tissue.” Moving only `mineral/n_tissue`
   cannot fix it: Michaelis–Menten and `f_max` independently zero the income.
   **Change:** document that retained stand mineral is an inventory, not yet a
   reusable internal nutrient reserve. When nutrient physiology matters, separate
   carbon fixation/maintenance from mineral-funded tissue construction and make
   the nutrient response read the intended usable stock. Keep the fraction rule;
   do not clamp enriched tissue back to `n_tissue` and discard the difference.
   Correct `design/backlog.md:42` now: at defaults the stock cap is `50*N`,
   whereas the per-tick rate cap is `0.0005*N`; the former cannot be the binding
   one for positive N. Also describe the lazy `initial_mineral` at
   `crates/cubarium-voxel-flora/src/step.rs:1015-1020` as provisioning previously unrepresented ground:
   colonization currently imports mineral, explicitly booked, so the inventory
   is not closed during first landings. A fixed initial per-site inventory is
   the appropriate baseline for a future fertility comparison.

4. **R4.4 — P2: bloomcrown's failure is a funded-package problem, not evidence that `propagule_rate` needs raising. Timing: before round 4 presets.**
   `crates/cubarium-voxel-flora/src/step.rs:955-963` already caps the donor at
   `rate * DT * recipient_count`: the rate is **per recipient**, then a scarce
   reserve is split across all recipients. At the advertised rate, an unlimited
   donor delivers `0.0002/1.2 = 0.0001667` organic units/s to each site. With
   attrition 0.001/s, a bank reaches 0.05 in about **357 s**, and holds about
   **0.0752** after 600 s. The defaults are not intrinsically too slow when
   funded. Conversely, a true 600-second lifetime needs sustained net delivery
   about **0.0001108/site/s** to reach 0.05; funding 24 such sites costs about
   **0.00319/s** before maintenance or vegetative growth. The stressed donors
   are not supplying that. Raising the already nonbinding rate creates no income.
   **Change:** distinguish requested, funded and landed reproductive flux in
   the diagnosis, and specify reproduction allocation separately from dispersal
   footprint. I recommend saving a paid viable parcel and delivering it to one
   recipient at a time, chosen within the same hop without screening for habitat;
   at these splits one minimum parcel is 0.05 net, costing 0.06 reserve. Preserve
   the actual donor budget, construction cost and mineral transfer. Do not grant
   every recipient a full parcel without paying for all of them. This is a
   structural change to fragment-and-pool reproduction, not hidden tuning.
   For scale, lowering `alive_min` from 0.02 to at most **0.00294** would make
   the observed 0.00735 bank large enough arithmetically; it would also change
   adult death and seedling size, and does not establish survival. I do not
   recommend that knob change. Reassess under R4.2's controlled conditions;
   zero descendants under a flooding treatment is not proof of universal sterility.

5. **R4.5 — P2: germination currently gives enum order ecological priority and spends arbitrarily large banks on one stand. Timing: before round 4 presets.**
   `crates/cubarium-voxel-flora/src/step.rs:738-783` always lets the first
   qualifying species win, even with a much smaller bank; the test at
   `crates/cubarium-voxel-flora/tests/round3.rs:646-723` demonstrates that intentional choice. Adding three
   species after bloomcrown would build its precedence into every shared gap.
   The same phase spends the entire winning bank: `wood = w_frac * pooled`
   has no `wood_max` bound. A large bank waiting under a living stand can thus
   produce an oversized “small” preset; growth's later demand cap does not
   shrink it (`crates/cubarium-voxel-flora/src/step.rs:501`).
   **Change:** use an explicit reproducible local lottery among qualifying banks,
   with declared weights, for example viable-package counts, rather than enum
   precedence. Consume one germination package and leave the paid remainder
   ageing; transfer the consumed cohorts' actual mineral. Add tiny contested-gap
   and oversized-bank cases: storage order must not select the winner, losing
   banks remain, one birth stays within its preset's valid starting stocks, and
   no surplus is deleted. Check split sums and starting wood/foliage/reserve
   against each preset's caps before admitting that preset.

6. **R4.6 — P2: the repaired stress shape is sound; the shared tolerance is a simplifying trait choice. Timing: before round 4 presets, document the trait contract; add knobs later only for a role that needs them.**
   `crates/cubarium-voxel-flora/src/step.rs:420-425,472-482` has the desired
   interior target and first-order approach at the current rates. Sharing adult
   stress onset with the germination ceiling is acceptable for this slice,
   but “can germinate” does not logically imply “adult pays no stress.” State
   that assumption as such (`crates/cubarium-voxel-flora/src/lib.rs:326-331`). Umbrellafrond's 1.0 explicitly
   makes it saturation-immune (`crates/cubarium-voxel-flora/src/lib.rs:450-460`); that is a workable wetland
   producer proxy, not the biosphere's distinct moist-but-aerated understory role.
   **Change:** keep 1.0 for this wetland role and label the distinction. Do not
   lower it slightly as a repair: for *every* tolerance below 1, full saturation
   targets stress 1 and eventually removes all assimilation again. If a later
   role should germinate differently from its adult tolerance, separate those
   traits; if it should pay a partial cost even at full saturation, give the
   response an explicit subunit maximum or a funded tolerance cost. Those are
   different hypotheses. Neither requires a new oxygen solver now.

7. **R4.7 — P2: the probe has neither a stationary resident nor a measured single-founder generation time; lineage tracking also misses replacement. Timing: correct claims and tracking now; rerun the coexistence study later after the substrate repairs.**
   `crates/cubarium-voxel-flora/examples/two_producers.rs:869-897,938-939`
   calls any positive descendant count success after an arbitrary fill/probe
   interval. J's resident is 8 or 3 stands across 3,072 columns, its habitat is
   still moving, and the measured 1,700 s crossing belongs to the multi-founder
   comparison, not a controlled single-founder measurement
   (`design/7_Research/voxel-round3-experiment-2026-09-16.md:328-370`). One
   newcomer drowns; the other distributes 0.17026 across 24 subthreshold banks.
   Neither observation establishes that another 200 s would make it invade.
   **Change:** print “recruitment observed/not observed within T,” not a verdict
   on coexistence. First require the same founder treatment to replace itself
   without the competitor under the same forcing. Keep one founder as the rare
   test; optionally compare a declared small adjacent cohort, e.g. four founders
   at wood 0.3, in matched competitor/no-competitor arms. A cohort that rescues
   pooled-bank funding diagnoses density dependence, not invasion from arbitrary
   rarity. Choose duration from the positive control's full reproduction time
   and verify offspring replacement beyond the initial reserve subsidy; do not
   blindly extend the existing 1,500 s. Use established resident dynamics and
   matched initial stocks/forcing, with several suitable introduction sites.
   Positive growth while rare in **both directions**, through complete
   generations while the resident persists, would be coexistence evidence;
   one birth or founder survival is insufficient.
   Separately, the watches at `crates/cubarium-voxel-flora/examples/two_producers.rs:248-268,900-920` miss a founder
   dying and its own species germinating on that site in the same tick, which
   `crates/cubarium-voxel-flora/src/step.rs:99-102,588-598,741-783` permits. Give stands stable birth IDs and
   compare identity, or expose equivalent birth/death events. Add one tiny
   same-species death-and-replacement fixture. Current fractions are not
   certified exact lineage counts by the site watch.

8. **R4.8 — P3: carry the soil and light boundaries into the presets explicitly; the present tests do not establish all their advertised niches. Timing: before round 4 presets for boundary fixtures; fuller light budgeting later.**
   `crates/cubarium-voxel-flora/src/step.rs:344-371,911-917` uses soil-only
   root boxes and highest-support dispersal. That fits stonecushion in soil
   pockets and velvetpad as a damp-soil analogue; neither needs free access
   to rock water or atmospheric humidity. It does not populate a lower ledge
   merely because that ledge is a valid support. `crates/cubarium-voxel-flora/src/step.rs:1039-1057` tests
   geometric sky for germination, whereas `crates/cubarium-voxel-flora/src/step.rs:203-223` gives adults
   plant-owned canopy attenuation. Call the exposed predicate *abiotic
   eligibility*, not realized recruitment habitat under a canopy.
   **Change:** give each new preset a small paid-recruitment/survival fixture
   at its intended soil/light boundary, with a failing neighbouring condition;
   make any need for lower-face dispersal or canopy-sensitive germination an
   explicit rule change, not a new numeric preset. Keep the low-soil roles
   within the existing scope if those extensions are unnecessary. The equal-top
   no-shade expectation at `crates/cubarium-voxel-flora/tests/model.rs:143-182` and the 9% single-crown
   attenuation noted in `design/backlog.md:41` are current model choices, not
   proof of a closed canopy or a finite intercepted-light budget. Revisit those
   before deriving consumer carrying capacity from total producer income.

### What this does and does not establish

The split ledger, proportional transfers and repaired stress target satisfy the
central accounting and response-shape corrections on this read; the existing
short fixtures exercise those mechanisms. Package J supplies evidence of paid
umbrellafrond births and deaths under its particular forcing, with the reported
small residuals. It does not establish a finite-age bank under continuous rain
of seeds, bloomcrown self-replacement, stationary habitat, exact lineage fractions
or mutual invasion. I checked the cited implementation and the budget arithmetic;
the long-run numbers remain the experiment author's measurements.

**Verdict: changes requested before clearing round 3 for the presets round.**
Keep the organic/mineral split and the target-based aeration repair. Fix seed
age rejuvenation (R4.1), correct the forcing and experiment claims (R4.2/R4.7),
and settle funded reproductive packages and gap arbitration before multiplying
the presets (R4.4/R4.5). R4.3's accounting qualifications, R4.6's explicit wetland
trait and R4.8's small boundary fixtures can accompany that work; none calls for
knob tuning, another long run as a gate, or a general ecological rewrite.

## Round 5

Reviewed package K at `e1e1921` against the round-3b brief and R4.1–R4.8.
This is a source/test review with isolated binary64 arithmetic checks; I ran no
cargo command or world experiment. The earlier rounds remain history.

The requested repairs, finding by finding:

- **R4.1: resolved.** `crates/cubarium-voxel-flora/src/step.rs:1007` rounds bin
  width upward; `:1037` joins only the arrival's own bin, without changing its
  start. The continuous/pulsed fixtures at
  `crates/cubarium-voxel-flora/tests/round3.rs:1142,1226` test the former
  rejuvenation failure. The count bound is cap+1, not cap. K7 gives an expiring
  bin its last opportunity before removal; pin the exact boundary as R5.5 below.
- **R4.2: forcing and budget repair resolved.**
  `crates/cubarium-voxel-flora/examples/two_producers.rs:44,88` states the
  experimental tap and prints actual flows/storage. The reported near-2.50 m
  head supports bounded head in those runs; it does not make the warm-up habitat
  a stationary-habitat measurement or prove that every store has equilibrated.
- **R4.4: structural repair resolved.** At
  `crates/cubarium-voxel-flora/src/step.rs:1098`, a donor requests one gross
  rate, funds it above its reserve floor, respiring construction immediately
  and retaining the net parcel. At `:1125`, delivery takes the package's
  fraction of the donor's **current material including parcel** and its mineral.
  Mineral stays through respiration; no extra construction factor multiplies
  the recipient's density. Senescence/dieback use that same denominator
  (`:631,638`); death includes the parcel (`:680`). Clear and pruning book it
  out. The tests at `crates/cubarium-voxel-flora/tests/round3.rs:321,544,590`
  cover delivery density, unfunded requests and death with a saved parcel.
  Keep this rule; bloomcrown's remaining funding/habitat limitation is not the
  former split-over-24 defect. `funded − landed` includes lost donors' parcels,
  not just parcels still standing, when deaths or removals occur.
- **R4.5: lottery and package rule resolved, numeric birth invariant incomplete.**
  `crates/cubarium-voxel-flora/src/step.rs:828,906,912` uses whole-package
  weights, sorts candidates, and keys the draw by domain/world/site/tick.
  Dispersal has a separate domain (`:1180`); neither draw consumes a shared
  iteration-dependent stream. The swapped-candidate test at `:1307` exercises
  that contract. Oldest-first consumption preserves each bin's mineral
  fraction and losing banks; R5.1 is a remaining edge in the newborn it builds.
- **R4.7: identity repair resolved; observation wording still incomplete.**
  `crates/cubarium-voxel-flora/examples/two_producers.rs:330` counts surviving
  descendants by ID, including same-site replacements. The fixture at
  `crates/cubarium-voxel-flora/tests/round3.rs:1314` covers the blind spot.
  Withdrawing the invasion verdict was right. The probe still confuses no
  surviving descendants with no observed recruitment; see R5.4.

1. **R5.1 — P2: a funded birth can round below the death threshold. Timing: now, before carrying germination into the presets.**
   `crates/cubarium-voxel-flora/src/step.rs:943,952` accumulates the organic
   debit independently of the diminishing remainder. With three oldest-first
   bins holding `0.001, 0.009, 0.04`, the bank holds `0.05`, enough for the
   default package `0.02 / 0.4 = 0.049999999999999996`. These operations return
   `0.04999999999999999`; `:861` builds wood `0.019999999999999997`, below
   `alive_min`. With assimilation, maintenance and senescence frozen at zero,
   the next growth pass kills that paid newborn at `:646` without any loss of
   wood. I reproduced the ordered arithmetic separately; I did not run a Rust
   integration fixture. The direct division/multiplication test at `:1280`
   misses spending across bins, and the `1e-15` comparison in
   `crates/cubarium-voxel-flora/tests/round3.rs:1011` admits this failure.
   **Change:** construct funded newborn wood at `alive_min` exactly and take
   the rounding difference from another newborn compartment, preserving the
   actual paid organic total and transferred mineral. Pin this three-bin case
   through birth and one frozen growth tick, with the existing residual checks;
   also exercise each preset's split. This needs a floating-point allocation
   correction, not a larger biological package or a relaxed death threshold.

2. **R5.2 — P2: 96 columns is a warm-up observation, not the settled world's niche size. Timing: correct the interpretation now; settle the niche contract before presets.**
   `crates/cubarium-voxel-flora/examples/two_producers.rs:208,224,376` computes
   `eligible` after 1,000 world ticks (50 s), before planting, and carries that
   unchanged into the final result. The head settles only around 900 s in
   `design/7_Research/voxel-round3-experiment-2026-09-16.md:463`. Thus the
   comparison at `:554,566` measures different initial wetting under the two
   taps; it cannot identify a 96-column steady habitat, much less which gate
   causes its size. At `:515` the note even dismisses the predicate while
   reporting that none of the three remaining bloomcrown banks passes it.
   **Change:** label habitat counts with their measurement time; on the next
   authorised experiment, sample eligibility at introduction and observation,
   alongside the water budget. Diagnose independent failure combinations for
   empty soil root box, mean pore, saturated fraction, water depth and geometric
   light, plus the eligible recipients in each donor's hop. Use the model's
   gates at `crates/cubarium-voxel-flora/src/step.rs:1198,1221`, not a second
   approximate predicate. This is a short diagnostic over an available state,
   not a request for another long comparison now.
   Bloomcrown on open, aerated soil at pore fraction 0.25 already passes its
   0.1 moisture/0.6 light thresholds; the rule is not intrinsically incapable
   of a sunny-soil niche. Whether this generated world supplies that condition
   persistently is unmeasured here. Keep the values. Before adding springturf,
   stonecushion and velvetpad, specify whether the intended roles require sunny
   moist soil, soil pockets near rock, or damp soil under canopy. If the intent
   is instead bare-rock water uptake or dry-ridge recruitment, that needs an
   explicit substrate/trait decision. A count of 96 does not choose it.

3. **R5.3 — P2: the pre-K7 results are neither lower bounds nor unchanged coupled budgets. Timing: now, documentation only.**
   `design/7_Research/voxel-round3-experiment-2026-09-16.md:618` correctly dates
   the experiment, but `:626` calls establishments, fractions and occupancy
   sets lower bounds, and `:627` says funding, water and residual numbers are
   unaffected. Earlier births change gap occupation, water withdrawal, shading,
   litter and subsequent funding. There is no monotonicity guarantee for those
   counts or fractions, and unchanged accounting rules do not imply unchanged
   numerical budgets or residuals. **Change:** retain all numbers as pre-K7
   observations; state that K7 repairs immediate one-package germination and
   that its coupled outcomes have not been measured. At the defaults the lost
   first-tick fraction was `0.001 × 0.05 = 0.00005`, or **0.005%**, not 0.1%.
   Also correct `:576`: identical initial state, seed, founders and ticks are
   reproducible. A noise reseed can change the donor's voxel-index key and the
   subsequent delivery times, so those arms mix terrain effects with changed
   deterministic draws (`crates/cubarium-voxel-flora/src/step.rs:1180`). Do not
   use their occupancy difference as independent replicated dispersal evidence.

4. **R5.4 — P2: count recruitment events, and design the control around replacement rather than first birth. Timing: later, before rerunning the probe; not a preset gate.**
   `crates/cubarium-voxel-flora/examples/two_producers.rs:1024,1035,1059`
   observes only the final living newcomers. A descendant born and dead during
   the window yields “recruitment NOT OBSERVED” despite a real birth.
   **Change:** count newly seen newcomer birth IDs after each tick (or expose
   per-species birth counters); print cumulative births, deaths and final
   surviving descendants separately. A tiny birth-then-death fixture is enough.
   The current global establishment delta includes the resident's births.

   For the future positive control and matched arms, use this concrete design:

   - One founder of the tested species at wood **0.3**, ordinary `Seed` foliage
     and reserve, zero parcel and no bank of that species. Keep this treatment
     identical with and without the competitor. Do not rescue a failed
     single-founder control with four founders and still call it the same test.
   - Condition the hydrology and resident first, checking interval storage and
     habitat as well as head. Predeclare three suitable, unoccupied introduction
     sites across the eligible band, each with eligible dispersal recipients;
     report their actual recipient counts. For each site and direction, branch
     the same conditioned state into resident-only, resident-plus-newcomer and
     newcomer-with-resident-excluded arms. In the exclusion arm remove resident
     stands **and seed banks**, book removals, and retain matched water, litter
     and soil mineral. Hold geometry, forcing, phase, species values and draw
     keys fixed within each pair. Provision the same per-site mineral inventory
     before this resource-competition study; lazy colonisation imports are not
     matched fertility (`design/backlog.md:42`).
   - Start the control with a declared **6,000 s simulated observation cap**,
     as a future study, not a run requested here. Measure first birth separately
     from descendants reaching donor size, funding packages and sustaining
     recruitment themselves. The growth cap at
     `crates/cubarium-voxel-flora/src/step.rs:558,602`, with `wood_rate = 0.001`
     (`crates/cubarium-voxel-flora/src/lib.rs:473`), means wood 0.02 needs at
     least **2,708.15 s** to reach 0.3, then about 300 s to fund a package even
     under ideal income. Neither 300 s nor the pre-K7 1,300 s first birth is a
     generation time. If replacement is not observed within the cap, report an
     inconclusive control; do not infer exclusion or automatically extend it.
   - Once controls establish full replacement time `G` in both directions,
     predeclare a matched probe window of at least **3 × the larger G**.
     Record newborn IDs and donor funding/delivery events so repeated founder
     donations cannot masquerade as descendant reproduction. Positive increase
     while rare through replacement, beyond the founder reserve subsidy, in
     both directions while the resident persists would be coexistence evidence
     for these conditions. One birth, survival of the imported founder, or a
     resident that is itself declining does not establish it. A single successful
     site is a possible refuge, not evidence that every introduction can invade.

5. **R5.5 — P3: pin the expiry tick's last chance explicitly. Timing: with the small germination repair.**
   `crates/cubarium-voxel-flora/src/step.rs:876,976` implements removal on the
   first tick with age **greater than** the lifetime, after that tick's lottery.
   That is a coherent K7 rule: the bin can recruit once on its removal tick,
   and otherwise goes to litter; it cannot remain indefinitely. Current tests
   pin immediate recruitment and expiry under a permanently failing predicate,
   but not their intersection. **Change:** a two-tick lifetime, zero attrition,
   a paid package in a bin starting at tick 0, and a gate that first opens at
   tick 3 should recruit then. Its still-blocked twin should fall wholly to
   litter on tick 3 and never recruit on tick 4. Check organic/mineral transfers
   and document this boundary rather than changing `>` without a rule decision.

6. **R5.6 — P3: give preset authors the implemented contract. Timing: before presets land.**
   `crates/cubarium-voxel-flora/src/lib.rs:329` still calls `propagule_rate`
   per neighbour and says every face receives. **Change:** describe the donor's
   gross saving rate, construction cost and one drawn recipient per funded
   package; keep the corrected account in `design/backlog.md:41`. Apply the
   paid-recruitment/survival and failing-neighbour fixtures already required by
   `design/handoffs/voxel-round3b-briefs-2026-09-17.md:114` to each new preset,
   including its actual split and `alive_min <= wood_max`. Soil-only roots,
   highest-face dispersal and geometric-only germination light remain explicit
   boundaries, not evidence of bare-rock feeding or canopy-sensitive germination.
   R4.3/R4.6's mineral-inventory and wetland-proxy qualifications are now stated
   correctly in `design/backlog.md:42,44`; keep them, with fuller nutrient/light
   physiology deferred until an experiment actually needs it.

### What this does and does not establish

K resolves rejuvenation, fragmented delivery, enum-order arbitration and founder
misidentification. Its parcel accounting follows the organic/mineral fraction
rule, and K7 makes a fresh whole package available before decay. The supplied
long-run evidence belongs to the earlier phase order; this review establishes
neither post-K7 outcomes nor bloomcrown self-replacement or coexistence.

**Verdict: changes requested before clearing round 3 for presets:** fix the
funded-newborn threshold failure (R5.1), correct the habitat phase and post-K7
claims (R5.2/R5.3), and carry the small boundary/contract checks into that repair
and the presets (R5.5/R5.6). The structural package-K choices can stand. The
positive-control study and its observation fix (R5.4) are later work, not a
condition for adding presets; no tuning or long rerun is required for clearance.

## Round 6

Clearance read at `6cbe4af`, covering the five K8 commits after `7259ae9`, their
implementation and fixtures, and the updated experiment note. Source review and
isolated arithmetic only; no cargo command, world run or capture. Prior rounds
are preserved.

The Round 5 repairs now stand as follows:

- **R5.1: resolved for the landed presets and the reported three-bin failure.**
  `crates/cubarium-voxel-flora/src/step.rs:917` allocates wood first at
  `alive_min`, capped by the actual material available. The three-bin spend is
  still `0.04999999999999999`; the new stocks are wood `0.02`, foliage
  `0.019999999999999997`, reserve `0.009999999999999992`. They sum back to the
  actual spend exactly in this case, with mineral unchanged. I reproduced that
  arithmetic. For finite nonnegative material and valid preset values,
  `left = max(organic − wood, 0)` and `0 <= foliage <= left`, so
  `reserve = left − foliage` cannot be negative, including a zero-reserve split.
  The fixture at `:1524` now checks birth and one frozen growth tick, unchanged
  identity, remaining bank material and residuals. The allocation cases at
  `:1612` cover both existing presets and zero reserve. This is not a claim
  that arbitrary invalid configs are validated, or that an underfunded input
  is promoted to `alive_min`; the helper correctly declines to invent material.
- **R5.2: the measurement/API repair is resolved.**
  `crates/cubarium-voxel-flora/src/step.rs:1242,1258,1298,1312` routes cached
  germination and public queries through the same `gates` calculation and
  `Gates::passes`. The old predicate helper is gone; the harness wrapper at
  `crates/cubarium-voxel-flora/examples/two_producers.rs:582` delegates to
  `can_establish`. Eligibility is read at introduction (`:229`) and again at
  observation (`:340`), and diagnosis reads the public gates (`:432`). A few
  causal claims still exceed these measurements; those are R6.1/R6.3 below.
- **R5.3: the substantive correction is resolved.**
  `design/7_Research/voxel-round3-experiment-2026-09-16.md:583,693` withdraws
  independent-draw, lower-bound and unchanged-budget claims. It distinguishes
  deterministic replay from changed keys under a noise reseed. Remaining
  provenance/wording inconsistencies are documentation follow-ups, not a reason
  to repeat the experiment.
- **R5.5: resolved.** `crates/cubarium-voxel-flora/src/step.rs:805,1019`
  documents and implements the last lottery on the first tick past expiry.
  `crates/cubarium-voxel-flora/tests/round3.rs:1380` opens one twin's gate on
  tick 3 of a two-tick lifetime: it recruits, the blocked twin falls wholly to
  litter with mineral/energy, and opening that twin on tick 4 creates nothing.
- **R5.6: the rate contract is resolved in substance.**
  `crates/cubarium-voxel-flora/src/lib.rs:332` now describes one donor's gross
  saving rate, reserve floor, construction, whole package and one unscreened
  recipient. The per-preset boundary fixtures remain work for the presets
  themselves; they are not missing implementations in this two-species slice.
- **R5.4 observation fix: resolved.**
  `crates/cubarium-voxel-flora/examples/two_producers.rs:1178,1197,1223`
  counts newly seen newcomer IDs each tick and prints births, losses and
  survivors separately. Newborns remain observable at the end of their birth
  tick because death precedes germination. The birth-then-death fixture at
  `crates/cubarium-voxel-flora/tests/round3.rs:1304` exercises the missing case.
  The replacement-control study remains later work, not a clearance condition.

1. **R6.1 — P2: define umbrellafrond's wet-soil role; the diagnostic does not establish a transient-only species. Timing: in the presets brief.**
   `design/7_Research/voxel-round3-experiment-2026-09-16.md:619,633` gives
   2,404 mean-pore refusals and a skyline mean of 0.401. That mean averages
   different root boxes at one unsettled moment
   (`crates/cubarium-voxel-flora/examples/two_producers.rs:439,466`); it is not
   the equilibrium of drained soil. Soil's actual retained fraction is 0.25
   (`crates/cubarium-voxel/src/material.rs:44`), and ongoing supply can sustain
   wetter locations. A 0.45 recruitment floor can exclude ordinary drained
   ground while admitting persistently wet hollows. Neither the 668 qualifying
   columns nor the falling head tells us those hollows must disappear.
   **Change:** retain the stated wetland-producer role unless the brief explicitly
   chooses another: soil roots with sustained wetness, saturation tolerance 1,
   and the existing free-water depth limit. Give it a passing wet-root fixture
   (e.g. mean pore 0.6) and a failing drained-soil neighbour (0.25), and check
   adult moisture/income alongside recruitment. If the intended role is instead
   ordinary moist, aerated understory, state that change and choose the
   `establish_pore_min`, `wilt_pore` and `sat_pore` placeholders together
   (`crates/cubarium-voxel-flora/src/lib.rs:575`). Lowering 0.45 below 0.40
   alone is not justified by this spatial mean. The **role is a contract choice**;
   the numeric thresholds encoding it are preset choices the next round may
   set and label as placeholders. No hydrology or stress-equation rewrite is
   required merely to choose those thresholds.

2. **R6.2 — P2: define bloomcrown by resources, and give each new preset its own small boundary case. Timing: before the presets land.**
   `crates/cubarium-voxel-flora/src/lib.rs:540` describes a light-demanding
   producer needing moist, aerated soil. The observation at
   `design/7_Research/voxel-round3-experiment-2026-09-16.md:613` is compatible
   with that: 2,688 columns pass; the other 384 fail saturation, with 55 also
   failing water depth. It does not require bloomcrown to occupy only peaks,
   or only a small fraction of the map. **Change:** make sunny, aerated soil
   the brief's explicit baseline for bloomcrown, with no occupancy target or
   threshold adjustment to recover the former 96-column count. State
   springturf's soil/light role, stonecushion's access to soil pockets, and
   velvetpad's damp-soil role before assigning their values. Each needs one
   paid birth plus short survival/income check and a failing neighbouring
   condition, using its actual split and size limits. Carry finite nonnegative
   splits summing to one, a positive wood fraction and `alive_min <= wood_max`
   into those checks. Soil-only roots, highest-face dispersal and geometric
   germination light remain the boundaries at
   `crates/cubarium-voxel-flora/src/step.rs:396,1188,1305`; bare-rock water
   uptake, lower-ledge landing or canopy-sensitive germination would be explicit
   rule additions if a role needs them. Nothing here requires those additions.

3. **R6.3 — P3: finish the reporting corrections without another repair cycle. Timing: with the presets brief or before the next experiment.**
   **Change:** in `design/7_Research/voxel-round3-experiment-2026-09-16.md:627`,
   call this 400 **coupled** seconds after introduction: the harness first warms
   up for 50 s, then runs the requested duration
   (`crates/cubarium-voxel-flora/examples/two_producers.rs:212,271`), so the
   final world age is 450 s, not “350 s later.” At `:630`, zero pore/light
   refusals at observation does not identify which gate caused the initial 96;
   at `:640`, a later eligible-neighbour count cannot dismiss the predicate in
   the earlier 2,000 s arms, which reported refused bloomcrown banks. Limit both
   claims to their measured states. At `:687`, scope “pre-K7” to the three-arm
   comparison, excluding the newly added K8 diagnostic, and change the surviving
   0.1% at `:689` to 0.005%. Remove “settled water” from the unconditional
   observation label at `crates/cubarium-voxel-flora/examples/two_producers.rs:973`;
   call the size difference at `:982` a net count change, not a count of columns
   that moved. Finally, at `crates/cubarium-voxel-flora/src/lib.rs:342`, write
   construction as `gross − gross / (1 + build)` rather than “c_g of” the gross.
   The adjacent 0.06-cost/0.05-package example and implementation are correct.

### What this does and does not establish

The K8 source and focused fixtures answer the implementation failures from Round
5. The supplied diagnostic demonstrates changing eligibility and identifies the
gates at observation. It establishes neither settled habitat nor self-replacement
or coexistence. I checked the source and allocation arithmetic; the reported
world measurements and test executions remain the workers' observations.

**Verdict: clears round 3 for the presets round at `6cbe4af`.** No further model
repair or long experiment is required before springturf, stonecushion and
velvetpad. Put the niche contracts and each preset's small boundary fixture in
that round, carry the reporting corrections with it, and keep the later
replacement-control study separate. This clears the substrate for more presets;
it does not certify ecological balance.

## Round 7

Source review at `8dd0a57`, covering the presets commits from `2f1d680` and the
brief at `4b2fcf3`. I ran the eleven `round4` flora tests, the newborn-allocation
unit test across all five presets, and the focused five-palette/ground-crown
test: all passed. Arithmetic below is isolated arithmetic, not another world
experiment. The two 400-second community runs remain the worker's observations.

1. **R7.1 — P2: the harness's “under a crown” test can put the understory above its canopy. Timing: now, before reusing the placement helper.**
   `crates/cubarium-voxel-flora/examples/two_producers.rs:737,744,757`
   compares crown heights relative to their respective support faces, omitting
   both support heights. This is not conservative as claimed at `:725`.
   Concrete case: a half-grown bloomcrown at `(x,y,z)=(0,2,0)` has top 4 and
   radius 1; a half-grown velvetpad at `(1,4,0)` has top 4.75. The helper admits
   it (`2 > 0.75`, distance 1), but the model correctly applies no canopy shade
   (`crates/cubarium-voxel-flora/src/step.rs:261,282`). Conversely a short plant
   on sufficiently higher terrain can shade a pad despite failing this helper.
   `habitat` also discards living stands' actual wood at
   `crates/cubarium-voxel-flora/examples/two_producers.rs:1273`: an old or
   newborn resident is subsequently treated as a half-grown founder.
   **Change:** resolve planned founders' support heights in the current world;
   use actual site, wood and foliage for existing stands; compare absolute tops
   and wrapped horizontal coverage using the model's conditions. Add the
   two-height case and an undersized resident case. Exclude already-reserved
   columns before sampling founders at `:638`; the six-of-eight velvetpad result
   at `design/7_Research/voxel-round3-experiment-2026-09-16.md:777` is a
   placement collision, not a failed habitat trial. No change to canopy physics
   is needed for this repair.

2. **R7.2 — P2: accepting any species pair does not yet make the probe a valid pairwise treatment. Timing: before the replacement-control study.**
   `crates/cubarium-voxel-flora/examples/two_producers.rs:1326,1330` starts
   with no stands and asks `habitat` for the resident. For a velvetpad resident,
   `UnderACrown` tests an empty crown list and returns no sites. The probe then
   runs with zero residents and can report newcomer recruitment in that empty
   treatment. An isolated velvetpad positive control has the same placement
   problem, although its model predicate does not require a canopy.
   **Change:** separate the community's illustrative placement from the study's
   predeclared, gate-passing introduction sites; refuse to label an arm an
   invasion if its resident population was never planted or has disappeared.
   Velvetpad's control may use declared damp terrain shade, or a separately
   declared background canopy held constant across arms. It must not secretly
   require the competitor being excluded. Add an empty-layer velvetpad-resident
   selection check. Keep matched water, mineral, founder treatment and bank
   removal from R5.4; the existing `chesson` command is still a recruitment
   diagnostic, not that completed study.
   Also revise R5.4's **two-species** observation cap before including
   stonecushion: `crates/cubarium-voxel-flora/src/lib.rs:766,768,772,773`
   implies at least `ln(0.05/0.01) / ln(1 + 0.0002*0.05) * 0.05 ≈ 8,047 s`
   from newborn to donor, then **600 s** to save one 0.025 package when fully
   funded. Even the optimistic replacement path exceeds 8,647 s, so 6,000 s
   cannot demonstrate its completion. Predeclare a species-appropriate cap or
   report this control as unresolved; measure `G` before setting `3 × G` arms.
   This is a study-design correction, not authorization for a long run now.

3. **R7.3 — P2: springturf's recruitment floor is not a newborn maintenance boundary, and reduced light is not demonstrated canopy exclusion. Timing: document now; pin the boundary before using this role in a consumer comparison.**
   `crates/cubarium-voxel-flora/src/lib.rs:657,692,694,704,708` admits soil
   at pore 0.25. With the actual newborn split, `W=P=0.006`, initial site
   mineral 1, open sky and zero stress, `μ=1/3`. The income expression at
   `crates/cubarium-voxel-flora/src/step.rs:548,550` gives
   `0.008*(1/3)*0.006*(2/3) = 1.0667e-5` organic/s against maintenance
   `0.002*0.006 = 1.2e-5`. Reserve pays the deficit; no income remains to
   build the missing foliage. The ample-water fixture at
   `crates/cubarium-voxel-flora/tests/round4.rs:408` uses pore **0.6** and
   cannot establish solvency on ordinary retained-water soil. This does not
   invalidate germination into a temporarily unfavourable site, but it is a
   different claim from a viable drained-soil pioneer.
   **Change:** explicitly distinguish germination permission from positive
   newborn income, and add a short post-birth budget check at pore **0.26**
   (still passing, still maintenance-deficient) with ordinary preset stocks.
   If retained-water soil must sustain recruitment, make that a deliberate
   contract repair rather than raising assimilation until a community count
   looks right. The added 0.008 and velvetpad's 0.0005 senescence are legitimate
   named placeholders; their necessity has not been established by these tests.
   Similarly, `tests/round4.rs:511,515` proves only a small reduction in adult
   light, not that springturf loses under canopy. Accept worker point **(a)** as
   the existing geometric-germination boundary; correct the literal dense-crown
   refusal requirement in
   `design/handoffs/voxel-round4-presets-briefs-2026-09-17.md:101`. Label
   “wins the first years”/“loses under canopy” as intended succession, still
   untested. A canopy-sensitive germination rule is not required for clearance.

4. **R7.4 — P2: the proposed open-soil placement is sensible, but the stress diagnosis does not show that wettest-first caused the observed stress. Timing: correct the claim now; repair sampling before the next habitat comparison.**
   `crates/cubarium-voxel-flora/examples/two_producers.rs:627,635,686`
   filters on all gates, sorts by mean pore, then takes a **strided sample of
   the whole sorted pool**, not the eight wettest sites. Every qualifying
   springturf site initially has saturated fraction at most 0.3, hence target
   stress zero (`crates/cubarium-voxel-flora/src/step.rs:468`). Positive later
   stress requires subsequent root-zone conditions or retained stress from an
   intervening wet period; mean pore and saturated fraction are different
   measurements. The final mean 0.862 also cannot establish “a seventh of its
   potential income for the whole run” at
   `design/7_Research/voxel-round3-experiment-2026-09-16.md:859,863`.
   **Change:** accept Fable's proposed deterministic draw among unoccupied,
   gate-passing **soil support** sites, excluding actual overhead crowns for
   the open-soil treatment. The current `MoistSoil` branch never tests support
   material. Record selected founders' gate values and actual later saturated
   fraction/stress by identity on the next authorised diagnostic; do not tune
   the ceiling or promise the new draw removes stress. If no contract sites
   exist, report that rather than falling back to off-predicate founders at
   `examples/two_producers.rs:633`. This is my judgement on **(b)**.

5. **R7.5 — P2: a depleted bank at observation does not exonerate its germination gates. Timing: now, reporting only.**
   `design/7_Research/voxel-round3-experiment-2026-09-16.md:838,843`
   calls the bank the shut gate in every species and says the predicate is not
   refusing sites, while reporting that all three default-arm velvetpad banks
   fail it. Under K7 a whole newly landed package at a vacant passing site
   germinates before attrition. A later fractional package can instead reflect
   earlier occupancy or a refused predicate, followed by attrition. These are
   concurrent constraints; stonecushion has no landed bank at all.
   **Change:** report observation-time bank sufficiency, vacancy and predicate
   separately. Attribute an earlier refusal only to an observed event, not the
   remaining fraction. Keep the funded/requested diagnosis for stonecushion:
   **(c) is sound** — `400*0.00005/1.2 = 0.016667 < 0.025` per donor. Its
   lack of births is expected rate limitation, not a reason to accelerate it.
   Retain the R6.3 time/provenance corrections already applied, but remove the
   surviving identification of skyline mean 0.402 with drained-soil equilibrium
   at `:874`; the retained fraction remains 0.25.

6. **R7.6 — P3: correct the small numerical and API claims before they become future contracts. Timing: next documentation repair, before a consumer uses these quantities.**
   **Change:** at `crates/cubarium-voxel/src/world.rs:163`, describe
   `soil_below` only as contiguous soil in that column; delete “a plant rooted
   on rock has no root box.” `crates/cubarium-voxel-flora/src/step.rs:396,413`
   independently collects soil throughout the root box. Worker point **(e)**
   is correct; no core behaviour change or rock-support flag is warranted.
   At `crates/cubarium-voxel-flora/src/lib.rs:747`, stonecushion's package
   takes **600 s**, twice bloomcrown's saving time, not four times. At `:790`,
   umbrellafrond reaches full moisture at **0.8**, not 0.6; at `:796`, 0.1 is
   not twice springturf's 0.03. At `:798`, 20% incident light gives velvetpad
   **0.7333** effective light and bloomcrown **0.36**, not 0.83/0.49
   (`step.rs:291`; the test comment at `tests/round4.rs:690` is correct).
   State the conditions behind the break-even claim at `lib.rs:803` and
   `design/backlog.md:48`: full foliage, mineral 1, no stress and no growth
   require `L_eff*μ ≈ 0.2625` for velvetpad, or **0.4875** at base senescence,
   not 0.45; 4% sky works there only with sufficient moisture. Likewise the
   claim that springturf must double assimilation to survive at all (`:678`)
   is too strong: at full foliage/light/moisture and mineral 1 the base
   assimilation supplies `0.005333 W/s`, above maintenance plus foliage
   replacement `0.0044 W/s`. Extra income can fund faster growth; that is the
   honest placeholder rationale. Correct the duplicated “full moisture at
   0.1” prose at `tests/round4.rs:577`: its own assertion correctly checks
   about **0.2424**, not 1.

### What this does and does not establish

The three paid births and exclusion fixtures agree with the implemented
boundaries: springturf on ample open moist soil versus terrain shade/drier soil;
stonecushion on rock with reachable soil versus rock without it; velvetpad on
damp aerated soil versus saturation. Stonecushion is **rock-capable**, not
rock-exclusive; velvetpad is shade-tolerant, not canopy-obligate. Keep those
distinctions explicit rather than adding substrate or canopy requirements to
manufacture spatial segregation. The velvetpad income fixture also checks its
own positive material change (`tests/round4.rs:357,376`), so the presence of
other producers does not reduce it to an aggregate-ledger claim. It establishes
income in the fixture's mild shade, not a closed-canopy specialist advantage.

Worker point **(d)** is correct: newborn light/moisture have not been sampled
on the birth tick; zero there is not evidence of drought, and the next growth
window checks drinking. Five-way species indexing and validation cover the
brief's requested invariants (`src/lib.rs:65,857,1031,1241`); the presenter
test at `crates/cubarium/src/voxel/stand.rs:635` covers nonempty stemless
geometry and distinct palettes. I found no new transfer or conservation error
in the presets change. I did not assess a rendered community frame, settled
habitat, replacement or consumer carrying capacity. The weak canopy/light
budget and lazy mineral provisioning already recorded in `design/backlog.md:41,42`
remain limitations, not quantities this smoke run has calibrated.

**Verdict: changes requested for the harness and claims (R7.1, R7.3–R7.6), with
R7.2 required before the replacement-control study.** The five presets clear
the conserved producer substrate for a first consumer transfer prototype;
they do not yet clear the existing harness as the replacement study or establish
the claimed succession. Make the small placement, boundary-fixture and reporting
repairs; carry the matched-arm and species-specific timing requirements into
the study. A consumer round should start with explicit bounded food transfers
and the organic/mineral/energy ledger, leaving population targets and carrying
capacity unclaimed. No new hydrology, species tuning or long rerun is required
to answer this review.

## Round 8

Source review at `e7f4020`: package L (`e7d850c` through `c36e8d7`) and
package M's five commits. The eleven `round5a` tests, both example placement
tests and L's retained-water springturf test pass. I checked the transfer and
recovery arithmetic separately; no harvest rerun or long world run. The
reported 400-second outcomes remain the worker's observations.

Package L answers the Round 7 repair requests in substance:

- **R7.1 resolved.** `crates/cubarium-voxel-flora/examples/two_producers.rs:829,870,993`
  resolves planned support heights, reads existing stands' actual wood and
  foliage, compares absolute tops and wraps horizontal distance. The two
  fixtures at `:2397,2437` cover the reported failures. Reserved/occupied
  columns are removed before sampling at `:682`. Worker point **(g)** holds;
  OpenSoil asks about cover above the ground face, UnderACrown about cover
  above the candidate founder's crown, an explicit and reasonable distinction.
- **R7.3 resolved.** The fixture at
  `crates/cubarium-voxel-flora/tests/round4.rs:447` demonstrates a passing
  pore-0.26 site with income below newborn maintenance and falling reserve.
  The preset and brief distinguish permission, solvency and intended succession.
  Its fast-donor config still changes `reserve_cap` as well as saving rate;
  the comment at `:459` should acknowledge that, but neither changes this
  maintenance-deficient newborn's result: it earns no allocation surplus.
- **R7.4/R7.5 resolved.** OpenSoil's support/occupancy/canopy filter and keyed
  ordering are at `examples/two_producers.rs:765`; there is no off-predicate
  fallback at `:656`. Planting gates and identity-based observation are printed
  at `:728,933`. The experiment note at
  `design/7_Research/voxel-round3-experiment-2026-09-16.md:849,890,924`
  separates bank, vacancy and predicate, withdraws the stress attribution and
  distinguishes a skyline mean from retained-water equilibrium.
- **R7.6 resolved:** the numerical corrections are present in the preset docs
  and backlog; `crates/cubarium-voxel/src/world.rs:162` now describes only
  contiguous column soil. **R7.2 remains deferred, not implemented by L.**
  The old probe still selects velvetpad residents from an empty canopy at
  `examples/two_producers.rs:1571,1575`. Its separate study brief records the
  required treatment/refusal and timing repairs. This does not block transfers.

1. **R8.1 — P1: pruning a carrion site silently destroys all three currencies. Timing: now, before a consumer's deposits land.**
   `crates/cubarium-voxel-flora/src/step.rs:200,201,202` books litter,
   dead wood and soluble mineral when dropping an unsupported Ground, but
   omits `carrion`, `carrion_mineral` and `carrion_energy`. The new stock
   totals include them, so this is an actual residual, not a missing label.
   Concrete one-tick case: deposit `(organic, mineral, energy) =
   (0.4, 0.012, 0.9)` on a support face, remove that support, then step.
   The Ground disappears before decomposition; residuals become approximately
   **−0.4, −0.012, −0.9**. The same failure follows a deposit directly on an
   unsupported site, which `src/lib.rs:1683` explicitly promises to book out.
   **Change:** include all three carrion stocks in the corresponding
   `removed_*` additions. Add one deposit/remove-support/step fixture, checking
   the removal terms and residuals, plus the directly unsupported deposit case.
   `tests/round5a.rs:382` mentions this promise but never exercises pruning.
   No relocation, terrain rule or consumer behaviour change is needed.

2. **R8.2 — P2: accepted zero-organic deposits can strand mineral and energy indefinitely. Timing: settle before frondgrazer death/excretion uses the API.**
   `crates/cubarium-voxel-flora/src/lib.rs:1699,1704` accepts any finite
   nonnegative triplet with at least one positive component. Thus a carrion
   deposit `(0, 0.02, 0.4)` succeeds, but
   `src/step.rs:833,834` always returns with zero decomposition: its mineral
   never reaches the soluble pool and its energy never becomes heat. Litter
   with `(0, 0.02, 0)` strands the mineral too. Conservation totals still close;
   the defect is a public terminal-state contract that creates an inert sink
   until unrelated organic material happens to arrive. This matters for an
   exhausted consumer whose respiration left mineral behind.
   **Proposed change:** for an accepted deposit with organic exactly zero,
   credit mineral directly to the site's soluble pool and energy to heat,
   retaining the full `deposited_*` booking and existing provisioning rule.
   Document that terminal case and test it for both kinds. If the intended API
   instead requires positive organic for either kind, reject these triplets
   before provisioning and explicitly give the consumer another destination
   for its remaining mineral; do not accept them into an undecomposable pool.

3. **R8.3 — P2: the harvest note mistakes mediated treatment effects for coupling between arms and overstates non-recovery. Timing: now, reporting only.**
   `crates/cubarium-voxel-flora/examples/two_producers.rs:2050,2052,2209,2212`
   constructs separate World/Flora instances. At
   `design/7_Research/voxel-round3-experiment-2026-09-16.md:1031,1035,1199`,
   the arms therefore do **not** share a water table or shade field. Stands
   within each arm do. With identical starts and deterministic rules, a
   difference in an unbitten species can be a downstream effect of harvesting;
   its fourth-decimal magnitude is not a threshold below which attribution to
   the treatment becomes impossible. The direct bite effect and its ecological
   mediators have not been separated, nor has generality across seeds been tested.
   **Change:** replace the cross-arm coupling claim with within-arm feedback
   and distinguish total treatment response from a particular causal pathway.
   Keep the 15-versus-14 establishments as a measured contrast with mechanism
   unmeasured; delete “less income ... should ... recruit less” at `:1127`.
   Competition, water, shade, timing and keyed lotteries supply no monotonicity
   guarantee. This is the correction to worker points **(c)** and **(d)**.
   Also replace “nothing measurably recovered” at `:1105` with **limited partial
   regrowth, no stand reaching the declared full-foliage threshold**. The given
   cohorts gain **1.03%** and **7.99%** absolute foliage after harvest stops;
   the springturf-location deficit against control narrows from **0.77724 to
   0.74510**, while the bloomcrown-location deficit grows from **0.69295 to
   0.71720**. Neither closes its deficit, but these are measured changes.
   Do not attribute the control's 0.941 fill solely to per-tick senescence at
   `:1047`: base senescence removes 0.005% per tick, and the growth, resource
   and stress budgets jointly determine whether it is replaced. No rerun is
   needed to make these statements match the recorded observations.

4. **R8.4 — P3: an adult bloomcrown is beyond reach from its own face, not from every face. Timing: before adopting the reach explanation in the browser contract.**
   The implementation at `crates/cubarium-voxel-flora/src/lib.rs:1389`
   correctly compares **absolute** crown-cell height against the eater's
   ceiling. The unconditional “only its seedlings are ever food” at
   `examples/two_producers.rs:1918` and the general exclusion in
   `design/backlog.md:51` are stronger than that rule. A founder bloomcrown
   on support `y=2` has crown cells at 4: an adjacent eater on support `y=3`
   reaches it with `up=1`, although an eater on `y=2` cannot.
   **Change:** qualify the prose by relative elevation and add that two-face
   case beside `tests/round5a.rs:478`. Worker point **(b)** is valid as the
   measured result of these declared harvest faces, not a species-wide food
   exclusion. Keep the actual species of every bite in the report. Point
   **(f)** is fine: `FloraView` plus `VoxelView` supplies wrapping without
   tying flora to the presenter. The upper-only vertical bound, no line of
   sight and whole-stand foliage availability once one crown cell qualifies
   are explicit approximations, not a measured animal's reach or bite capacity.

5. **R8.5 — P3: state precisely what waits for the next tick. Timing: with the transfer documentation repair.**
   `crates/cubarium-voxel-flora/src/step.rs:151,164` correctly snapshots
   inter-tick deposits before growth and decomposes them on the next flora
   step; `tests/round5a.rs:412` pins that for carrion. But the last sentence
   of `src/lib.rs:1696` equates this with the current tick's senescence,
   which is too late for that snapshot. L's reporting repair also says a
   newly landed package enters the “same tick's lottery” at
   `design/7_Research/voxel-round3-experiment-2026-09-16.md:868`; propagation
   follows the lottery, so its first opportunity is the next tick.
   **Change:** correct both descriptions without changing phase order.
   Also qualify the blanket waiting claim in `src/step.rs:49`: the snapshot
   at `:812` stores **organic amounts**, while `:837,841` uses the mixed
   pool's **current** energy/mineral density. For example, one old unit with
   no mineral plus one newly shed unit with one mineral, at a half-old-stock
   decomposition step, releases 0.25 mineral immediately. Organic throughput
   is delayed; individual material parcels are not age-isolated. That is the
   inherited well-mixed-pool rule, and it conserves the currencies. Document
   it and pin a small mixed-density fixture before a consumer relies on a
   stronger all-currencies delay; M's inter-tick API needs no new phase.

### What this does and does not establish

`take_foliage` uses pre-withdrawal `Stand::material`, parcel included, for the
existing fraction rule (`src/lib.rs:1643`); it leaves wood and reserve alone
and returns the same triplet booked as consumed. `take_pool` at `:1756`
preserves dead-pool mineral/energy density and empties all three stocks on a
full withdrawal. The six ledger terms have the correct signs; carrion appears
in stock totals; litter deposits book the full incoming energy and send excess
to heat. These ordinary positive-organic transfer paths are sound on this read.
The omission in R8.1 is on removal, not on decomposition or withdrawal.

Worker point **(a)** holds only with “directly” and the observation window:
foliage removal does not itself kill a stand, but unpaid maintenance later
removes wood (`src/step.rs:653,662`) and can kill it. Zero deaths in these
400 seconds is not grazing immunity. The short regrowth fixture checks a
positive direction with reserve available, not complete recovery after the
reported prolonged harvest. **(e)** is an appropriate scope boundary:
`DepositKind` has the two kinds the brief requested; package N may add dead
wood with the same booking/removal checks. No coupled deposit measurement was
made, and unit tests are sufficient for this substrate once the missing cases
above are covered. `carrion_decomposition` remains **0.005/s**, unmeasured;
the report at `:1153` should say its *flux* was zero, not its configured rate.

**Verdict: changes requested — R8.1 and R8.2 before clearing the shared deposit
boundary for glowcap/frondgrazer integration; R8.3–R8.5 are reporting and
contract follow-ups at the stated timings.** The bounded foliage/dead-wood
withdrawals are suitable for the packages already in progress. They can
continue against actual `Taken` receipts; fix carrion removal and define the
zero-organic terminal case before calling the combined consumer ledger closed.
Then check one small consume/deposit/decompose exchange across both ledgers,
including litter-cap heat and separately booked lazy mineral provisioning.
No tuning, recovery mechanic or long harvest rerun is a clearance requirement.
