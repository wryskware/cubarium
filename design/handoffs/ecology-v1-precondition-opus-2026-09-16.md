---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream S (Opus): a plant-only preconditioned opening, compared with status quo

Fable orchestrates. Item 4 of the reconciled next steps in
[the round-3 result](../7_Research/ecology-v1-round3-results-2026-09-16.md),
from Astra's [round-3 review](../7_Research/ecology-v1-round3-review-2026-09-16.md)
(P2 on M, next steps item 4). You own this brief; Fable reviews once with at
most two repair cycles. Model: Opus 5, high reasoning effort. Time target: one
session. **Worktree.** Run under `AGENTS.md` and `WORKING_POLICY.md`. Shared `target/`: set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`; a build failing on a symbol the tests just used means another tree was building — `touch crates/cubarium-core/src/lib.rs` and rebuild; pin `CUBARIUM_SEARCH_BUILD=<your commit>` in recorded stamps. If you are in a worktree, first `git log --oneline -1` and `git reset --hard <the brief commit>` if HEAD differs. `runs/` is git-ignored: read retained rows from the main checkout and copy your outputs back there at the end. Commit by path only (never `git add -A`); leave `.claude/*`, `WORKING_POLICY.md`, `.agents/` and other uncommitted design documents alone. Do not push, tag, restart the cube, or touch `state/`, port 7393, the shim or the running `cubarium` (the cube is live for the owner). Commit messages end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Tests are their own pass, written from the definitions before the implementation. Ask no questions; record routine choices in the note. Never change `WorldConfig`'s fields: adding one changes `calibrate::config_hash` for every existing TOML and breaks policy provenance and every retained row.

**Files you own:** `crates/cubarium-core/src/world/lifecycle.rs` (one door, see
below), `crates/cubarium-core/tests/`, `crates/cubarium-search/src/{calibrate,depletion,evaluate}.rs`,
a new `crates/cubarium-search/src/precondition.rs` (+ `lib.rs` line and CLI
dispatch line), `crates/cubarium-search/tests/`, `design/7_Research/`,
`design/7_Research/assets/`, `runs/`. Another worker owns the hunt-intent
region of `step.rs`, `hunter.rs`, `apex_audit.rs`; another owns `es/`; another
adds `census.rs`. `crates/cubarium/` (the presenter) may be used read-only to
render frames through the real `ArtPresenter` (the `shoulder_sheet` and
`meal_capture` examples show how); do not edit it.

## Read first

- [M's note](../7_Research/ecology-v1-plant-budget-2026-09-16.md) as
  corrected: the per-cell plant budget (`--plant-record`), `--no-animals`, the
  confirmed over-seeding (§11 seeds 9–15 % of watched cells above what the
  plant step sustains; crossings are opening-stock declines with zero
  withdrawal), the owner's options A (plant-only warm-up) and B (endpoint
  fit, demoted to an initialiser), and the revised next task: **a whole-field
  plant-only preconditioning comparison**. Astra: define a deterministic,
  seed- and config-bound preconditioning operator (initialise the full §11
  field, advance the ordinary §4 dynamics with animals absent for a declared
  duration or until declared moving-window criteria are met, then found the
  animals from that joint state); call the result a fixed-age preconditioned
  opening, not an equilibrium; no universal critical `L·μ`; provide Wrysk the
  opening frames and early founder outcomes against status quo. The absent
  arm's first crossing is at tick 42,000–55,000 and the median about 145,000.
- `crates/cubarium-core/src/world/lifecycle.rs:52-63` (the ordinary founding
  of the 24-kind roster) and J's `found_animal_with_genome`; `fields.rs::react`.

## Deliverables

1. **Core: one door, `World::found_roster()`**, that performs the ordinary
   founding (the same 24 founders at the same positions and genomes the
   constructor would place for this config and seed) on an existing world at
   its current tick, refusing if animals are already present; books
   `external_material_in` as the constructor does. Tests first: founding into a
   fresh tick-0 world equals the constructor's world (state hash); founding
   into a world stepped plant-only for 1,000 ticks places the same genomes;
   refusal by name when animals exist. No other core change; no `WorldConfig`
   field.
2. **The preconditioning operator**, search side: `precondition --ages
   <ticks,…>` runs each (config, seed) plant-only (`--no-animals` path) with
   the plant record on, saves the whole-field state at each age, and reports
   moving-window (6,000-tick) changes in total and per-cell `P`, `W`, `Q`, `N`,
   exact plant income and loss, threshold crossings and spatial variance at
   each age — so "how settled is the field at age T" is measured, not
   assumed.
3. **The comparison** (≤ 10 wall minutes, 8 workers): both configurations,
   the 6 training seeds, ages {0 (status quo), 48,000, 96,000, 180,000};
   at each age found the roster with the door and run 180,000 ticks with the
   ledger, plant record and depletion split on (arm 0). The age-0 rows must
   reproduce M's present-arm rows by `final_state_hash`. Report per age:
   opening foliage distribution (total, median, CV), the counter's two
   branches (crossings with exact withdrawal / without) over the run, founder
   survival and first broods by kind, population and kinds alive at the
   horizon, foliage retention, and the first-hour plant-budget trajectory.
4. **Opening frames for Wrysk**: through the real `ArtPresenter` with normal
   assets at 64 × 64, one Front face at founding for each age beside status
   quo, and the same face one simulated hour later — one PNG under
   `design/7_Research/assets/`, a few hundred KB at most. Say plainly that
   the physical cube was not inspected.
5. **Verdict:** at which age (if any) the ungrazed crossings vanish while the
   founders' outcomes are at least as good as status quo; what the display
   would visibly lose (the green-then-fade succession) and gain
   (heterogeneity, a greener opening, by how much). State what Wrysk would be
   choosing: leave §11; adopt preconditioning at a declared age (option A with
   this operator); or neither. Do not change §11 or `producer.initial_fraction`.
6. **Result note** `design/7_Research/ecology-v1-precondition-2026-09-16.md`,
   tests green (`cargo test -p cubarium-core`, `-p cubarium-search`), `graft
   build`, commit on the branch. Storage `runs/ecology-v1-precondition/`
   ≤ 80 MiB (saved states count; keep only the ages used).

Return: branch and commits, test totals, the per-age table in compact form,
the frames' path, the verdict with what Wrysk would be choosing, wall time,
usage, evidence and reasoning behind each decision.
