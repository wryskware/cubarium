---
design_status: exploration
status: open
last_reviewed: 2026-09-18
decision_refs: []
---

# Cache static geometry for water exchange

Wrysk requested this handoff on 2026-09-18 following the performance survey
(item 4). The cache design is a candidate to measure, not an ecological rule.
Animal sensing optimization is deferred while sensing is actively redesigned.
Read [working policy](../../WORKING_POLICY.md). Resume the water worker after
[sparse fall](voxel-sparse-fall-2026-09-18.md); establish a fresh baseline then,
so the two packages' gains are not counted twice.

## Evidence and entry points

The survey's seed-1 release bench (1,000 ticks, 50 s warm-up, 4 initial grazers,
8 workers) spent 486 µs/tick in exchange, about 27% of the 1.822 ms tick.
This is the **whole phase**, not a measured cacheable fraction. The previous
[schedule addendum](../7_Research/voxel-tick-profile-2026-09-18.md#addendum-same-day-the-schedule-and-what-threads-bought)
describes the column-major scratch optimization already landed.

Start at `crates/cubarium-voxel/src/water.rs::scan_column` (1190–1242),
`scan_columns` (1085–1147), `Scratch` (629–715), and `exchange_inner`.
`scan_column` rediscovers each maximal non-solid vertical run on every call.
`World::terrain_version` advances when `SetMaterial` changes a material.

## Work

- Cache static void-run bounds per column, including each run's ceiling.
  Rebuild when terrain changes; begin with simple whole-cache invalidation
  unless measuring it shows a need for per-column invalidation.
- Recompute water-dependent values every substep: wet-run heads,
  `room_target` (lowest cell with room at or above the target), driving heads,
  offers and acceptance. Drying, rain and transfers must change the next
  scan without waiting for a terrain edit. A void run and a wet run are
  different things; only the former is static.
- Cache ownership must distinguish worlds. The existing `SCRATCH` is
  thread-local and can serve multiple worlds with the same dimensions and
  `terrain_version`. Those two values alone are **not** a valid cache key.
  Prefer world-owned derived geometry or another explicit lifecycle that
  handles creation, clone, reset and snapshot load without stale reuse.
- Keep derived cache state disposable and avoid snapshot format changes.
  Preserve wrapped-x geometry, solid ceilings, head propagation, flux limits,
  conservation and phase order. Keep existing disjoint column-worker writes;
  build geometry before workers read it. No new parallel phase, global grid
  transpose, `f32` conversion or solver change in this package.

## Check and return

Use short fixtures for an open column, stacked cavities/roofed water, a terrain
edit opening or closing a passage, and wet/dry changes without terrain edits.
Exercise two different same-size/same-version worlds on one thread and reuse
after clone/load. Compare cached results with the uncached calculation on
small fixtures at existing tolerances; retain water conservation checks.
Run `cargo nextest run -p cubarium-voxel`, plus only any other crate touched.

Use the bounded before/after bench matrix in [sparse fall](voxel-sparse-fall-2026-09-18.md#check-and-return),
against the post-fall baseline. Report exchange µs/tick and total ms/tick,
cache size, and initial-build/terrain-edit rebuild cost separately. Keep
rebuild work in real end-to-end timings; do not hide it outside the tick.
If savings are negligible, return that result rather than expanding scope.

Return the commit and concise measurements. No new report file, deployment,
captures or retained benchmark binaries/data.
