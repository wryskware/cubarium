---
status: open
date: 2026-09-22
owner: Fable (orchestration); one Opus worker, medium effort
---

# Package 1c: water conductivity in metres per second

## Why

D5 (`voxel-small-collapse-2026-09-22.md`, integration note) found that the
water solver's infiltration and drainage rate is a cell fraction per tick:
`rate = permeability_per_s · DT · pore_capacity · voxel_volume` (m³ per tick
per cell) in `crates/cubarium-voxel/src/water.rs` (three sites near lines
757, 788, 1569 at 4661f97). The physical flux is therefore
`permeability · pore_capacity · voxel_m` metres per second, half as fast on
the panel's 0.125 m cells as on 0.25 m. The first shower on small stood
0.03–0.08 m deep and drowned 14 of 27 stands at minute 10; on default 3 of
87. Same class of bug as the shade area in cells² (package 1b): decision §7
says authored geometry and transport are in metres and the cell is the
discretisation.

## The rule

Flux between two cells is a physical conductivity `K` in metres per second
times the shared face area in m² times `DT`: `rate_m3 = K · face_area ·
DT`, with `K = permeability_per_s · pore_capacity · 0.25` so that the 0.25 m
reference grid is numerically identical, digit for digit, before and after
(state that as the conversion on the material's field; rename the field or
add the derived accessor so an old config cannot be misread). Every site
that scales a flux by `voxel_volume` (surface infiltration, lateral pore
flow, aquifer exchange, spring or re-entry if they use the same form) moves
to the face-area form; list them. Storage terms (`pore_room_m3`, `room`,
`level`) stay volumes. Evaporation and rain are already per area; check and
say so.

## Deliverables

1. Tests, ≤200 ticks, authored before the change: the reference-grid
   identity for one column draining and one lateral exchange (assert equal
   to the old expression); a 1 m slab of the same material drains the same
   physical volume per second on 0.125 m and 0.25 m grids within
   discretisation error (state the tolerance and why); the water ledger
   conserves across both.
2. The rule at every listed site; world snapshot schema bump if any
   serialised material field changed (fresh worlds only).
3. Measurement, after: `voxel_plant_autopsy 1 preset=small` and `preset=
   default` (drownings by minute 10 and at 60 min, before → after; the
   D5 arms are the before), `water_cycle` example on both presets for 1 h
   (stored, pooled, residual). Append to the census note as "Water units,
   2026-09-22" at the END (two other sections are being appended by
   parallel workers; keep yours last).

## Constraints

- No tuning: `permeability_per_s` and `pore_capacity` keep their values;
  only the unit of the derived flux changes. No golden hashes; fresh worlds.
- Worktree `.claude/worktrees/water-units`, branch `water-units` from main;
  `CARGO_TARGET_DIR` inside it. Explicit-path commits only; every commit ends
  with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`,
  `design/organism-anatomy-2026-09-21.md`, fauna sources, or
  `voxel_plant_autopsy.rs`.
- `graft ask "<question>" --source`; `graft grep "voxel_volume"` for the
  exhaustive site list before changing anything. No windows.

## Verification

`cargo nextest run --workspace --exclude cubarium-gpu` green; deliverable 1;
the perf schedule tests still pass (`cubarium-voxel-sim::schedule`); Fable
re-runs one arm.

## Return (≤40 lines)

The site list with before/after forms, the identity and invariance test
results, the drowning table, the commit list, anything the rule could not
express.
