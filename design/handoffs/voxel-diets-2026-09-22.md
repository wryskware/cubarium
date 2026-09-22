---
status: open
date: 2026-09-22
owner: Fable (orchestration); one Opus worker, high effort
---

# Package 3: the diets the decisions chose

## Why

Decision §3 (`voxel-organism-decisions-2026-09-21.md`): the browser eats
every vascular species' foliage inside its mouth band and nothing fungal; the
shredder is a detritivore with three foods, litter, glowcap cap tissue and
carrion, steering by a detritus cue. Today (audit §2) the browser bites
glowcap caps as if they were leaves, the shredder eats litter only, and
carrion decomposes with no consumer. Layers (`voxel-plant-layers-2026-09-22.md`)
and bodies in metres (`voxel-body-anchors-2026-09-22.md`) are on main; this
package is the food-web edges on top of them. It changes no sensing shape:
the Chem channel keeps its slot and digest and reads detritus instead of
litter, which is a meaning change the scheduled retrain (package 5) owns;
say so on the channel and in `assets/policies/README.md`.

## The rules

- **Browser acceptance**: a foliage layer is food only if its species is not
  a saprotroph (`Trophic`). Glowcap is not browser food.
- **Shredder foods**, each through the existing mouth band and reach: (1)
  litter at the standing face, as today; (2) glowcap cap tissue, taken as a
  foliage bite from a glowcap stand whose layer intersects the shredder's
  band (0 to 1.33 × its 0.0625 m height), booked through `take_foliage`; (3)
  carrion at the standing face, withdrawn from the ground carrion pool with
  its organic, mineral and energy exactly as the pool holds them. Order when
  several are present: whatever the controller's held action already means
  (bite = take from what is at the mouth), taking the richest available
  first is acceptable if it is stated; no new action.
- **Yield per food class** is an authored placeholder on the shredder's
  physiology: litter keeps its current yield; cap tissue uses the foliage
  yield; carrion uses the litter yield until told otherwise. All three in
  `design/backlog.md` §1. Assimilation stays `min(organic yield,
  mineral / 0.05)`.
- **Detritus cue**: the litter field's source becomes litter + carrion +
  glowcap cap stock at each face, same transport, decay and threshold, same
  channel. Rename the type honestly (`DetritusField`) without renaming the
  manifest module id.
- **Carrion** already appears on drown and removal, and eggs lost to water
  become carrion; nothing else changes about how it arises or decays.
- The imitation teacher (`BlindForager`) is untouched.

## Deliverables

1. Tests, ≤200 ticks, authored from this brief before implementing: a
   shredder on a face with no litter and a glowcap cap in its band eats the
   cap and the flora and fauna ledgers conserve; a shredder on carrion eats
   it and the ground pool and fauna ledger conserve; a browser next to a
   glowcap does not bite it and next to a bloomcrown rosette does; the cue
   field rises over a carrion pool with no litter; the built-in centres
   still validate (digest unchanged); the cue channel's value is unchanged
   on a litter-only world (so the field is the litter field when there is
   only litter).
2. The rules above in `cubarium-voxel-fauna` (`step.rs`, `senses.rs`,
   `lib.rs`, physiology) and wherever the flora exposes the carrion pool;
   fauna snapshot schema bump if state changed; fresh worlds only.
3. The autopsy and observer report bites by food class for the shredder and
   the detritus block per food (the observer's detritus block already exists).
4. Measurement, after: `voxel_founder_autopsy 60 preset=small` and
   `preset=default` (shredder deaths by cause, bites by food, alive at 60;
   browser bites by species, glowcap must be 0), and `voxel_census 6
   preset=default` shredders alive at 6 h (before: 10). Append to the census
   note as "Diets, 2026-09-22".

## Constraints

- No knob tuning beyond the listed placeholders; no golden hashes; fresh
  worlds; tests ≤200 ticks. The trained-policy digest and shipped JSON stay.
- Worktree `.claude/worktrees/diets`, branch `diets` from main;
  `CARGO_TARGET_DIR` inside it. Explicit-path commits only; every commit ends
  with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never touch `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`,
  or `design/organism-anatomy-2026-09-21.md` (Wrysk's untracked file).
- A parallel read-only diagnosis (`voxel-small-collapse-2026-09-22.md`) runs
  in another worktree and may edit `voxel_plant_autopsy.rs` and the census
  note; do not edit `voxel_plant_autopsy.rs`, and append your census section
  at the end so the two merge cleanly.
- `graft ask "<question>" --source`; `graft callers take_foliage --depth
  all` and `graft callers LitterField --depth all` before changing them. No
  windows.

## Verification

`cargo nextest run --workspace --exclude cubarium-gpu` green; deliverable 1;
Fable re-runs one arm.

## Return (≤40 lines)

The two autopsy arms with bites by food class, the 6 h shredder count, the
commit list, and anything the rules could not express.
