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

## Integration note (Fable, 2026-09-22)

Landed 41d0a25 (`voxel_plant_autopsy` reads a preset world; `coupled` arm)
and f2ac848 (census "D5"). Fable re-ran both presets at t = 0 and confirmed
the seeding picture: small seeds only springturf, glowcap, bloomcrown and
stonecushion, every root box at pore 0.25 (field capacity, no standing
water anywhere); default seeds all six with pore 0.48–0.65 under the wet
species. The conductivity claim checks in `water.rs`: the per-tick rate is
`permeability · DT · pore_capacity · voxel_volume`, a cell fraction, so the
physical flux is `permeability · pore_capacity · voxel_m` metres per second
and halves at 0.125 m.

| 6 h | small plants-only | small coupled | default plants-only |
| --- | --- | --- | --- |
| species seeded | 4 | 4 | 6 |
| foliage t = 0 → 6 h | 4.7 → 0.66 | 4.7 → 0.26 | 20.2 → 25.8 |
| establishments / deaths | 33 / 29 | 12 / 29 | 215 / 141 |
| drowned at minute 10, first shower | 14 of 27 | 14 of 27 | 3 of 87 |
| umbrellafrond-eligible columns at t = 0 | 0.4 % | 0.4 % | 27.8 % |

The killer: the small ring has no wet ground, so the two producers that are
solvent anywhere (umbrellafrond, velvetpad, 6–18× maintenance) are never
planted, and the four it does get are insolvent at field capacity on default
too (there they persist by turnover, 215 establishments; on small there is
no donor). Two units-and-rules findings came with it, both decided under
Wrysk's delegation: (1) water conductivity is a cell fraction per second,
half the physical speed at 0.125 m: package 1c, metres per second with the
reference grid identical; (2) the seeder judges eligibility before the
first shower and floors the wet species' founder counts to zero: package 4
seeds after the first scheduled shower has fallen and drained. The small
recipe's water split itself (38 % of the same 1.5 m buried under a lifted
lake floor, 2.4 % pooled vs 8.7 %) is the terrain line's lever
(`lake_depth_m`, the tier-0 trough) and is handed to it in
`voxel-terrain-note-small-water-2026-09-22.md`, not decided here. Whether a
producer should be solvent at field capacity is a model question left open
(backlog, not tuning).
