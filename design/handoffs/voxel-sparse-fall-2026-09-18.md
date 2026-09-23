---
design_status: exploration
status: open
last_reviewed: 2026-09-18
decision_refs: []
---

# Sparse falling-water updates

Wrysk requested this handoff on 2026-09-18 following the performance survey
(item 3). The algorithm below is an implementation candidate, not new canon.
Animal sensing optimization is deferred while sensing is actively redesigned.
Read [working policy](../../WORKING_POLICY.md); use the existing water worker,
and finish this before [exchange geometry caching](voxel-exchange-geometry-2026-09-18.md)
because both edit `crates/cubarium-voxel/src/water.rs`.

## Evidence and entry points

The survey's release bench, seed 1, 1,000 measured ticks after 50 s warm-up,
4 initial grazers and 8 workers, took 1.822 ms/tick. `fall` took 502 µs/tick;
the sampled run counted about 282,000 visited positions per tick. These are
current implementation costs, not measured savings. Earlier context:
[tick profile, schedule addendum](../7_Research/voxel-tick-profile-2026-09-18.md#addendum-same-day-the-schedule-and-what-threads-bought).

Start at `water.rs::fall` (457–481), `wet_columns` (486–502), and the
`World::wet` / `CellSet` membership maintained by `add_free` and `take_free`.
Today each wet column is walked through its full height, four times per tick.

## Work

- Prototype a reusable snapshot of the wet cells at the start of **each fall
  call**, ordered bottom-up. Ascending world index is one simple candidate:
  it preserves bottom-up order within each independent column. Measure the
  sorting/copying cost as part of the phase; do not assume sparse always wins.
- Skip the bottom row and retain the existing material, room and bounded
  transfer checks. Keep all writes through the active-set-maintaining
  primitives; do not iterate a set while swap-removal changes its membership.
- Preserve one-cell-per-substep falling. A newly wetted cell below has already
  had its turn; it must not carry that arrival farther down in the same call.
  Existing wet cells still get their turn bottom-up, making room for the cell
  above. Do not replace this with instantaneous column compaction.
- Reuse scratch capacity. Keep the current phase order, substep count,
  conservation accounting and water rules. No threading or store-precision
  changes in this package. Update `FallCells` and its report label to count
  actual work after the change, not the old columns-times-height estimate.

## Check and return

Use tiny function fixtures for a falling droplet, a stacked wet column, a
partly full receiver, a solid floor and the bottom boundary. Check conservation
and wet-set membership after transfers; exercise empty and densely wet cases.
Run `cargo nextest run -p cubarium-voxel`, plus only any other crate touched.
No long-run tests or pinned world hashes.

Build `cargo build --release -p cubarium-voxel-sim --features profile --example bench`.
Before/after, run `target/release/examples/bench 1000 0 50 <seed> 0 <threads>`
for seeds 1–3 and threads 1 and 8, sequentially with other checks finished.
Each process must warm up with the requested thread count; the survey fixed
`prepared_world` to use `step_with(threads)`. Report `fall` µs/tick, visited
cells and total ms/tick, including spread. Use a small dense-water probe to
check whether ordering cost needs a fallback. Keep only a measured improvement.

Return the commit and concise before/after numbers, with any remaining cost.
No new report file, deployment, captures or retained benchmark binaries/data.
