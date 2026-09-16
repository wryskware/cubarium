---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream J (Opus): the controlled form × diet factorial

Fable orchestrates. Step 3 of the reconciled next steps in
[the next-steps result](../7_Research/ecology-v1-next-steps-results-2026-09-16.md),
from Astra's [review](../7_Research/ecology-v1-next-steps-review-2026-09-16.md)
(finding 3, next steps item 3). You own this brief; Fable reviews once with at
most two repair cycles. Model: Opus 5, high reasoning effort (a new founding
door in core and a matched design). Time target: one working session.

**You are in a separate git worktree.** `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`
on every cargo command (spurious extern/doctest failures mean another worker
was building — re-run; `touch crates/cubarium-core/src/lib.rs` clears a stale
artifact). Touch only `crates/cubarium-core/src/world/lifecycle.rs` (one new
founding door, see below), `crates/cubarium-core/tests/`, a **new**
`crates/cubarium-search/src/factorial.rs` (+ its CLI dispatch line and `lib.rs`
module line), `crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`.
Another worker owns `step.rs`/`budget.rs`/`neural/`/`es/`; another owns
`hunter.rs`/`apex_audit.rs`; another owns `calibrate/evaluate/movement/params`.
Commit on your worktree branch by path. Do not push, tag, restart the cube, or
touch `state/`, port 7393, the shim or the running `cubarium`. Commit messages
end with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.

## Objective

Decide whether the skimmer's loss is its diet or its body, with a matched
factorial instead of bins of descendants: clone founders at the same tick and
locations with only `diet` changed, then hold `diet` fixed and vary form, with
mutation and reproduction off, measuring yield with E's ledger.

## Read first

- `design/7_Research/ecology-v1-movement-2026-09-16.md` "The skimmer: which
  leg is it?" as corrected (the association, the confound, the factorial named).
- `crates/cubarium-core/src/genome.rs` (v2 genome: `diet`, `form`, `depth`,
  `swim`; `Genome::founder`; the decode to `cap_foliage = φ(diet)`,
  `cap_detrital = φ(1 − diet)` with `θ = 0.2`, `γ = 1`), `crates/cubarium-core/src/world/lifecycle.rs:52-63`
  (the 24-founder roster: burrower 4, grazer 10, glider 5, skimmer 5 — what
  genome each kind gets, especially the skimmer's `diet` ≈ 0.60 and `form`),
  `found_training_animal` / `found_neural_animal` (the existing doors; yours
  is a sibling that takes an explicit `Genome`).
- `design/7_Research/ecology-v1-budget-2026-09-16.md`: the ledger
  (`World::record_body_budgets`, `body_budget`, closed records) — served,
  digestible, credited, oxidised, billed, terminal stores by channel.
- `design/ecology-v1-contract.md` §6 (diet, capability, intake, upkeep).

## Deliverables

1. **Core: one founding door** `World::found_animal_with_genome(pos, heading,
   genome) -> Result<OrganismId, String>` beside the two existing doors, same
   accounting (`external_material_in` booked, capacity refused by name), adult
   at founding, `Origin::Founder`. Tests first: founding with the roster's
   skimmer genome equals founding a skimmer through the ordinary roster path
   for every phenotype field; the material box closes; capacity is refused.
   No other core change.
2. **A way to switch mutation and reproduction off for the run** — verify
   what `WorldConfig` already offers (the ES `Layout::config` disables
   mutation; the training seam disables budding via `bud: Some(false)`); use
   what exists, and if reproduction cannot be disabled from config alone, use
   the diagnostic seam the ES fixtures use. Say what you used.
3. **The factorial** (≤ 10 wall minutes of simulation, 8 workers), in
   `fast-leaf` whole worlds with the ordinary 24 legacy founders present
   (so food competition is realistic) plus the clones, mutation and
   reproduction off for the clones (and for everyone if that is the only
   option — say so), horizon 90,000, 4 training seeds:
   - **Arm A (diet within body):** 8 skimmer-form clones at fixed locations
     (the same 8 cells in every arm), 4 at `diet` 0.60 (the founder's) and 4
     at 0.85, interleaved by location.
   - **Arm B (body within diet):** 8 clones at `diet` 0.85, two of each form
     (burrower, grazer, glider, skimmer), same 8 cells.
   - **Arm C (the founder pairing):** each form at its own roster diet, two
     each, same 8 cells — the observational baseline.
   Ledger on for every clone. Report per clone and per cell of the factorial:
   lifetime, death cause, served / digestible / credited / billed by channel,
   net margin, distinct cells, and the cell classes stood in (wet floor vs
   not, since the skimmer's niche is algae on the wet floor).
4. **The verdict:** does changing only `diet` from 0.60 to 0.85 rescue the
   skimmer body (Arm A)? At fixed `diet` 0.85, does form still matter (Arm B)?
   State effect sizes with the seed spread, and which of "diet", "body",
   "both", "neither — habitat" the design supports. If the roster's skimmer
   genome differs from the others in more than `diet` and `form` (depth,
   swim, drives), say so and treat it as a confound you did or did not
   control.
5. **Result note** `design/7_Research/ecology-v1-diet-factorial-2026-09-16.md`:
   design, build, commits, commands, tables, verdict, what this does not
   establish (no `γ` change, no reproduction, one ecology, four seeds), the
   next task named (not launched). No `γ > 1` experiment is to be run here.
6. `cargo test -p cubarium-core`, `cargo test -p cubarium-search` green;
   `graft build`; commit on the branch.

## Constraints

- No equation, constant, snapshot or neural change; core change limited to
  the one door and its tests.
- Compute ≤ 10 wall minutes, ≤ 8 workers; `runs/ecology-v1-diet-factorial/`
  ≤ 20 MiB.
- Ask no questions; record routine choices.

## Decision authority

Yours: the 8 cells, subcommand and flag names, how cell class is read.
Fable's: any core change beyond the door, any equation.

## Verification

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-core
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test -p cubarium-search
```

Fable re-runs one seed of Arm A and checks it reproduces.

## Return format

The note, plus: branch and commits, test totals, the three arms' tables in
compact form, the verdict with effect sizes, wall time, usage, and the
evidence and reasoning behind each decision.

## Stop

Stop after the note and commit. Fable merges.
