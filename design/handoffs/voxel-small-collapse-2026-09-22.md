---
status: open
date: 2026-09-22
owner: Fable (orchestration); one Opus worker, medium effort; read-only diagnosis
---

# D5: why the small preset's plants collapse inside six hours

## Why

The small preset is the panel's world (`config/tachyon/voxel.toml`, 0.125 m
cells). The six-hour observer runs in `voxel-plant-layers-2026-09-22.md`
(census note, "Layers, 2026-09-22") show its standing foliage going from 4.7
organic to 0.26 with or without layers, every browser and shredder dead, and
the same run on default keeping 12.8 of 20.2 with both lineages alive. The
collapse predates layers and bodies; nothing has diagnosed it. The default
preset does not do it. Only the cell size, the landform recipe and its water
inventory differ.

## Objective

Read-only. Say, with numbers, what kills the small preset's plants: thirst
(pore moisture outside the species' bands after `for_voxel_size` scaling of
root boxes), light, maintenance clocks, drowning, being eaten, or failure to
establish replacements; and whether it is the water inventory, the cell size
scaling, or the seeder.

## Deliverables

1. `crates/cubarium/examples/voxel_plant_autopsy.rs` accepts `preset=<name>`
   (it hardcodes the authored world today) via `voxel::ambient_world`, and a
   `plants-only` arm (fauna never stepped) beside the coupled one.
2. Runs, 6 simulated hours each, all cores: small plants-only, small coupled
   (built-in founders), default plants-only. Per species per 30 min: stands,
   foliage, births, deaths **by cause** (the flora ledger books them; if it
   does not book a cause, say so and infer from the per-stand state at
   death: reserve, water μ, light, foliage), mean pore moisture in root
   boxes against the species' bands, mean light, and the water ledger
   (stored, atmosphere, showers, residual). Also at t = 0: per species how
   many seeded stands pass their own establishment predicate where they
   stand, on small vs default.
3. A `## D5 — the small preset` section in
   `design/7_Research/voxel-census-2026-09-20.md`: the table, one sentence
   naming the killer, and "proposed, not made" candidates ranked by the
   evidence (water inventory of the small recipe; a scaling rule in
   `for_voxel_size`; the seeder's site choice; something else). No tuning.

## Constraints

- Read-only on simulation semantics: examples and the research note only.
- Worktree `.claude/worktrees/small-collapse`, branch `small-collapse` from
  main; `CARGO_TARGET_DIR` inside it. Explicit-path commits only; every commit
  ends with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`,
  `design/organism-anatomy-2026-09-21.md`.
- A parallel package (`voxel-diets-2026-09-22.md`) edits the fauna crate and
  appends its own census section; do not edit fauna sources, and append your
  section at the end of the census note.
- `graft ask "<question>" --source` before opening files. No windows.

## Return (≤40 lines)

The table, the sentence, the ranked candidates, the commit list, what you
could not measure and why.
