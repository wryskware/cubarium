---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream W (Opus): the disc model on the apex alone, in the identical prey world

Fable orchestrates. The isolation [U](../7_Research/ecology-v1-apex-grasp-2026-09-16.md)
names as its next experiment and item 3 of the reconciled next steps in
[the round-4 result](../7_Research/ecology-v1-round4-results-2026-09-16.md)
now requires before any host contract or recalibration is paid for. You own
this brief; Fable reviews once with at most two repair cycles. Model: Opus 5,
medium reasoning effort (one variable on an existing paired harness, the third
time this harness has been extended this way). Time target: a short session.
**Worktree.** Run under `AGENTS.md` and `WORKING_POLICY.md`. Shared `target/`:
set `CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target`; a build failing
on a symbol the tests just used means another tree was building — `touch
crates/cubarium-core/src/lib.rs` and rebuild; pin
`CUBARIUM_SEARCH_BUILD=<your commit>` and copy the release binary out of the
shared target before running arms. First `git log --oneline -1` and `git reset
--hard <the brief commit>` if HEAD differs. `runs/` is git-ignored: read
retained rows from the main checkout and copy your outputs back there at the
end. Commit by path only; do not push, tag, restart the cube, or touch
`state/`, port 7393, the shim or the running `cubarium`. Commit messages end
with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Tests first,
from the definitions. Never add a `WorldConfig` field.

**Files you own:** `crates/cubarium-core/src/motor.rs` (additions only),
`crates/cubarium-core/src/world/{mod.rs,lifecycle.rs}` (one transient beside
`motor_model` and `apex_turn_radius`, with setter and getter),
`crates/cubarium-core/src/world/step.rs` (the motor-model reads at the lines
named below, nothing else), new files under `crates/cubarium-core/tests/`,
`crates/cubarium-search/src/apex_audit.rs` (one flag beside
`--apex-turn-radius`, recorded on the report and every row), its lines in
`crates/cubarium-search/src/main.rs`, `crates/cubarium-search/tests/`,
`design/7_Research/`, `runs/`. **Do not touch** `hunter/*`, `snapshot.rs`,
`calibrate.rs`, `evaluate.rs`, the `Calibrate`/`Precondition`/`Factorial`/`Es*`
commands in `main.rs`, or anything under `crates/cubarium/` (workstream V is
in them concurrently).

## Read first

- [T's note](../7_Research/ecology-v1-motor-inertial-2026-09-16.md) as
  corrected, and its arm A rows `runs/ecology-v1-motor-inertial/armA-{sweep,inertial}.json`:
  every body ran `Inertial`, so the prey world at introduction was 628
  against `Sweep`'s 745 and 15 of 16 rows differ; T's gain is not
  apportioned between the envelope and that thinner world.
- [U's note](../7_Research/ecology-v1-apex-grasp-2026-09-16.md) and its rows
  `runs/ecology-v1-apex-grasp/{grasp,lobes}.json`: the harness, the pattern
  (`ApexTurnRadius`, `turn_radius_px_in_with`, `World::set_apex_turn_radius`,
  `apex-audit --apex-turn-radius`), the byte-identical pin reused from T's
  six hashes, the field-for-field reproduction, the paired sign tests, E's
  usable-energy ratio as used, and the sign convention (+ = the gap grew).
- `crates/cubarium-core/src/world/step.rs`: the world's `motor_model` is
  read once per tick (≈ L255) and consulted per body at the envelope radius
  (≈ L1388, where `apex_geometry` is `Some` only for a hunter member), the
  resolver (≈ L1403), the bill (≈ L1454) and the rotation price (≈ L1472);
  `neural_decision` (≈ L3088) and `view.rs::neural_observation` (≈ L317) are
  neural animals only and never an apex member.

## Deliverables

1. **The switch:** a `World` transient `apex_motor_model: Option<MotorModel>`
   (or an equivalent you can justify in one sentence) — the motor contract a
   hunter member runs, `None` meaning the world's own. Every one of the four
   per-body reads in `step.rs` consults it for a body that has apex contact
   geometry, and no read for any other body changes. `apex-audit
   --apex-motor {sweep,inertial}` (default: the world's `--motor`), recorded
   on the report and every row. Tests first: an apex member's envelope
   radius, resolver, bill and rotation price under the override are the
   `Inertial` ones and an ordinary body's are the world's; default off is
   byte-identical against T's six pinned hashes (reuse U's fixture, run green
   at the brief commit before implementing); a world with the override set
   and no apex ever introduced is hash-identical to one without it over
   3,000 ticks; with the override the strike record's motor name (or a new
   per-row field, your call, stated) says which contract the member ran.
2. **The pair** (≤ 3 wall minutes, 8 workers): P's eight-seed two-apex design,
   `--pursuit-stop reach-envelope --motor sweep`, `--apex-turn-radius grasp`,
   with `--apex-motor sweep` and `--apex-motor inertial`. The off arm must
   reproduce U's `grasp.json` field for field. Prey at introduction must be
   identical between the arms, row for row (this is the whole point). Report
   U's table with four columns: `sweep`, `grasp-only` (U's retained rows),
   `apex-inertial` (yours), `inertial` (T's retained rows): gap change per
   burst, contacts and captures by initial-gap bin, captures per life,
   delivered translation and rotation, motor billed translation and turn, E's
   usable-energy ratio (state the formula; reproduce U's 18.48 % on the off
   arm as the check), lifetime mean and max, lives past the 24,000-tick age
   gate, death cause, readiness terms. Paired sign tests per run as U did,
   and the pooled figures with `fast-leaf/9006` removed.
3. **Verdict:** the envelope is the source of T's apex gain if
   `apex-inertial` in the identical prey world recovers most of `Inertial`'s
   −0.57 px closure and 2.9 captures per life (and the paired tests separate
   it from noise); the thinner prey world was the source if it stays near
   `grasp-only`'s −0.03 px and 2.4. State which, and state what remains
   unexplained either way. Do not recommend a contract; Fable and Astra do
   that.
4. **Result note** `design/7_Research/ecology-v1-apex-motor-isolation-2026-09-16.md`;
   tests green (`cargo test -p cubarium-core`, `-p cubarium-search`); `graft
   build`; commit on the branch. Storage `runs/ecology-v1-apex-motor-isolation/`
   ≤ 20 MiB.

Return: branch and commits, test totals, the four-column table, the verdict,
wall time, usage, evidence and reasoning.
