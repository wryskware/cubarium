---
status: leaning
date: 2026-09-18
owner: Fable
---

# Voxel schedule: bevy_ecs as the tick's backbone, parallel inside phases

Wrysk (2026-09-18) decided the sim uses a solid ECS crate rather than hand-rolled
sets, with multithreading "early on because that's a huge performance win in
itself". The profile note (`design/7_Research/voxel-tick-profile-2026-09-18.md`,
"On using an ECS crate") recommended hand-rolled; Wrysk's decision stands, and
its two cautions carry: every sum or draw is taken over a stable key order, and
determinism is **not** a requirement (measurements are statistical, never
bit-exact). Base: `voxel-perf` at `3e826da` (local water model landed).

## Shape

- New crate `crates/cubarium-voxel-sim` owning a `bevy_ecs::World` and one
  `Schedule` per tick. `cubarium_voxel::World`, `Flora`, `Fauna` and their
  configs are **resources**; the voxel grid stays a dense resource, never
  one entity per voxel. Stands and animals stay in their crates' own stores
  this package; animals become entities when the senses spec lands
  (`design/handoffs/voxel-senses-handoff-2026-09-18.md`), which is when their
  body model is rewritten anyway. Say so in the crate doc.
- Systems, in the order the ledger needs and no looser: `rain`, the substep
  loop (`infiltrate`, `fall`, `exchange`, `drain`), `water_table`, `spring`,
  `flora`, `fauna`, then the samplers. Phase order is fixed by `.chain()` or
  system sets; the parallelism lives **inside** phases:
  - `exchange` is read-old/write-new already: split rows (`z` bands, or
    the wet set in chunks) across `bevy_tasks::ComputeTaskPool`.
  - `fall` and `infiltrate` the same where the write set is column-local.
  - fauna `sense` per animal in parallel (reads only), actions applied
    serially in animal-id order as today.
  - flora per-stand income in parallel if the stand step has a clean
    read-then-apply split; if it does not, leave it and say what blocks it.
  Reductions (ledger sums, lotteries) are gathered per chunk and folded in a
  stable order.
- The host (`crates/cubarium/src/voxel/mod.rs`) and the harness
  (`examples/harness/mod.rs`, `two_producers.rs`, `replacement.rs`,
  `grazed.rs`) step through the sim crate. The old three-call sequence may
  remain as a private helper for tests.
- Thread count is a `SimConfig` field (default: available parallelism).

## Verification

- Conservation: the water residual and both flora residuals close at the
  existing tolerances with the pool at 1, 4 and 16 threads; stored water and
  per-species mean root-box pore after 200 coupled ticks agree across thread
  counts within 1e-6 relative (float reassociation only). Statistical, not
  bit-exact; no pinned hashes.
- Existing fixtures: 41 core, flora, fauna and `cubarium` targets green.
- Bench: mean ± spread over 5 seeds, 1,000 ticks, 0 and 4 grazers, at 1, 4,
  8 and 16 threads; per-phase table at 16. Addendum to the profile note.
  State plainly what scales and what does not (the tick is a dependency
  chain; the win is within-phase).
- Dependency tail: list what `bevy_ecs` + `bevy_tasks` pull in and the
  release build-time delta once.

## Package

**S — schedule**, the persistent ecology worker, Opus high (concurrency),
worktree `voxel-perf`, commits per step (crate + resources + serial schedule;
parallel exchange; parallel sense; host/harness on the sim crate; bench).
Rules as every voxel brief: explicit-path commits, no cargo fmt, no tuning,
short function tests, never HashMap iteration, do not touch
`design/handoffs/README.md`, the cube, or `crates/cubarium/src/sink/**`.
Return ≤ 30 lines.
