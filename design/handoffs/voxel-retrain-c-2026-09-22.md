---
status: open
date: 2026-09-22
owner: Fable (organism line), under Wrysk's delegation of the nine decisions
package: 5, part C — follows design/handoffs/voxel-retrain-2026-09-22.md
---

# P5-C — train and judge

## Where things stand

Branch `retrain` (worktree `.claude/worktrees/retrain`) carries P5-A (contract
v2: 45-ray fan −40..+40, occlusion below the surface, solid z edge,
`Chem(detritus)`, cruise 1 BL/s, true birth readiness, windowed cone, manifest
schema 2, teachers that back off) and P5-B (founding loop in
`cubarium-voxel-sim`, rebuilt arena with in-band crowns, caps and carrion,
bystanders and a 0.125 m variant; `Prepared::{Arena, Landscape}`; 48 training
landscapes, seeds 101–116 × small/default/wide, drained + mid-shower water,
all placed founders act, bodies drawn over [body_min, body_max]; per-body
scoring; per-episode diagnostics; `cubarium-search voxel-landscapes`). The
shipped centres do not validate under schema 2; the host falls back to the
heuristics loudly. Read the parent brief's decisions D1–D12 — they bind here.

Measured cost (P5-B): landscape episodes run 10–14k ticks/s per worker at
16 workers, a 4,800-tick landscape episode is 0.28–0.38 s of one worker; the
arena runs 35k–124k ticks/s per worker. All 96 landscapes per generation would
be ~160 s/generation, ~22 h per 512-update run — too slow.

## Decisions (Fable)

- **C1 Per-generation set.** Every generation evaluates all candidates on the
  16 arena Stage-B layouts (8 at 0.25 m, 8 at 0.125 m; 2,400 ticks) plus
  **16 landscapes drawn from the 96** (48 worlds × 2 water states), a fresh
  draw each generation from the run seed, identical for every candidate of
  that generation (antithetic pairs see the same worlds). Horizon 4,800 ticks
  for landscapes (D8). Expected ~30 s/generation.
- **C2 Schedule.** 32 antithetic pairs, 512 updates, as P3-C. A checkpoint
  every 32 updates, each evaluated on the **held-out** landscapes (seeds
  201–208 × three presets, drained state, the P5-B redraw stride) — the ship
  candidate is the best held-out checkpoint, not the last.
- **C3 Both lineages at once**, ~11 workers each, run in the background with
  logs under `runs/` (disposable, not committed).
- **C4 Imitation first (P3-C path).** Fix teacher recording on landscapes
  (per-body buffers — P5-B found the shared buffer interleaves the eight bodies
  and each reset clears it). Record the P5-A teachers on the arena layouts and
  all 96 training landscapes; fit the clone; report clone vs teacher on the
  held-out set before ES. ES warm-starts from the clone.
- **C5 Score unchanged** (D9). No shaping term. If a run collapses (held-out
  score falls steadily for 128 updates), stop it and report with the
  diagnostics rather than inventing a term.

## Also in this package

1. `policy=<lineage>=<file>` on `voxel_founder_autopsy`, `voxel_census`
   (and `voxel_edible_stock` if cheap), so candidates run in the live tools.
2. Move `STARTING_STORES_PROTOCOL` at ship (P5-B left it so the host would
   still start).
3. On ship: copy the two chosen centres into `crates/cubarium/assets/policies`,
   update its README (provenance: run seed, generation, held-out score,
   protocol hashes), and make the built-in-centre test require the new centres
   again (P5-A relaxed it to accept the fallback).

## The gate (decides ship)

`voxel_founder_autopsy 60` over seed bases 1–8 × small / default / wide:
**main at the merge base** (old centres) against **retrain + candidates**, and
against **retrain + heuristics** (the fallback) as a third arm. Report per
preset and arm: median [min–max] alive at 60 per lineage, first browser death
minute and cause (ledger), drowned/starved totals, bites per food class,
metres walked and blocked-motor share. Plus `voxel_census 6 preset=default` on
bases 1–3 for candidates and main.

**Ship** when, against main: browser median alive at 60 is above 0 on
`default` and `wide` and not lower on any preset; shredder median not lower on
any preset; drownings not up. If it fails, do not ship: report the tables and
the diagnostics that say why, and stop. One-seed rows are anecdotes.

Water note: `water_depth_m` counts falling rain as depth
(`voxel-terrain-note-standing-depth-2026-09-22.md`); training and live read
the same function, so they agree. Do not patch it here.

## Constraints

Work in the `retrain` worktree only; `CARGO_TARGET_DIR` inside it. Up to 23
cores. Explicit-path commits only; end each with the `Co-Authored-By` line you
were given. Fresh worlds only; no pinned hashes; tests ≤ 200 ticks. No tuning
of model rates or thresholds; training hyper-parameters stay as P3-C's unless
this brief says otherwise. Do not edit `design/handoffs/README.md`,
`design/README.md`, `config/tachyon/*`, `scripts/tachyon-*`,
`docs/tachyon.md`, `art/gen/*`, or the untracked
`design/organism-anatomy-2026-09-21.md`. Do not merge to main. Full workspace
suite (release) before reporting. Results go in the census doc as "Retrain,
2026-09-2x".

## Return (≤ 40 lines)

Commits; clone vs teacher; the training curves in three lines each (held-out
score at 0 / 128 / 256 / 384 / 512 and the chosen checkpoint); the gate tables'
headline rows and the ship verdict; surprises with evidence.
