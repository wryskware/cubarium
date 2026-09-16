---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream Z (Opus): the coupled grazed opening — burn in with an ordinary roster, remove it, found again

Fable orchestrates. Item 6 of the reconciled next steps in
[the round-4 result](../7_Research/ecology-v1-round4-results-2026-09-16.md),
in Astra's words: *leave §11; compare conservation-accounted snapshots of the
full coupled field after an ordinary roster has produced the grazed state
(burn-in population removed, identical fresh roster founded) against the
status quo and the 48,000-tick plant-only opening, with opening and one-hour
frames, exact plant budgets and founder broods. No uniform total written
into §11.* This is the measurement [S](../7_Research/ecology-v1-precondition-2026-09-16.md)
§7 named under "Neither": the thing that settles the field is the founding,
and the honest target for any §11 change is the *grazed* standing crop, not
the ungrazed one. **No §11 change and no `producer.initial_fraction` change
is proposed by this workstream**; it measures. You own this brief; Fable
reviews once with at most two repair cycles. Model: Opus 5, **high**
reasoning effort (one core operator with a conservation contract, plus a
comparison whose reading must be honest about denominators). Time target:
one session. **Worktree.** Run under `AGENTS.md` and `WORKING_POLICY.md`.
Shared `target/`: set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`
and run `touch crates/cubarium-core/src/lib.rs crates/cubarium-search/src/lib.rs
crates/cubarium-search/src/main.rs` immediately before **every** cargo
invocation (two other trees build concurrently); pin
`CUBARIUM_SEARCH_BUILD=<your commit>` and copy the release binary out of the
shared target before running rows. **Never run `cargo fmt`** in this
repository. First `git log --oneline -1`; if HEAD's subject does not begin
"Ecology v1 round 5: brief Z", run `git reset --hard main`. `runs/` is
git-ignored: read retained rows from the main checkout and copy your outputs
back there at the end. Commit by path only; do not push, tag, restart the
cube, or touch `state/`, port 7393, the shim or the running `cubarium`.
Commit messages end with `Co-Authored-By: Claude Fable 5.1
<noreply@anthropic.com>`. Tests first, from the definitions. Never add a
`WorldConfig` field.

**Files you own:** `crates/cubarium-core/src/world/lifecycle.rs` (one new
operator beside `found_roster`; nothing else in core),
`crates/cubarium-core/tests/found_roster.rs` (extend) and new files under
`crates/cubarium-core/tests/`, `crates/cubarium-search/src/precondition.rs`,
the `Precondition` command's lines in `crates/cubarium-search/src/main.rs`,
`crates/cubarium-search/tests/precondition_measures.rs` (extend) and new
files under `crates/cubarium-search/tests/`, `design/7_Research/`, `runs/`.
**Do not touch** `es/`, `census.rs`, `evaluate.rs`, `calibrate.rs`,
`factorial.rs`, `apex_audit.rs`, `population.rs`, `hunter/`, `motor.rs`,
`snapshot.rs`, `step.rs`, or other commands in `main.rs` (workstreams X and
Y are in them concurrently).

## Read first

- [S's note](../7_Research/ecology-v1-precondition-2026-09-16.md) in full:
  `World::found_roster` and its two refusals (animals already present; a
  config with no roster), the `precondition --stage field|compare` operator,
  the recorder's relative origin, the measures (§4: founders by kind that
  bred, first brood tick, lineages alive, form evenness, crossings on the
  arm's own and on the common §11 reference, terminal starved cells; §5: the
  first simulated hour; §6: the frames through `shoulder_sheet::shot`), the
  reproduction checks (§2: 12 of 12 status-quo hashes), §7's three options
  and its "Neither" reading, §8's limits, and §10's decisions. S's rows:
  `runs/ecology-v1-precondition/{field.jsonl, compare.jsonl, configs/,
  field/}` (build under schema 16, half-space; arm 0 has no predator, so the
  age-0 status-quo rows are your reproduction targets across the schema-17
  predicate change, `state_hash` for `state_hash`; say so).
- [M's note](../7_Research/ecology-v1-plant-budget-2026-09-16.md) for the
  exact per-cell plant budget (`--plant-record`) and the depletion split;
  [E's note](../7_Research/ecology-v1-budget-2026-09-16.md) for the
  conservation identities the world keeps (`external_material_in`,
  material and energy residuals).
- Astra's round-4 review, next-steps item 6, and the §11 discussion in the
  round-4 results ("What this does and does not establish"): no uniform
  total; the grazed standing crop every arm converges to (215–230) is the
  quantity of interest.

## Deliverables

1. **The operator, core, tests first.** `World::remove_all_animals()` (or a
   name you justify in one sentence): removes every organism and every
   neural entry, leaves the field, water, weather, detritus and every pool
   untouched, and books the removed bodies' `structure + reserve` exactly
   into an external-out counter that the conservation identity sees (the
   mirror of what `found_roster` books into `external_material_in`), so a
   burn-in-then-found world's residuals are still ≤ 1e−9. Tests, in
   `found_roster.rs` or beside it: removal on a world with N animals leaves
   zero and books their material to the digit; the field's `P/W/Q/N` totals
   and every cell are byte-identical before and after; `found_roster` after
   removal succeeds and places the ordinary roster with `born_tick` at the
   removal tick; the removed-then-founded world steps on and keeps its
   invariants over 2,000 ticks; removal on an empty world is a no-op that
   books zero. No `WorldConfig` field.
2. **The operator, search:** `precondition --stage grazed` — for each
   (candidate, seed): run the *ordinary* coupled world (roster founded at
   tick 0, arm 0) to the declared ages, and at each age save the coupled
   field's state, remove the population, found the identical fresh roster
   through the door, and run S's 180,000-tick comparison after it with the
   ledger, the plant record and the depletion split on, exactly as `--stage
   compare` does, with the same row shape plus: the burn-in population and
   its composition at removal, the material removed, the field's `P/W/Q/N`
   totals and per-cell spread at the founding instant, and the opening's
   absolute foliage. Ages {48,000, 96,000, 180,000} (S's plant-only ages,
   so the coupled and plant-only openings are compared at the same age).
   Budget: 12 burn-in runs to 180,000 with three saved states each, 36
   comparison runs of 180,000, 8 workers, ≤ 8 wall minutes, storage
   `runs/ecology-v1-grazed-opening/` ≤ 80 MiB.
3. **The comparison, pre-registered before a row exists** (commit it
   first): three openings side by side per configuration — status quo (S's
   retained age-0 rows, reproduced), plant-only 48,000 (S's retained rows),
   and coupled-grazed at each age (yours) — on S's §4/§5 measures plus:
   `late foliage ÷ opening foliage` **and** absolute late foliage (the
   denominator problem S §7 raised), terminal starved cells on both
   references, exact plant income and loss over the first hour, founder
   broods by kind, and the standing crop the arm converges to. Frames at the
   founding instant and one simulated hour after it, one seed and one face,
   through S's `shoulder_sheet::shot` path, for status quo, plant-only
   48,000 and coupled-grazed 48,000. **Reading rule, stated before you run:**
   the coupled-grazed opening is *the better target for a §11 change* if it
   gives S's 48,000 founder gains (every grazer founder breeds, lineages
   alive at least 1.5× status quo, evenness up) **and** its opening is within
   20 % of the grazed standing crop the arm converges to (so it does not
   brown over the first hour as the plant-only opening does) **and**
   terminal starved cells are within noise of status quo; it is *not* if
   any of those fails; *mixed* otherwise, said plainly. Report which, per
   configuration; the selected ecology is `fast-leaf`.
4. **Result note** `design/7_Research/ecology-v1-grazed-opening-2026-09-16.md`
   with a "what Wrysk would be choosing — stated, not decided" section in
   S's form (leave §11 / adopt a coupled-grazed opening at a declared age,
   with its computation cost and what the display shows / neither), the
   denominators stated, and the routine decisions you made; tests green
   (`cargo test -p cubarium-core`, `-p cubarium-search`); `graft build`;
   commit on the branch.

Return: branch and commits (pre-registration first), test totals, the
reproduction of S's status-quo rows, the three-opening table per
configuration, the frames' paths, the reading by the rule above, wall time,
usage, evidence and reasoning.
