---
design_status: exploration
status: open
last_reviewed: 2026-09-22
decision_refs: []
---

# Voxel water algorithms: findings and next experiments

Wrysk requested a comparison with game implementations and the literature,
then this handoff. Research and code inspection were performed at `99b4c2d`,
including the water optimization in `bb47ff5`. No new benchmarks or solver
changes were made. The recommendations below are candidates, not accepted
algorithm decisions. Read [working policy](../../WORKING_POLICY.md).

## Finding

The next substantial opportunity is to **reduce how much water state gets
processed**. Our solver already skips dry space, uses cached terrain geometry,
and has optimized masks and memory access. It still revisits settled water and
processes the submerged voxels of a column individually.

Two useful directions emerge: separate wet membership from water that needs
an update, and compress water into vertical intervals connected by flow
interfaces. Sleeping is the smaller experiment; layered columns are the
preferred structural replacement candidate. Neither has measured savings here.
A third, a minimum spreading depth (C below), targets rain, which neither of
the other two does.

**Target (Wrysk, 2026-09-22).** The desktop world, `config/desktop/voxel.toml`
(256 × 128 × 48, 0.125 m, preset `default`), lags whenever it rains on Wrysk's
machine. It should run on a low-to-mid gaming PC or laptop taken to a festival
or exhibit, and water should leave room for larger worlds in a bigger ambient
display or a game. The panel is not the constraint: since PA/PB/PC it holds
20.0 ticks/s through showers. Measure the desktop world first.

## What the games and papers actually establish

- **Timberborn, documented 2023 implementation:** one water depth per terrain
  tile, four virtual pipes to neighbors, and retained flow momentum. Deep water
  therefore needs no separate simulation cell at every height. Updates were
  every couple hundred milliseconds, with continuous presentation. Mechanistry
  explicitly cites Mei, Decaudin and Hu's *Fast Hydraulic Erosion Simulation
  and Visualization on GPU* (2007) as its basis.
  [Developer technical account](https://www.gamedeveloper.com/design/deep-dive-timberborn-s-water-mechanics).
- **Timberborn since Update 6 (2024):** the developers replaced the single-sheet
  representation, enabling stacked waterways and water beneath structures.
  Their announcement establishes those capabilities, but does not disclose
  the replacement solver. This research did not verify a primary technical
  account of its current internals. Do not describe modern Timberborn as a
  single heightfield or assert that it uses the layered algorithm proposed here.
  [Official announcement](https://store.steampowered.com/news/posts/?appids=1062090&enddate=1719384498&feed=steam_community_announcements).
- **Dwarf Fortress, Tarn Adams' 2008 account:** local falling and spreading on
  tiles holding 0–7 units, plus a nonlocal pressure shortcut. Search through full
  water for available space, never above the originating level, then transfer
  directly there. Failed searches mark water static; changes clear those flags.
  Adams credits avoiding repeated searches with making waterfalls affordable.
  This is direct historical evidence, not verification of every current detail.
  [Interview with Adams](https://www.gamedeveloper.com/design/interview-the-making-of-dwarf-fortress).
- **Layered virtual pipes:** Kellomäki (2014), *Rigid Body Interaction for
  Large-Scale Real-Time Water Simulation*, extends pipes to water above and
  below blockers, with separate flows and hydrostatic pressure. This supports
  the representation direction, but the paper restricts blocker geometry; it
  is not an arbitrary-cave or sealed-passage solver ready to transplant.
  [Paper](https://onlinelibrary.wiley.com/doi/10.1155/2014/580154).
- **Higher-fidelity grid water:** Chentanez and Müller (2011), *Real-Time
  Eulerian Water Simulation Using a Restricted Tall Cell Grid*, compresses
  deep water into tall cells and spends ordinary 3D cells near the surface,
  with a multigrid pressure solve. Its restricted layout also needs adaptation
  for our terrain. The relevant lesson is reducing interior work, not adopting
  the entire GPU solver.
  [Paper](https://matthias-research.github.io/pages/publications/tallCells.pdf).
- **Particle fluids:** Macklin and Müller (2013), *Position Based Fluids*,
  provides richer 3D motion through particle neighborhoods and iterative density
  constraints. Those operations introduce substantial work of their own; this
  is not an evidenced performance shortcut for Cubarium.
  [Paper](https://matthias-research.github.io/pages/publications/pbf_sig_preprint.pdf).

These are documented examples and established method families, not a survey
proving one industry-wide standard or one universal state of the art. Virtual
pipes and full shallow-water-equation solvers are related but different: the
latter evolve depth and horizontal momentum with more complete transport.
A single heightfield cannot represent separate waterways above and below a
roof. Rendering a 3D-looking waterfall does not establish a 3D fluid solver.

## Current implementation and remaining cost

Entry points in [water.rs](../../crates/cubarium-voxel/src/water.rs), with line
numbers at the inspected revision:

- `step` (505–527): four configured substeps, each infiltration → fall → exchange.
- `fall` (727–758): ascending snapshot of the wet set; one-cell falling transfers.
- `exchange_inner_with_masks` (1075–1339): wet-set construction, column heads,
  pressure approximation, proposals, donor/receiver limits, then settlement.
- Head propagation (1150–1189): initialize driving head from column head, then
  up to four double-buffered maximum-head passes through full cells. Early exit
  is possible. Driving heads are rebuilt next substep, not retained as pressure.
- `scan_column_mask` (1452–1465): identifies wet runs, then writes the same head
  to each voxel of a run. Cached void runs describe terrain, not water volume.

The model conserves transferred volume through shared proposals and capacity
limits. It has **no momentum or persistent velocity**. Its pressure propagation
is a heuristic, not a Poisson pressure solve. The module documents uneven final
levels in a closed, surcharged passage (roughly 127–140).

The active set is all wet voxels, including settled ponds and tiny films.
Every substep reconstructs masks/heads, revisits full cells, and examines flow
offers. At the configured maximum, pressure propagation alone can examine
4 neighbors × 4 passes × 4 substeps per full voxel per tick. Early exit reduces
that count, but does not make settled water disappear from the other loops.

Flow also lacks a physical timestep in its exchange coefficient:
`FLOW_PER_SUBSTEP = 0.5`; `HEAD_PASSES = 4`. Reducing `water_substeps` changes
falling and relaxation speed. It is not a behavior-preserving accuracy knob.

Already present: sparse falling/infiltration/drainage, terrain-owned void-run
and sky-floor caching, u128 masks through height 128, ordered wet-set walks,
world-order scratch, and precomputed horizontal neighbors. Above height 128
exchange uses its dense fallback. The current exchange accepts but ignores
`threads`; historical parallel-column measurements do not describe this version.
The [September 18 fall](voxel-sparse-fall-2026-09-18.md) and
[geometry-cache](voxel-exchange-geometry-2026-09-18.md) briefs describe earlier
work, not the remaining optimization plan.

**Existing measurements:** `git show bb47ff5` records desktop release/headless
runs using `config/tachyon/voxel.toml`, CPU affinity 0–5, seed
14400042426867678818. Total step time fell **6.1 → 3.8 ms dry** and
**11.7 → 8.3 ms during a forced shower**. These were 60-second measurement
windows; wet counts remained about 31.0k and 68.3k respectively. They are whole
simulation-step timings, not water-only or board timings. The older board
exchange figure of about 14 ms/tick in the
[performance review](terrain-generation-briefs-2026-09-21.md#performance-review-before-the-sim-package--2026-09-22)
predates PA. Establish a fresh baseline before attaching a speedup to any proposal.

## Proposed next work

First, one bounded diagnostic pass on the current solver: count wet voxels,
full voxels, wet vertical runs, distinct wet columns, head passes, nonzero
accepted transfers and water touched by external exchanges. Separate gross
movement from net per-cell change: equal incoming/outgoing amounts do not mean
the water is inactive. Measure phase and total step time in a quiet window and
a forced shower on one fixed seed and the desktop configuration (see Target).
Cap each of these two diagnostic runs at two minutes wall time. Include set
construction and cache maintenance. Use those results to choose the next small
implementation experiment; do not assume every proposal needs a full package.

Also report, because each one sets a candidate's ceiling:

- **Depth histogram** of `free` over wet cells (decades from 1e-9 to 1 of a
  cell), quiet and shower. The share below each decade is C's ceiling. PA's
  rejected f32 heads stopped films thinner than about 4e-6 of a cell from
  spreading and cut shower wet cells 35 % (`git show bb47ff5`).
- **Net change by phase:** of the wet cells, how many a tick changes through
  fall/exchange, evaporation, infiltration and water table/spring. Only cells
  no phase touches are sleepable (A's ceiling). Evaporation acts on every
  sky-exposed surface every tick.
- **Wet voxels ÷ wet runs** (B1's ceiling) and full ÷ wet (the head passes'
  share).
- **Share of wet cells with no nonzero edge** in a substep, and the cells
  `fall` actually moves. A deep column exchanging sideways debits each
  submerged row (`water.rs:1324`), and the next `fall` repacks the whole run.

Measure through the live path, the leaf systems in
`cubarium-voxel-sim/src/lib.rs:469-524`, not `water::step`. Extend the existing
`profile` feature's counters (`profile.rs:147`) rather than building new
tooling. Lead with counts: they transfer to other machines and milliseconds do
not.

### A. Sleeping water: smaller experiment

Keep wet membership for storage and queries, with a separate dirty/awake set
for flow. Wake affected water after transfers, rain, evaporation, infiltration,
spring/aquifer exchange, outlets and terrain edits. Head changes can matter
through a whole saturated connection; immediate-neighbor invalidation alone
is insufficient. A sleeping chunk with flow across its boundary must wake.

Start with exact no-work cases. If a numerical sleep tolerance becomes
necessary, retain the water and accumulate unresolved changes; do not delete
films or silently stop small continuing fluxes. Check that distant openings
and slowly accumulating inputs wake a pond or flooded passage correctly.
Measure quiet and shower behavior separately: distributed forcing may keep
most of the world awake, limiting this candidate's benefit. Evaporation is
continuous, so a per-cell dirty set would wake every open lake each tick.
Letting an even lowering of a flat body keep it asleep needs body identity,
which is halfway to B. If A goes ahead, sleep by tile of columns rather than
by cell (Noita's per-chunk dirty rectangles, GDC 2019, are precedent).

### B. Layered columns: preferred structural candidate

Two steps. **B1** runs fall and exchange once per wet run, over the existing
per-voxel `free` storage. The per-column u128 wet/full masks are maintained
in the store primitives, as the wet set already is, and only a run's end cells
are written. Every other reader and writer of water is unchanged: rain,
evaporation, drain, water table, spring, outlet, commands, flora, fauna, pack
and snapshot. At run granularity a connected-region pressure solve (e.g.
union-find over sealed runs) may be affordable, which could fix the uneven
surcharged passage; that is a candidate, not a decision. **B2** makes
intervals the stored state, only if B1 falls short. The rest of this section
applies to both.

Falls stay visible: Wrysk accepted the terrace falls because they read at
4 px, so instantaneous settling is out unless he decides otherwise. For
explicit falling packets beside a bulk representation, Chentanez and Müller
(2010), *Real-Time Simulation of Large Bodies of Water with Small Scale
Details*, is precedent (cited from memory, not re-verified here).

Reuse static void-run geometry, but make dynamic water volumes and connections
the flow state. Keep separate intervals for water above and below roofs;
connect neighboring intervals only through their geometrically open interfaces.
Exchange a single bounded volume across each connection, debiting and crediting
the same amount. Work should tend toward the number of intervals/interfaces
rather than the number of submerged voxels.

A terrain void run is not automatically one settled water column: detached
falling water and separated wet runs may exist inside it. Retain an explicit
representation/rule for falls, or state and evaluate any instantaneous-settling
approximation. Reconstruct voxel occupancy/depth for ecological and rendering
queries and include that reconstruction cost in the measurement.

Specify full, roofed-passage pressure before claiming support for communicating
vessels. Layered storage alone does not resolve pressure; evaluate a cached
connectivity/head treatment or a reduced pressure solve. Persistent edge flow
is an option if momentum is useful, with rates expressed per second and an
explicit stability restriction. Preserve infiltration's opportunity before
runoff; changing that ordering previously changed soil moisture substantially.

### C. Minimum spreading depth — agreed by Wrysk, 2026-09-22

Any film now spreads into every empty neighbour on its row, because an empty
cell's head is its own floor (`water.rs:1228-1236`), so rain multiplies
near-empty wet cells. Water shallower than a threshold does not offer
sideways: it stays, and still infiltrates, evaporates and falls. Nothing is
deleted. This is the wet/dry threshold of shallow-water solvers. The
threshold's value is chosen from the diagnostic's depth histogram and stated
in the commit together with its visible effect (wet cells, pore water,
pooled water). PA's accidental version moved pore −5 % and pooled +3 %.

### Not now: GPU compute or the NPU

The Tachyon's Hexagon NPU runs quantised neural-network graphs, not an exact,
branching conservation stencil. Adreno generally lacks f64, and PA showed f32
heads change the world. Displacement up a run is a scattered write. Flora,
fauna and pack read water on the CPU every tick. If GPU water is ever wanted,
B is the GPU-shaped representation to port (a layered heightfield with pipes,
as in Mei et al. 2007), not the per-voxel solver.

## Check and return

Use small function fixtures: a still lake, a falling drop/spillway, stacked
waterways separated by a roof, a U-tube and a surcharged roofed passage, a
terrain edit opening/closing a connection, and thin rain over absorbent soil.
Check conservation, blocked paths, waking, settling, fall timing and soil
intake. Treat known weaknesses of the current solver as comparison evidence,
not results a replacement must reproduce. Keep routine tests within the
working policy's few-hundred-tick limit; any longer measurement is an explicit
bounded study, not a new routine test.

Run `cargo nextest run -p cubarium-voxel` if that crate changes, plus only other
crates actually touched. Report before/after phase and total time for quiet
and shower conditions, active-work counts, setup/rebuild costs, and observable
behavior changes. No promised speedup, pinned world hashes or retained build
copies. Keep results concise in the implementation commit. Choose the next
step from measured work reduction and useful water behavior.
